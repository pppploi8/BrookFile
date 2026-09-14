use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{Stream, StreamExt};
use genai::adapter::AdapterKind;
use genai::chat::{
    Binary, ChatMessage, ChatOptions, ChatRequest, ContentPart, MessageContent,
    ReasoningEffort, Tool, ToolCall, ToolResponse,
};
use genai::resolver::{AuthData, Endpoint};
use genai::{Client, ModelSpec, ServiceTarget, ModelIden};

use super::store::StoredMessage;
use super::tools::ToolDef;

#[derive(Clone)]
pub struct AiCallConfig {
    /// 供应商适配器（由 provider_type 预设决定）
    pub adapter: AdapterKind,
    /// 供应商根地址（以 / 结尾），genai 按适配器拼接协议路径
    pub base_url: String,
    pub api_key: String,
    pub proxy: String,
    pub model_id: String,
    /// 思考等级：None=不传（模型默认）；Some("none"|"low"|"high"|"max")
    pub thinking: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ToolCallDone {
    pub call_id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StreamUsage {
    pub total_tokens: i64,
}

#[derive(Debug, Default)]
pub struct ChatStreamEvent {
    pub reasoning_delta: Option<String>,
    pub content_delta: Option<String>,
    pub tool_calls: Vec<ToolCallDone>,
    pub finished: bool,
    pub max_tokens_hit: bool,
    pub usage: Option<StreamUsage>,
}

pub struct ChatCompletionResult {
    pub text: String,
    pub max_tokens_hit: bool,
}

/// 构建带代理配置的 genai 客户端（复用旧的代理语义：空=直连，http 代理）
pub fn build_genai_client(proxy: &str) -> Result<Client, String> {
    let mut builder = reqwest13::Client::builder().connect_timeout(Duration::from_secs(30));
    if !proxy.is_empty() {
        let p = reqwest13::Proxy::all(proxy).map_err(|_| "PROXY_INVALID".to_string())?;
        builder = builder.proxy(p);
    } else {
        builder = builder.no_proxy();
    }
    let http = builder.build().map_err(|_| "PROXY_INVALID".to_string())?;
    Client::builder()
        .with_reqwest(http)
        .build()
        .map_err(|_| "PROXY_INVALID".to_string())
}

/// 供模型列表拉取等旁路 HTTP 请求使用的普通客户端（旧语义：空=直连）
pub fn build_http_client(proxy: &str) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder().connect_timeout(Duration::from_secs(30));
    if !proxy.is_empty() {
        let p = reqwest::Proxy::all(proxy).map_err(|_| "PROXY_INVALID".to_string())?;
        builder = builder.proxy(p);
    } else {
        builder = builder.no_proxy();
    }
    builder.build().map_err(|e| e.to_string())
}

fn reasoning_effort_from(level: &str) -> Option<ReasoningEffort> {
    match level {
        "none" => Some(ReasoningEffort::Zero),
        "low" => Some(ReasoningEffort::Low),
        "medium" => Some(ReasoningEffort::Medium),
        "high" => Some(ReasoningEffort::High),
        "max" => Some(ReasoningEffort::Max),
        _ => None,
    }
}

fn service_target(config: &AiCallConfig) -> ModelSpec {
    let mut base = config.base_url.trim().to_string();
    if !base.ends_with('/') {
        base.push('/');
    }
    ServiceTarget {
        endpoint: Endpoint::from_owned(base),
        auth: if config.api_key.is_empty() {
            AuthData::None
        } else {
            AuthData::from_single(config.api_key.clone())
        },
        model: ModelIden::new(config.adapter, config.model_id.clone()),
    }
    .into()
}

fn base_options(config: &AiCallConfig) -> ChatOptions {
    let mut options = ChatOptions::default()
        .with_capture_usage(true)
        .with_capture_content(true)
        .with_capture_tool_calls(true)
        .with_capture_reasoning_content(true);
    if let Some(effort) = config.thinking.as_deref().and_then(reasoning_effort_from) {
        options = options.with_reasoning_effort(effort);
    }
    options
}

fn usage_of(u: genai::chat::Usage) -> StreamUsage {
    let total = u
        .total_tokens
        .map(|t| t as i64)
        .unwrap_or(u.prompt_tokens.unwrap_or(0) as i64 + u.completion_tokens.unwrap_or(0) as i64);
    StreamUsage { total_tokens: total }
}

fn tool_call_done(tc: &ToolCall) -> ToolCallDone {
    ToolCallDone {
        call_id: tc.call_id.clone(),
        name: tc.fn_name.clone(),
        arguments: tc
            .fn_arguments
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| tc.fn_arguments.to_string()),
    }
}

