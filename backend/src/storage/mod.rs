pub mod encrypt;
pub mod webdav;

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;

pub struct FileInfo {
    pub path: String,
    pub is_dir: bool,
}

#[derive(Debug)]
pub enum StorageError {
    ConnectionError(String),
    AuthError(String),
    Other(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            StorageError::AuthError(msg) => write!(f, "Auth error: {}", msg),
            StorageError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for StorageError {}

pub type ProgressCallback = Arc<dyn Fn(u64, u64) + Send + Sync>;

#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn list_files(&self, path: &str) -> Result<Vec<FileInfo>, StorageError>;
    async fn upload_stream(
        &self,
        remote_path: &str,
        total_size: u64,
        data_receiver: Receiver<Vec<u8>>,
        progress: ProgressCallback,
    ) -> Result<(), StorageError>;
    async fn download_file(&self, remote_path: &str) -> Result<Vec<u8>, StorageError>;
    async fn download_stream(
        &self,
        remote_path: &str,
        data_sender: tokio::sync::mpsc::Sender<Vec<u8>>,
        progress: ProgressCallback,
    ) -> Result<u64, StorageError>;
    async fn delete_file(&self, remote_path: &str) -> Result<(), StorageError>;
    async fn move_file(&self, from: &str, to: &str) -> Result<(), StorageError>;
    async fn mkdir(&self, path: &str) -> Result<(), StorageError>;
}

pub fn create_backend(storage_type: &str, config: &serde_json::Value) -> Result<Box<dyn StorageBackend>, String> {
    match storage_type {
        "webdav" => {
            let address = config.get("address").and_then(|v| v.as_str()).ok_or("INVALID_STORAGE_CONFIG")?.to_string();
            let username = config.get("username").and_then(|v| v.as_str()).ok_or("INVALID_STORAGE_CONFIG")?.to_string();
            let password = config.get("password").and_then(|v| v.as_str()).ok_or("INVALID_STORAGE_CONFIG")?.to_string();
            let path = config.get("path").and_then(|v| v.as_str()).unwrap_or("/").to_string();
            Ok(Box::new(webdav::WebDAVBackend::new(address, username, password, path)))
        }
        _ => Err("UNSUPPORTED_STORAGE_TYPE".to_string()),
    }
}

pub use encrypt::{Decryptor, Encryptor, encrypt_filename};
