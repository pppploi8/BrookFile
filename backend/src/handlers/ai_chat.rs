use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use actix_web::{web, HttpRequest, HttpResponse, Responder};
use futures_util::StreamExt;
use genai::chat::{ChatMessage, ChatRequest};
use serde::{Deserialize, Serialize};

use crate::ai::{self, AiToolContext, ChatFile, ChatIndex, ChatMeta, StoredMessage, ToolCallDone};
use crate::app_state::AppState;
use crate::error_logger;
use crate::handlers::{
    get_current_user_id, get_user_root_path, internal_error_response, ApiResponse,
};

/// 主流程结束通知的 drop guard：外层任务任意退出路径都会触发，标题任务据此开始落盘
struct MainDoneGuard(Arc<tokio::sync::Notify>);

impl Drop for MainDoneGuard {
    fn drop(&mut self) {
        self.0.notify_one();
    }
}

/// 用同一模型（最低思考等级）为用户首轮提问生成会话标题
async fn generate_chat_title(
    client: &genai::Client,
    config: &ai::AiCallConfig,
    system_prompt: Option<&str>,
    question: &str,
) -> Result<String, String> {
    let mut req = ChatRequest::new(vec![ChatMessage::user(format!(
        "请根据以下用户提问生成一个不超过20字的会话标题，直接输出标题文本，不要引号、句号或任何解释：
{}",
        question
    ))]);
    if let Some(sys) = system_prompt {
        req = req.with_system(sys.to_string());
    }
    let title_config = ai::AiCallConfig {
        thinking: Some("none".to_string()),
        ..config.clone()
    };
    let result = ai::client::complete_chat(client, &title_config, req, None).await?;
    let title = result
        .text
        .trim()
        .trim_matches(|c| c == '"' || c == '《' || c == '》' || c == '。')
        .to_string();
    if title.is_empty() {
        return Err("空标题".to_string());
    }
    Ok(title.chars().take(50).collect())
}

fn fail(fail_code: &str) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse {
        success: false,
        fail_code: Some(fail_code.to_string()),
    })
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn valid_biz_type(biz_type: &str) -> bool {
    !biz_type.is_empty()
        && biz_type
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && biz_type.len() <= 64
}

fn resolve_ai_dir(app_state: &AppState, user_id: &str, root_path: &str) -> Result<PathBuf, String> {
    let rel = app_state
        .user_model
        .get_user_full(user_id)
        .map_err(|e| e.to_string())?
        .and_then(|u| u.ai_chat_path)
        .filter(|p| !p.is_empty())
        .ok_or("AI_CHAT_PATH_NOT_SET")?;
    Ok(Path::new(root_path).join(rel))
}

fn refresh_index(dir: &Path, biz_type: &str, meta: &ChatMeta) -> Result<(), String> {
    let mut index: ChatIndex = ai::store::read_index(dir, biz_type)?;
    match index.chats.iter_mut().find(|c| c.id == meta.id) {
        Some(entry) => *entry = meta.clone(),
        None => index.chats.push(meta.clone()),
    }
    ai::store::write_index(dir, biz_type, &index)
}

#[derive(Debug, Deserialize)]
pub struct ChatListRequest {
    pub biz_type: String,
    pub biz_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatListItem {
    pub id: String,
    pub biz_type: String,
    pub biz_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct ChatListResponse {
    pub success: bool,
    pub chats: Vec<ChatListItem>,
}

pub async fn ai_chat_list(
    body: web::Json<ChatListRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };
    if !valid_biz_type(&body.biz_type) {
        return fail("PARAM_INVALID");
    }
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(p) => p,
        Err(response) => return response,
    };
    let dir = match resolve_ai_dir(&app_state, &user_id, &root_path) {
        Ok(d) => d,
        Err(code) => return fail(&code),
    };
    if !ai::store::valid_name(body.biz_type.trim()) {
        return fail("PARAM_INVALID");
    }

    let index = match ai::store::read_index(&dir, &body.biz_type) {
        Ok(i) => i,
        Err(e) => {
            error_logger::log_error("/api/ai/chat/list", &e);
            return internal_error_response("/api/ai/chat/list", &e);
        }
    };