fn arguments_value(args: &str) -> serde_json::Value {
    serde_json::from_str(args).unwrap_or_else(|_| serde_json::Value::String(args.to_string()))
}

fn image_part(biz_root: &Path, rel: &str) -> Option<ContentPart> {
    if rel.contains("..") || rel.starts_with('/') || rel.contains('\\') {
        return None;
    }
    let full = biz_root.join(rel);
    let bytes = std::fs::read(&full).ok()?;
    let ext = full
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "image/png",
    };
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Some(ContentPart::Binary(Binary::new(
        mime,
        genai::chat::BinarySource::Base64(Arc::from(b64.as_str())),
        None,
    )))
}

/// 工具产出的截图以视觉用户消息注入本轮上下文（模型支持视觉时由调用方判定）
pub fn tool_screenshot_message(tool_name: &str, biz_root: &Path, rel: &str) -> Option<ChatMessage> {
    let part = image_part(biz_root, rel)?;
    Some(ChatMessage::user(MessageContent::from_parts(vec![
        ContentPart::Text(format!("（工具 {tool_name} 返回的截图）")),
        part,
    ])))
}

/// 单条存储消息转换为 genai 消息（工具循环内追加上游消息用）
pub fn stored_to_chat_message(m: &StoredMessage, biz_root: &Path) -> ChatMessage {
    match m.role.as_str() {
        "assistant" => {
            let mut parts: Vec<ContentPart> = Vec::new();
            if !m.content.is_empty() {
                parts.push(ContentPart::Text(m.content.clone()));
            }
            if let Some(tc) = &m.tool_calls {
                if let Some(calls) = tc.as_array() {
                    for call in calls {
                        let function = &call["function"];
                        parts.push(ContentPart::ToolCall(ToolCall {
                            call_id: call["id"].as_str().unwrap_or_default().to_string(),
                            fn_name: function["name"].as_str().unwrap_or_default().to_string(),
                            fn_arguments: arguments_value(function["arguments"].as_str().unwrap_or_default()),
                            thought_signatures: None,
                        }));
                    }
                }
            }
            if parts.is_empty() {
                parts.push(ContentPart::Text(String::new()));
            }
            ChatMessage::assistant(MessageContent::from_parts(parts))
        }
        "tool" => {
            let response = ToolResponse {
                call_id: m.tool_call_id.clone().unwrap_or_default(),
                fn_name: m.name.clone(),
                content: m.content.clone(),
            };
            ChatMessage::tool(MessageContent::from_tool_responses(vec![response]))
        }
        _ => {
            let mut parts: Vec<ContentPart> = Vec::new();
            if !m.content.is_empty() {
                parts.push(ContentPart::Text(m.content.clone()));
            }
            for rel in &m.images {
                if let Some(part) = image_part(biz_root, rel) {
                    parts.push(part);
                }
            }
            if parts.is_empty() {
                parts.push(ContentPart::Text(String::new()));
            }
            ChatMessage::user(MessageContent::from_parts(parts))
        }
    }
}

/// 存储消息序列转换为 genai 请求（system 提升为请求级 system，compact 摘要以用户消息注入）
pub fn to_chat_request(messages: &[StoredMessage], biz_root: &Path) -> (Option<String>, Vec<ChatMessage>) {
    let mut system: Option<String> = None;
    let mut msgs: Vec<ChatMessage> = Vec::new();
    for m in messages {
        match m.role.as_str() {
            "system" => {
                let merged = match system {
                    Some(ref s) => format!("{s}\n\n{}", m.content),
                    None => m.content.clone(),
                };
                system = Some(merged);
            }
            "compact" => {
                msgs.push(ChatMessage::user(format!(
                    "{COMPACTION_PREFIX}{}{COMPACTION_SUFFIX}",
                    m.content
                )));
            }
            _ => msgs.push(stored_to_chat_message(m, biz_root)),
        }
    }
    (system, msgs)
}

