use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn valid_name(name: &str) -> bool {
    valid_id(name)
}

pub fn now_str() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub id: String,
    pub role: String,
    #[serde(default)]
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_key: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMeta {
    pub id: String,
    pub biz_type: String,
    pub biz_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    /// 上次上游调用返回的上下文 token 总量（压缩阈值判定用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_tokens: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatFile {
    #[serde(flatten)]
    pub meta: ChatMeta,
    pub messages: Vec<StoredMessage>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ChatIndex {
    #[serde(default)]
    pub chats: Vec<ChatMeta>,
}

// 存储布局：{ai_chat_path}/{功能名}/index.json + {功能名}/images/ + {功能名}/{会话id}.json
// 用户私人目录本身已隔离，按功能名（biz_type）分目录即可
fn biz_dir(root: &Path, biz_type: &str) -> Result<PathBuf, String> {
    if !valid_id(biz_type) {
        return Err("PARAM_INVALID".to_string());
    }
    Ok(root.join(biz_type))
}

fn index_path(root: &Path, biz_type: &str) -> Result<PathBuf, String> {
    Ok(biz_dir(root, biz_type)?.join("index.json"))
}

fn chat_path(root: &Path, biz_type: &str, chat_id: &str) -> Result<PathBuf, String> {
    if !valid_id(chat_id) {
        return Err("CHAT_NOT_FOUND".to_string());
    }
    Ok(biz_dir(root, biz_type)?.join(format!("{}.json", chat_id)))
}

fn write_json_atomic(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, content).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn read_index(root: &Path, biz_type: &str) -> Result<ChatIndex, String> {
    let path = index_path(root, biz_type)?;
    if !path.exists() {
        return Ok(ChatIndex::default());
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

pub fn write_index(root: &Path, biz_type: &str, index: &ChatIndex) -> Result<(), String> {
    let content = serde_json::to_string_pretty(index).map_err(|e| e.to_string())?;
    write_json_atomic(&index_path(root, biz_type)?, &content)
}

pub fn read_chat(root: &Path, biz_type: &str, chat_id: &str) -> Result<Option<ChatFile>, String> {
    let path = chat_path(root, biz_type, chat_id)?;
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

pub fn write_chat(root: &Path, biz_type: &str, chat: &ChatFile) -> Result<(), String> {
    let path = chat_path(root, biz_type, &chat.meta.id)?;
    let content = serde_json::to_string_pretty(chat).map_err(|e| e.to_string())?;
    write_json_atomic(&path, &content)
}

pub fn delete_chat(root: &Path, biz_type: &str, chat_id: &str) -> Result<bool, String> {
    let path = chat_path(root, biz_type, chat_id)?;
    if !path.exists() {
        return Ok(false);
    }
    // 删除消息引用的图片（用户上传 + 工具产出），避免会话删除后留下孤儿文件
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(chat) = serde_json::from_str::<ChatFile>(&content) {
            let biz_root = biz_dir(root, biz_type)?;
            for msg in &chat.messages {
                for rel in &msg.images {
                    if rel.contains("..") || rel.starts_with('/') || rel.contains('\\') {
                        continue;
                    }
                    let _ = fs::remove_file(biz_root.join(rel));
                }
            }
        }
    }
    fs::remove_file(&path).map_err(|e| e.to_string())?;
    Ok(true)
}

pub fn save_image(
    root: &Path,
    biz_type: &str,
    message_id: &str,
    data_url: &str,
) -> Result<String, String> {
    let (meta, encoded) = data_url
        .split_once(",")
        .ok_or("IMAGE_INVALID")
        .and_then(|(m, d)| {
            if m.starts_with("data:") && m.ends_with(";base64") {
                Ok((m, d))
            } else {
                Err("IMAGE_INVALID")
            }
        })?;
    let ext = match meta {
        "data:image/png;base64" => "png",
        "data:image/jpeg;base64" | "data:image/jpg;base64" => "jpg",
        "data:image/webp;base64" => "webp",
        "data:image/gif;base64" => "gif",
        _ => return Err("IMAGE_INVALID".to_string()),
    };
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "IMAGE_INVALID".to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("IMAGE_TOO_LARGE".to_string());
    }
    let dir = biz_dir(root, biz_type)?.join("images");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut index = 0;
    loop {
        let filename = format!("{}_{}.{}", message_id, index, ext);
        let target = dir.join(&filename);
        if !target.exists() {
            fs::write(&target, &bytes).map_err(|e| e.to_string())?;
            return Ok(format!("images/{}", filename));
        }
        index += 1;
    }
}

const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];

pub fn read_image(root: &Path, biz_type: &str, relative: &str) -> Result<(Vec<u8>, String), String> {
    if relative.contains("..") || relative.starts_with('/') || relative.contains('\\') {
        return Err("PATH_INVALID".to_string());
    }
    let ext = relative
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if !IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        return Err("PATH_INVALID".to_string());
    }
    let biz_root = biz_dir(root, biz_type)?;
    let full = biz_root.join(relative);
    let canonical_root = fs::canonicalize(&biz_root).unwrap_or_else(|_| biz_root.clone());
    let canonical = fs::canonicalize(&full).map_err(|_| "PATH_INVALID".to_string())?;
    if !canonical.starts_with(&canonical_root) {
        return Err("PATH_INVALID".to_string());
    }
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    };
    let bytes = fs::read(&canonical).map_err(|e| e.to_string())?;
    Ok((bytes, mime.to_string()))
}