    let biz_id = body.biz_id.as_deref().unwrap_or("");
    let mut chats: Vec<ChatListItem> = index
        .chats
        .into_iter()
        .filter(|c| c.biz_type == body.biz_type && c.biz_id == biz_id)
        .map(|c| ChatListItem {
            id: c.id,
            biz_type: c.biz_type,
            biz_id: c.biz_id,
            title: c.title,
            created_at: c.created_at,
            updated_at: c.updated_at,
        })
        .collect();
    chats.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    HttpResponse::Ok().json(ChatListResponse {
        success: true,
        chats,
    })
}

#[derive(Debug, Deserialize)]
pub struct ChatCreateRequest {
    pub biz_type: String,
    pub biz_id: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatCreateResponse {
    pub success: bool,
    pub chat_id: String,
}

pub async fn ai_chat_create(
    body: web::Json<ChatCreateRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };
    if !valid_biz_type(&body.biz_type) {
        return fail("PARAM_INVALID");
    }
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(p) => p,
        Err(response) => return response,
    };
    let dir = match resolve_ai_dir(&app_state, &user_id, &root_path) {
        Ok(d) => d,
        Err(code) => return fail(&code),
    };
    if !ai::store::valid_name(body.biz_type.trim()) {
        return fail("PARAM_INVALID");
    }

    let now = ai::store::now_str();
    let meta = ChatMeta {
        id: new_id(),
        biz_type: body.biz_type.trim().to_string(),
        biz_id: body.biz_id.as_deref().unwrap_or("").to_string(),
        title: body.title.as_deref().unwrap_or("").trim().to_string(),
        created_at: now.clone(),
        updated_at: now,
        context_tokens: None,
    };
    let chat = ChatFile {
        meta: meta.clone(),
        messages: Vec::new(),
    };

    if let Err(e) = ai::store::write_chat(&dir, &meta.biz_type, &chat)
        .and_then(|_| refresh_index(&dir, &meta.biz_type, &meta))
    {
        error_logger::log_error("/api/ai/chat/create", &e);
        return internal_error_response("/api/ai/chat/create", &e);
    }

    HttpResponse::Ok().json(ChatCreateResponse {
        success: true,
        chat_id: meta.id,
    })
}

#[derive(Debug, Deserialize)]
pub struct ChatRenameRequest {
    pub biz_type: String,
    pub chat_id: String,
    pub title: String,
}

pub async fn ai_chat_rename(
    body: web::Json<ChatRenameRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(p) => p,
        Err(response) => return response,
    };
    let dir = match resolve_ai_dir(&app_state, &user_id, &root_path) {
        Ok(d) => d,
        Err(code) => return fail(&code),
    };
    if !ai::store::valid_name(body.biz_type.trim()) {
        return fail("PARAM_INVALID");
    }

    let mut chat = match ai::store::read_chat(&dir, &body.biz_type, &body.chat_id) {
        Ok(Some(c)) => c,
        Ok(None) => return fail("CHAT_NOT_FOUND"),
        Err(e) => {
            error_logger::log_error("/api/ai/chat/rename", &e);
            return internal_error_response("/api/ai/chat/rename", &e);
        }
    };

    chat.meta.title = body.title.trim().to_string();
    chat.meta.updated_at = ai::store::now_str();
    if let Err(e) = ai::store::write_chat(&dir, &body.biz_type, &chat)
        .and_then(|_| refresh_index(&dir, &body.biz_type, &chat.meta))
    {
        error_logger::log_error("/api/ai/chat/rename", &e);
        return internal_error_response("/api/ai/chat/rename", &e);
    }

    HttpResponse::Ok().json(ApiResponse {
        success: true,
        fail_code: None,
    })
}

#[derive(Debug, Deserialize)]
pub struct ChatIdRequest {
    pub biz_type: String,
    pub chat_id: String,
}

pub async fn ai_chat_delete(
    body: web::Json<ChatIdRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(p) => p,
        Err(response) => return response,
    };
    let dir = match resolve_ai_dir(&app_state, &user_id, &root_path) {
        Ok(d) => d,
        Err(code) => return fail(&code),
    };
    if !ai::store::valid_name(body.biz_type.trim()) {
        return fail("PARAM_INVALID");
    }

    match ai::store::delete_chat(&dir, &body.biz_type, &body.chat_id) {
        Ok(_) => {}
        Err(e) => {
            error_logger::log_error("/api/ai/chat/delete", &e);
            return internal_error_response("/api/ai/chat/delete", &e);
        }
    }

    match ai::store::read_index(&dir, &body.biz_type) {
        Ok(mut index) => {
            index.chats.retain(|c| c.id != body.chat_id);
            if let Err(e) = ai::store::write_index(&dir, &body.biz_type, &index) {
                error_logger::log_error("/api/ai/chat/delete", &e);
                return internal_error_response("/api/ai/chat/delete", &e);
            }
        }
        Err(e) => {
            error_logger::log_error("/api/ai/chat/delete", &e);
            return internal_error_response("/api/ai/chat/delete", &e);
        }
    }

    HttpResponse::Ok().json(ApiResponse {
        success: true,
        fail_code: None,
    })
}