const COMPACTION_PREFIX: &str = "The conversation history before this point was compacted into the following summary:\n\n<summary>\n";
const COMPACTION_SUFFIX: &str = "\n</summary>";

pub fn to_tools(defs: &[ToolDef]) -> Vec<Tool> {
    defs.iter()
        .map(|d| Tool {
            name: genai::chat::ToolName::Custom(d.name.clone()),
            description: Some(d.description.clone()),
            schema: Some(d.parameters.clone()),
            custom_format: None,
            strict: None,
            config: None,
            cache_control: None,
            eager_input_streaming: None,
        })
        .collect()
}

pub async fn stream_chat_completion(
    client: &Client,
    config: &AiCallConfig,
    req: ChatRequest,
    tools: Option<Vec<Tool>>,
) -> Result<impl Stream<Item = Result<ChatStreamEvent, String>>, String> {
    let mut req = req;
    if let Some(list) = tools {
        if !list.is_empty() {
            req = req.with_tools(list);
        }
    }
    let options = base_options(config);
    let resp = client
        .exec_chat_stream(service_target(config), req, Some(&options))
        .await
        .map_err(|_| "AI_CALL_FAILED".to_string())?;
    Ok(map_stream(resp.stream))
}

fn map_stream(resp: genai::chat::ChatStream) -> impl Stream<Item = Result<ChatStreamEvent, String>> {
    resp.filter_map(|item| async move {
        match item {
            Err(_) => Some(Err("AI_CALL_FAILED".to_string())),
            Ok(genai::chat::ChatStreamEvent::Chunk(c)) => Some(Ok(ChatStreamEvent {
                content_delta: Some(c.content),
                ..Default::default()
            })),
            Ok(genai::chat::ChatStreamEvent::ReasoningChunk(c)) => Some(Ok(ChatStreamEvent {
                reasoning_delta: Some(c.content),
                ..Default::default()
            })),
            Ok(genai::chat::ChatStreamEvent::End(end)) => {
                let tool_calls = end
                    .captured_tool_calls()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|tc| tool_call_done(tc))
                    .collect();
                Some(Ok(ChatStreamEvent {
                    finished: true,
                    max_tokens_hit: matches!(
                        &end.captured_stop_reason,
                        Some(genai::chat::StopReason::MaxTokens(_))
                    ),
                    tool_calls,
                    usage: end.captured_usage.map(usage_of),
                    ..Default::default()
                }))
            }
            Ok(_) => None,
        }
    })
}

/// 辅助任务（标题生成、上下文摘要）的聚合调用：内部同样走流式，聚合正文与结束状态
pub async fn complete_chat(
    client: &Client,
    config: &AiCallConfig,
    req: ChatRequest,
    max_tokens: Option<u32>,
) -> Result<ChatCompletionResult, String> {
    let mut options = base_options(config);
    if let Some(mt) = max_tokens {
        options = options.with_max_tokens(mt);
    }
    let resp = client
        .exec_chat_stream(service_target(config), req, Some(&options))
        .await
        .map_err(|_| "AI_CALL_FAILED".to_string())?;
    let stream = map_stream(resp.stream);
    tokio::pin!(stream);
    let mut text = String::new();
    let mut max_tokens_hit = false;
    while let Some(item) = stream.next().await {
        match item {
            Err(e) => return Err(e),
            Ok(ev) => {
                if let Some(d) = ev.content_delta {
                    text.push_str(&d);
                }
                if ev.finished {
                    max_tokens_hit = ev.max_tokens_hit;
                    break;
                }
            }
        }
    }
    Ok(ChatCompletionResult { text, max_tokens_hit })
}
