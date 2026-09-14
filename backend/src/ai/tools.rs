use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::{mpsc, oneshot, Mutex as AsyncMutex};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// 工具执行结果：文本回复 + 产出的图片（相对会话图片目录的路径）。
pub struct ToolResult {
    pub content: String,
    pub images: Vec<String>,
}

impl ToolResult {
    pub fn text(content: String) -> Self {
        ToolResult {
            content,
            images: Vec::new(),
        }
    }
}

pub struct AiToolContext {
    pub user_id: String,
    pub biz_type: String,
    pub biz_id: String,
    pub root_path: String,
    /// AI 会话存储根目录（工具产图落盘用），业务未接入时为 None
    pub chat_store_dir: Option<std::path::PathBuf>,
    pub chat_id: String,
    /// 页面查询桥（如 PDF 页码/截图查询），业务不支持时为 None
    pub page_query: Option<Arc<PageQueryBridge>>,
}

/// 工具向用户页面查询数据的桥接：后端按业务需要发起 `page_query`（SSE 事件，携带
/// kind + 参数），页面自行决定如何应答并 POST /api/ai/chat/page_query_result 回传，
/// 按 request_id 交付给等待中的工具。回复内容为任意字符串，格式由各 kind 自行定义
/// （如电子书 PDF 截图回传 JSON{page,image}；密码库/云笔记可回传解密后的文本）。
/// tx 只承载查询帧（纯 Bytes），由调用方的泵任务转发到 SSE 主通道。
pub struct PageQueryBridge {
    tx: mpsc::Sender<actix_web::web::Bytes>,
    pending: Arc<AsyncMutex<HashMap<String, oneshot::Sender<String>>>>,
}

const QUERY_SEND_TIMEOUT_MS: u64 = 500;
const QUERY_RESULT_TIMEOUT_SECS: u64 = 20;

impl PageQueryBridge {
    pub fn new(
        tx: mpsc::Sender<actix_web::web::Bytes>,
        pending: Arc<AsyncMutex<HashMap<String, oneshot::Sender<String>>>>,
    ) -> Self {
        PageQueryBridge { tx, pending }
    }

    /// 发起页面查询并等待回传，成功返回应答内容（格式由 kind 定义）。
    pub async fn query(
        &self,
        kind: &str,
        params: serde_json::Value,
    ) -> Result<String, String> {
        let request_id = uuid::Uuid::new_v4().to_string();
        let (otx, orx) = oneshot::channel();
        self.pending
            .lock()
            .await
            .insert(request_id.clone(), otx);
        let frame = actix_web::web::Bytes::from(format!(
            "event: page_query\ndata: {}\n\n",
            serde_json::json!({ "request_id": request_id, "kind": kind, "params": params })
        ));
        let sent = tokio::time::timeout(
            std::time::Duration::from_millis(QUERY_SEND_TIMEOUT_MS),
            self.tx.send(frame),
        )
        .await;
        match sent {
            Ok(Ok(())) => {}
            _ => {
                self.pending.lock().await.remove(&request_id);
                return Err("页面连接已断开，无法查询".to_string());
            }
        }
        match tokio::time::timeout(
            std::time::Duration::from_secs(QUERY_RESULT_TIMEOUT_SECS),
            orx,
        )
        .await
        {
            Ok(Ok(data)) if !data.is_empty() => Ok(data),
            Ok(Ok(_)) => {
                self.pending.lock().await.remove(&request_id);
                Err("页面查询失败".to_string())
            }
            _ => {
                self.pending.lock().await.remove(&request_id);
                Err("等待页面应答超时".to_string())
            }
        }
    }
}

/// page_query_result 接口回填：按 request_id 交付应答，返回是否成功送达等待方。
pub async fn resolve_page_query(
    pending: &Arc<AsyncMutex<HashMap<String, oneshot::Sender<String>>>>,
    request_id: &str,
    data: String,
) -> bool {
    match pending.lock().await.remove(request_id) {
        Some(tx) => tx.send(data).is_ok(),
        None => false,
    }
}

#[async_trait]
pub trait AiToolProvider: Send + Sync {
    fn biz_type(&self) -> &str;

    fn max_tool_rounds(&self) -> u32;

    fn tool_definitions(&self) -> Vec<ToolDef>;

    /// 业务上下文前缀（如电子书告知当前书籍），随每轮请求置于会话输入最前；无则 None
    fn context_prefix(&self, _ctx: &AiToolContext) -> Option<String> {
        None
    }

    async fn execute(
        &self,
        ctx: &AiToolContext,
        name: &str,
        arguments: &str,
    ) -> Result<ToolResult, String>;
}

#[derive(Default)]
pub struct AiToolRegistry {
    providers: RwLock<HashMap<String, Arc<dyn AiToolProvider>>>,
}

impl AiToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, provider: Arc<dyn AiToolProvider>) {
        if let Ok(mut map) = self.providers.write() {
            map.insert(provider.biz_type().to_string(), provider);
        }
    }

    pub fn get(&self, biz_type: &str) -> Option<Arc<dyn AiToolProvider>> {
        self.providers
            .read()
            .ok()
            .and_then(|map| map.get(biz_type).cloned())
    }
}