#[derive(Debug, Deserialize)]
pub struct ChatMessagesRequest {
    pub biz_type: String,
    pub chat_id: String,
    pub before_id: Option<String>,
    pub after_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ChatMessageItem {
    pub id: String,
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    pub images: Vec<String>,
    pub tool_calls: Option<serde_json::Value>,
    pub tool_call_id: Option<String>,
    pub name: Option<String>,
    pub model_key: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ChatMessagesResponse {
    pub success: bool,
    pub messages: Vec<ChatMessageItem>,
    pub has_more_before: bool,
    pub has_more_after: bool,
}

fn to_message_item(m: &StoredMessage) -> ChatMessageItem {
    ChatMessageItem {
        id: m.id.clone(),
        role: m.role.clone(),
        content: m.content.clone(),
        reasoning: m.reasoning.clone(),
        images: m.images.clone(),
        tool_calls: m.tool_calls.clone(),
        tool_call_id: m.tool_call_id.clone(),
        name: m.name.clone(),
        model_key: m.model_key.clone(),
        created_at: m.created_at.clone(),
    }
}

pub async fn ai_chat_messages(
    body: web::Json<ChatMessagesRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(p) => p,
        Err(response) => return response,
    };
    let dir = match resolve_ai_dir(&app_state, &user_id, &root_path) {
        Ok(d) => d,
        Err(code) => return fail(&code),
    };
    if !ai::store::valid_name(body.biz_type.trim()) {
        return fail("PARAM_INVALID");
    }

    let chat = match ai::store::read_chat(&dir, &body.biz_type, &body.chat_id) {
        Ok(Some(c)) => c,
        Ok(None) => return fail("CHAT_NOT_FOUND"),
        Err(e) => {
            error_logger::log_error("/api/ai/chat/messages", &e);
            return internal_error_response("/api/ai/chat/messages", &e);
        }
    };

    let limit = body.limit.unwrap_or(100).clamp(1, 500);
    let messages = &chat.messages;
    let total = messages.len();

    let (start, end) = if let Some(id) = &body.before_id {
        match messages.iter().position(|m| &m.id == id) {
            Some(pos) => {
                let start = pos.saturating_sub(limit);
                (start, pos)
            }
            None => return fail("MESSAGE_NOT_FOUND"),
        }
    } else if let Some(id) = &body.after_id {
        match messages.iter().position(|m| &m.id == id) {
            Some(pos) => {
                let end = (pos + 1 + limit).min(total);
                (pos + 1, end)
            }
            None => return fail("MESSAGE_NOT_FOUND"),
        }
    } else {
        (total.saturating_sub(limit), total)
    };

    let slice: Vec<ChatMessageItem> = messages[start..end].iter().map(to_message_item).collect();

    HttpResponse::Ok().json(ChatMessagesResponse {
        success: true,
        messages: slice,
        has_more_before: start > 0,
        has_more_after: end < total,
    })
}

#[derive(Debug, Deserialize)]
pub struct ChatSendRequest {
    pub biz_type: String,
    pub chat_id: String,
    pub model_key: String,
    pub content: String,
    pub images: Option<Vec<String>>,
    pub thinking: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChatImageQuery {
    pub biz_type: String,
    pub path: String,
}

pub async fn ai_chat_image(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
    query: web::Query<ChatImageQuery>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(p) => p,
        Err(response) => return response,
    };
    let dir = match resolve_ai_dir(&app_state, &user_id, &root_path) {
        Ok(d) => d,
        Err(code) => return fail(&code),
    };
    if !ai::store::valid_name(query.biz_type.trim()) {
        return fail("PARAM_INVALID");
    }

    match ai::store::read_image(&dir, &query.biz_type, &query.path) {
        Ok((bytes, mime)) => HttpResponse::Ok().content_type(mime).body(bytes),
        Err(code) => fail(&code),
    }
}

#[derive(Debug, Deserialize)]
pub struct PageQueryResultRequest {
    pub request_id: String,
    /// 页面应答内容，格式由 page_query 的 kind 定义；空串表示应答失败
    pub data: String,
}

/// 页面查询应答回传：交付给等待中的工具（见 ai::tools::PageQueryBridge）。
pub async fn ai_chat_page_query_result(
    body: web::Json<PageQueryResultRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    if let Err(resp) = get_current_user_id(&http_req, &app_state) {
        return resp;
    }
    if body.request_id.is_empty() {
        return fail("PARAM_INVALID");
    }
    let delivered = ai::tools::resolve_page_query(
        &app_state.ai_page_query_pending,
        &body.request_id,
        body.data.clone(),
    )
    .await;
    if delivered {
        HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        })
    } else {
        fail("PAGE_QUERY_NOT_FOUND")
    }
}

fn sse_frame(event: &str, data: &serde_json::Value) -> web::Bytes {
    web::Bytes::from(format!("event: {}\ndata: {}\n\n", event, data))
}

pub async fn ai_chat_send(
    body: web::Json<ChatSendRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };
    if body.content.trim().is_empty()
        && body.images.as_ref().map_or(true, |v| v.is_empty())
    {
        return fail("PARAM_INVALID");
    }
    if body.images.as_ref().map_or(false, |v| v.len() > 4) {
        return fail("PARAM_INVALID");
    }
    if let Some(level) = &body.thinking {
        if !matches!(level.as_str(), "none" | "low" | "high" | "max") {
            return fail("PARAM_INVALID");
        }
    }
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(p) => p,
        Err(response) => return response,
    };
    let dir = match resolve_ai_dir(&app_state, &user_id, &root_path) {
        Ok(d) => d,
        Err(code) => return fail(&code),
    };
    if !ai::store::valid_name(body.biz_type.trim()) {
        return fail("PARAM_INVALID");
    }

    let mut chat = match ai::store::read_chat(&dir, &body.biz_type, &body.chat_id) {
        Ok(Some(c)) => c,
        Ok(None) => return fail("CHAT_NOT_FOUND"),
        Err(e) => {
            error_logger::log_error("/api/ai/chat/send", &e);
            return internal_error_response("/api/ai/chat/send", &e);
        }
    };

    let first_round = chat.messages.is_empty();
    let model = match app_state.ai_model_model.get_by_id(&body.model_key) {
        Ok(Some(m)) => m,
        Ok(None) => return fail("MODEL_NOT_FOUND"),
        Err(e) => {
            error_logger::log_error("/api/ai/chat/send", &e);
            return internal_error_response("/api/ai/chat/send", &e);
        }
    };
    let provider_cfg = match app_state.ai_provider_model.get_by_id(&model.provider_id) {
        Ok(Some(p)) => p,
        Ok(None) => return fail("MODEL_NOT_FOUND"),
        Err(e) => {
            error_logger::log_error("/api/ai/chat/send", &e);
            return internal_error_response("/api/ai/chat/send", &e);
        }
    };
    let preset = match crate::ai::preset_by_type(&provider_cfg.provider_type) {
        Some(p) => p,
        None => return fail("PROVIDER_TYPE_INVALID"),
    };
    // 兼容旧配置：base_url 曾存完整 /responses 地址，剥掉后缀还原为根地址
    let legacy_base = provider_cfg
        .base_url
        .trim()
        .trim_end_matches('/')
        .trim_end_matches("/responses")
        .to_string();
    let config = ai::AiCallConfig {
        adapter: preset.adapter,
        base_url: if legacy_base.is_empty() {
            preset.default_base_url.to_string()
        } else {
            legacy_base
        },
        api_key: provider_cfg.api_key.clone(),
        proxy: provider_cfg.proxy.clone(),
        model_id: model.model_id.clone(),
        thinking: body.thinking.clone(),
    };
    let client = match ai::client::build_genai_client(&config.proxy) {
        Ok(c) => c,
        Err(_) => return fail("PROXY_INVALID"),
    };

    let tool_provider = app_state.ai_tools.get(&chat.meta.biz_type);
    let tool_defs: Vec<ai::ToolDef> = tool_provider
        .as_ref()
        .map(|p| p.tool_definitions())
        .unwrap_or_default();
    let max_rounds = tool_provider.as_ref().map_or(0, |p| p.max_tool_rounds());
    let model_vision = model.supports_vision;
    let context_window = model.context_length;
    let max_output_tokens = model.max_output_tokens;
    let page_query_pending = app_state.ai_page_query_pending.clone();

    let mut image_paths: Vec<String> = Vec::new();
    for data_url in body.images.iter().flatten() {
        match ai::store::save_image(&dir, &body.biz_type, &body.chat_id, data_url) {
            Ok(rel) => image_paths.push(rel),
            Err(e) if e == "IMAGE_INVALID" || e == "IMAGE_TOO_LARGE" => return fail(&e),
            Err(e) => {
                error_logger::log_error("/api/ai/chat/send", &e);
                return internal_error_response("/api/ai/chat/send", &e);
            }
        }
    }

    let now = ai::store::now_str();
    let title_was_empty = chat.meta.title.is_empty();
    // 首轮注入：业务系统提示落盘为会话首条消息，此后作为历史原样重放、不再变更
    if chat.messages.is_empty() {
        let prefix_ctx = AiToolContext {
            user_id: user_id.clone(),
            biz_type: chat.meta.biz_type.clone(),
            biz_id: chat.meta.biz_id.clone(),
            root_path: root_path.clone(),
            chat_store_dir: None,
            chat_id: chat.meta.id.clone(),
            page_query: None,
        };
        if let Some(prefix) = tool_provider.as_ref().and_then(|p| p.context_prefix(&prefix_ctx)) {
            chat.messages.push(StoredMessage {
                id: new_id(),
                role: "system".to_string(),
                content: prefix,
                reasoning: None,
                images: Vec::new(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
                model_key: None,
                created_at: now.clone(),
            });
        }
    }
    let user_msg = StoredMessage {
        id: new_id(),
        role: "user".to_string(),
        content: body.content.trim().to_string(),
        reasoning: None,
        images: image_paths,
        tool_calls: None,
        tool_call_id: None,
        name: None,
        model_key: None,
        created_at: now.clone(),
    };
    if chat.meta.title.is_empty() {
        chat.meta.title = user_msg.content.chars().take(50).collect();
    }
    let fallback_title = chat.meta.title.clone();
    chat.meta.updated_at = now;
    chat.messages.push(user_msg.clone());
    if let Err(e) = ai::store::write_chat(&dir, &body.biz_type, &chat)
        .and_then(|_| refresh_index(&dir, &body.biz_type, &chat.meta))
    {
        error_logger::log_error("/api/ai/chat/send", &e);
        return internal_error_response("/api/ai/chat/send", &e);
    }

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<web::Bytes, actix_web::Error>>(64);
    let model_key = body.model_key.clone();
    let lock = app_state.ai_chat_lock.clone();

    // 首轮并行生成会话标题：同一模型、最低思考等级（none）。
    // 生成结果等主流程结束后再落盘（用最新文件状态 + 仅覆盖兜底标题），避免与主流程互相覆盖；
    // 生成失败/超时不写，保留提问前 50 字的兜底标题；用户已手动改名则跳过。
    let title_notify = Arc::new(tokio::sync::Notify::new());
    if first_round && title_was_empty {
        let notify = title_notify.clone();
        let gen_client = client.clone();
        let gen_config = config.clone();
        let gen_dir = dir.clone();
        let gen_biz_type = body.biz_type.clone();
        let gen_chat_id = body.chat_id.clone();
        let gen_lock = lock.clone();
        let gen_fallback = fallback_title.clone();
        let gen_system = chat.messages.iter().find(|m| m.role == "system").map(|m| m.content.clone());
        let gen_question = body.content.trim().to_string();
        let _title_task = actix_web::rt::spawn(async move {
            let generated = match tokio::time::timeout(
                Duration::from_secs(30),
                generate_chat_title(&gen_client, &gen_config, gen_system.as_deref(), &gen_question),
            )
            .await
            {
                Ok(Ok(t)) => t,
                Ok(Err(e)) => {
                    error_logger::log_error("title_gen", &format!("生成失败: {}", e));
                    return;
                }
                Err(_) => {
                    error_logger::log_error("title_gen", "生成超时");
                    return;
                }
            };
            notify.notified().await;
            let _guard = gen_lock.lock().await;
            match ai::store::read_chat(&gen_dir, &gen_biz_type, &gen_chat_id) {
                Ok(Some(mut chat)) => {
                    if chat.meta.title != gen_fallback {
                        error_logger::log_error("title_gen", "标题已被修改，跳过");
                        return;
                    }
                    chat.meta.title = generated;
                    chat.meta.updated_at = ai::store::now_str();
                    if let Err(e) = ai::store::write_chat(&gen_dir, &gen_biz_type, &chat)
                        .and_then(|_| refresh_index(&gen_dir, &gen_biz_type, &chat.meta))
                    {
                        error_logger::log_error("title_gen", &e);
                    }
                }
                Ok(None) => error_logger::log_error("title_gen", "会话不存在"),
                Err(e) => error_logger::log_error("title_gen", &e),
            }
        });
    }

    actix_web::rt::spawn(async move {
        // 任意退出路径（含断连/出错 return）都触发通知，标题任务据此开始落盘
        let _main_done = MainDoneGuard(title_notify);
        // 写锁只覆盖"读改写会话文件"的临界区，不覆盖流式调用全程，
        // 否则客户端断连后任务滞留会一直占锁，阻塞后续所有发送。
        async fn persist(
            lock: &tokio::sync::Mutex<()>,
            dir: &Path,
            biz_type: &str,
            chat: &ChatFile,
        ) -> Result<(), String> {
            let _guard = lock.lock().await;
            ai::store::write_chat(dir, biz_type, chat)
                .and_then(|_| refresh_index(dir, biz_type, &chat.meta))
        }

        let mut disconnected = false;
        async fn emit(
            tx: &tokio::sync::mpsc::Sender<Result<web::Bytes, actix_web::Error>>,
            disconnected: &mut bool,
            event: &str,
            data: &serde_json::Value,
        ) {
            if *disconnected {
                return;
            }
            let frame = sse_frame(event, data);
            match tokio::time::timeout(Duration::from_millis(500), tx.send(Ok(frame))).await {
                Ok(Ok(())) => {}
                _ => *disconnected = true,
            }
        }

        let user_event = serde_json::to_value(to_message_item(&user_msg))
            .unwrap_or(serde_json::json!({}));
        emit(&tx, &mut disconnected, "user_message", &user_event).await;

        // 页面查询桥：工具可向页面查询数据（如 PDF 页码/截图），应答经 page_query_result 接口回传。
        // 查询帧走独立通道，由泵任务转发到 SSE 主通道（actix Error 非 Send，不能直接跨 trait 持有）。
        let (query_tx, mut query_rx) = tokio::sync::mpsc::channel::<web::Bytes>(32);
        let bridge = std::sync::Arc::new(ai::tools::PageQueryBridge::new(
            query_tx,
            page_query_pending,
        ));
        let tx_pump = tx.clone();
        actix_web::rt::spawn(async move {
            while let Some(frame) = query_rx.recv().await {
                if tokio::time::timeout(Duration::from_millis(500), tx_pump.send(Ok(frame)))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });
        let ctx = AiToolContext {
            user_id: user_id.clone(),
            biz_type: chat.meta.biz_type.clone(),
            biz_id: chat.meta.biz_id.clone(),
            root_path: root_path.clone(),
            chat_store_dir: Some(dir.clone()),
            chat_id: chat.meta.id.clone(),
            page_query: Some(bridge),
        };

        let biz_root = dir.join(&chat.meta.biz_type);

        // —— 上下文自动压缩（pi 逻辑）：构建本轮请求前判定并执行 ——
        let tokens_before = ai::compaction::context_token_estimate(chat.meta.context_tokens, &chat.messages);
        if ai::compaction::should_compact(tokens_before, context_window)
            && ai::compaction::prepare(&chat.messages).is_some()
        {
            emit(
                &tx,
                &mut disconnected,
                "compact_start",
                &serde_json::json!({ "tokens_before": tokens_before }),
            )
            .await;
            match ai::compaction::run_compaction(&client, &config, max_output_tokens, &chat.messages).await {
                Ok(Some((summary, prep))) => {
                    let applied =
                        ai::compaction::apply(chat.messages, &prep, summary, new_id(), &ai::store::now_str());
                    for rel in &applied.orphan_images {
                        if !rel.contains("..") && !rel.starts_with('/') && !rel.contains('\\') {
                            let _ = std::fs::remove_file(biz_root.join(rel));
                        }
                    }
                    chat.messages = applied.messages;
                    chat.meta.context_tokens = None;
                    chat.meta.updated_at = ai::store::now_str();
                    match persist(&lock, &dir, &chat.meta.biz_type.as_str(), &chat).await {
                        Ok(()) => {
                            emit(
                                &tx,
                                &mut disconnected,
                                "compact_end",
                                &serde_json::json!({ "ok": true, "tokens_before": tokens_before }),
                            )
                            .await;
                        }
                        Err(e) => {
                            error_logger::log_error("/api/ai/chat/send", &e);
                            emit(
                                &tx,
                                &mut disconnected,
                                "compact_end",
                                &serde_json::json!({ "ok": false }),
                            )
                            .await;
                        }
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    error_logger::log_error("context_compaction", &e);
                    emit(
                        &tx,
                        &mut disconnected,
                        "compact_end",
                        &serde_json::json!({ "ok": false }),
                    )
                    .await;
                }
            }
        }

        let (system, mut history) = ai::client::to_chat_request(&chat.messages, &biz_root);
        let tools_genai = ai::client::to_tools(&tool_defs);
        let mut rounds = 0u32;
        // 各轮思维链合并为一条，只随最终 assistant 消息落盘（前端呈现为单张思考卡片）
        let mut reasoning_all = String::new();
        while rounds < max_rounds.max(1) && !disconnected {
            rounds += 1;
            let mut req = ChatRequest::new(history.clone());
            if let Some(sys) = &system {
                req = req.with_system(sys.clone());
            }
            let stream = match ai::client::stream_chat_completion(
                &client,
                &config,
                req,
                if max_rounds > 0 { Some(tools_genai.clone()) } else { None },
            )
            .await
            {
                Ok(s) => s,
                Err(code) => {
                    emit(&tx, &mut disconnected, "error", &serde_json::json!({ "fail_code": code })).await;
                    break;
                }
            };

            let mut content_acc = String::new();
            let mut reasoning_acc = String::new();
            let mut tools_acc: Vec<ToolCallDone> = Vec::new();
            tokio::pin!(stream);
            let mut stream_ok = true;
            loop {
                let next = tokio::time::timeout(Duration::from_secs(120), stream.next()).await;
                match next {
                    Err(_) => {
                        emit(&tx, &mut disconnected, "error", &serde_json::json!({ "fail_code": "AI_TIMEOUT" })).await;
                        stream_ok = false;
                        break;
                    }
                    Ok(None) => break,
                    Ok(Some(Err(_))) => {
                        emit(&tx, &mut disconnected, "error", &serde_json::json!({ "fail_code": "AI_CALL_FAILED" })).await;
                        stream_ok = false;
                        break;
                    }
                    Ok(Some(Ok(event))) => {
                        if let Some(delta) = event.reasoning_delta {
                            reasoning_acc.push_str(&delta);
                            emit(&tx, &mut disconnected, "reasoning", &serde_json::json!({ "content": delta })).await;
                        }
                        if let Some(delta) = event.content_delta {
                            content_acc.push_str(&delta);
                            emit(&tx, &mut disconnected, "delta", &serde_json::json!({ "content": delta })).await;
                        }
                        tools_acc.extend(event.tool_calls);
                        if let Some(usage) = event.usage {
                            chat.meta.context_tokens = Some(usage.total_tokens);
                        }
                        if event.finished {
                            break;
                        }
                    }
                }
                if disconnected {
                    break;
                }
            }
            drop(stream);

            if !reasoning_acc.is_empty() {
                if !reasoning_all.is_empty() {
                    reasoning_all.push_str("\n\n");
                }
                reasoning_all.push_str(&reasoning_acc);
            }

            if disconnected || !stream_ok {
                // 客户端中断或上游出错：本轮已生成内容照常落盘（可能为空则跳过）
                if !content_acc.is_empty() || !reasoning_acc.is_empty() {
                    let partial = StoredMessage {
                        id: new_id(),
                        role: "assistant".to_string(),
                        content: content_acc,
                        reasoning: Some(reasoning_all.clone()).filter(|s| !s.is_empty()),
                        images: Vec::new(),
                        tool_calls: None,
                        tool_call_id: None,
                        name: None,
                        model_key: Some(model_key.clone()),
                        created_at: ai::store::now_str(),
                    };
                    chat.messages.push(partial);
                    chat.meta.updated_at = ai::store::now_str();
                    let _ = persist(&lock, &dir, &chat.meta.biz_type.as_str(), &chat).await;
                }
                return;
            }

            if !tools_acc.is_empty() && tool_provider.is_none() {
                emit(&tx, &mut disconnected, "error", &serde_json::json!({ "fail_code": "TOOL_NOT_AVAILABLE" })).await;
                return;
            }

            if tools_acc.is_empty() {
                let assistant = StoredMessage {
                    id: new_id(),
                    role: "assistant".to_string(),
                    content: content_acc,
                    reasoning: Some(reasoning_all).filter(|s| !s.is_empty()),
                    images: Vec::new(),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                    model_key: Some(model_key.clone()),
                    created_at: ai::store::now_str(),
                };
                let done_event = serde_json::json!({ "message_id": assistant.id });
                chat.messages.push(assistant);
                chat.meta.updated_at = ai::store::now_str();
                if let Err(e) = persist(&lock, &dir, &chat.meta.biz_type.as_str(), &chat).await {
                    error_logger::log_error("/api/ai/chat/send", &e);
                    emit(&tx, &mut disconnected, "error", &serde_json::json!({ "fail_code": "INTERNAL_ERROR" })).await;
                    return;
                }
                emit(&tx, &mut disconnected, "done", &done_event).await;
                return;
            }

            let tool_calls_json: Vec<serde_json::Value> = tools_acc
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "id": t.call_id,
                        "type": "function",
                        "function": { "name": t.name, "arguments": t.arguments }
                    })
                })
                .collect();

            let assistant = StoredMessage {
                id: new_id(),
                role: "assistant".to_string(),
                content: content_acc.clone(),
                reasoning: None,
                images: Vec::new(),
                tool_calls: Some(serde_json::Value::Array(tool_calls_json.clone())),
                tool_call_id: None,
                name: None,
                model_key: Some(model_key.clone()),
                created_at: ai::store::now_str(),
            };
            chat.messages.push(assistant.clone());
            history.push(ai::client::stored_to_chat_message(&assistant, &biz_root));

            if let Some(provider) = &tool_provider {
                for t in &tools_acc {
                    emit(&tx, &mut disconnected, "tool_start", &serde_json::json!({ "name": t.name, "arguments": t.arguments })).await;
                    let exec = futures_util::FutureExt::catch_unwind(
                        std::panic::AssertUnwindSafe(provider.execute(&ctx, &t.name, &t.arguments)),
                    )
                    .await;
                    let (ok, tool_res) = match exec {
                        Ok(Ok(r)) => (true, r),
                        Ok(Err(e)) => (
                            false,
                            ai::tools::ToolResult::text(format!("工具执行失败: {}", e)),
                        ),
                        Err(p) => {
                            let msg = p
                                .downcast_ref::<&str>()
                                .map(|s| s.to_string())
                                .or_else(|| p.downcast_ref::<String>().cloned())
                                .unwrap_or_else(|| "unknown panic".to_string());
                            error_logger::log_error("/api/ai/chat/send", &format!("tool panic: {}", msg));
                            (
                                false,
                                ai::tools::ToolResult::text(format!("工具执行失败: {}", msg)),
                            )
                        }
                    };
                    emit(&tx, &mut disconnected, "tool_end", &serde_json::json!({ "name": t.name, "ok": ok })).await;
                    let tool_msg = StoredMessage {
                        id: new_id(),
                        role: "tool".to_string(),
                        content: tool_res.content.clone(),
                        reasoning: None,
                        images: tool_res.images.clone(),
                        tool_calls: None,
                        tool_call_id: Some(t.call_id.clone()),
                        name: Some(t.name.clone()),
                        model_key: None,
                        created_at: ai::store::now_str(),
                    };
                    chat.messages.push(tool_msg.clone());
                    history.push(ai::client::stored_to_chat_message(&tool_msg, &biz_root));
                    // 工具产出的截图以视觉消息注入本轮上下文（模型支持视觉时）
                    if model_vision {
                        for rel in &tool_msg.images {
                            if let Some(msg) = ai::client::tool_screenshot_message(&t.name, &biz_root, rel) {
                                history.push(msg);
                            }
                        }
                    }
                    if disconnected {
                        break;
                    }
                }
            }

            chat.meta.updated_at = ai::store::now_str();
            if let Err(e) = persist(&lock, &dir, &chat.meta.biz_type.as_str(), &chat).await {
                error_logger::log_error("/api/ai/chat/send", &e);
                emit(&tx, &mut disconnected, "error", &serde_json::json!({ "fail_code": "INTERNAL_ERROR" })).await;
                return;
            }
        }

        if !disconnected && rounds >= max_rounds.max(1) {
            emit(&tx, &mut disconnected, "error", &serde_json::json!({ "fail_code": "TOOL_ROUNDS_EXCEEDED" })).await;
        }
    });

    HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(futures_util::stream::unfold(rx, |mut rx| async move {
            rx.recv().await.map(|item| (item, rx))
        }))
}
