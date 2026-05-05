use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::collections::HashMap;
use chrono::Utc;
use uuid::Uuid;
use serde::Serialize;
use tokio::sync::RwLock as AsyncRwLock;

#[derive(Clone, Serialize)]
pub struct RestorePendingItem {
    pub name: String,
    pub status: String,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip)]
    pub file_id: Option<String>,
    #[serde(skip)]
    pub sha256: String,
}

pub struct RestoreTask {
    pub id: String,
    pub user_id: String,
    pub config: RestoreConfig,
    pub target_path: String,
    pub encrypted: bool,
    pub backup_password: Option<String>,
    pub cancel_flag: Arc<AtomicBool>,
    pub pending_files: Arc<AsyncRwLock<Vec<RestorePendingItem>>>,
    pub downloading_files: Arc<AsyncRwLock<HashMap<String, RestorePendingItem>>>,
    pub failed_files: Arc<AsyncRwLock<HashMap<String, RestorePendingItem>>>,
    pub download_progress: Arc<std::sync::RwLock<HashMap<String, u64>>>,
    pub total_count: Arc<AtomicU64>,
    pub success_count: Arc<AtomicU64>,
    pub downloaded_bytes: Arc<AtomicU64>,
    pub started_at: String,
    pub completed: Arc<AtomicBool>,
    pub last_query_time: Arc<AsyncRwLock<String>>,
}

impl Clone for RestoreTask {
    fn clone(&self) -> Self {
        RestoreTask {
            id: self.id.clone(),
            user_id: self.user_id.clone(),
            config: self.config.clone(),
            target_path: self.target_path.clone(),
            encrypted: self.encrypted,
            backup_password: self.backup_password.clone(),
            cancel_flag: Arc::clone(&self.cancel_flag),
            pending_files: Arc::clone(&self.pending_files),
            downloading_files: Arc::clone(&self.downloading_files),
            failed_files: Arc::clone(&self.failed_files),
            download_progress: Arc::clone(&self.download_progress),
            total_count: Arc::clone(&self.total_count),
            success_count: Arc::clone(&self.success_count),
            downloaded_bytes: Arc::clone(&self.downloaded_bytes),
            started_at: self.started_at.clone(),
            completed: Arc::clone(&self.completed),
            last_query_time: Arc::clone(&self.last_query_time),
        }
    }
}

impl RestoreTask {
    pub fn new(user_id: String, config: RestoreConfig, target_path: String, encrypted: bool, backup_password: Option<String>) -> Self {
        RestoreTask {
            id: Uuid::new_v4().to_string(),
            user_id,
            config,
            target_path,
            encrypted,
            backup_password,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            pending_files: Arc::new(AsyncRwLock::new(Vec::new())),
            downloading_files: Arc::new(AsyncRwLock::new(HashMap::new())),
            failed_files: Arc::new(AsyncRwLock::new(HashMap::new())),
            download_progress: Arc::new(std::sync::RwLock::new(HashMap::new())),
            total_count: Arc::new(AtomicU64::new(0)),
            success_count: Arc::new(AtomicU64::new(0)),
            downloaded_bytes: Arc::new(AtomicU64::new(0)),
            started_at: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            completed: Arc::new(AtomicBool::new(false)),
            last_query_time: Arc::new(AsyncRwLock::new(Utc::now().format("%Y-%m-%d %H:%M:%S").to_string())),
        }
    }

    pub fn mark_completed(&self) {
        self.completed.store(true, Ordering::Relaxed);
    }

    pub fn is_completed(&self) -> bool {
        self.completed.load(Ordering::Relaxed)
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::Relaxed)
    }

    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }

    pub async fn get_progress(&self) -> RestoreProgress {
        let downloading = self.downloading_files.read().await;
        let progress = self.download_progress.read().unwrap_or_else(|e| e.into_inner());
        let mut downloading_list: Vec<RestorePendingItem> = downloading.values()
            .map(|v| {
                let mut item = v.clone();
                if let Some(&bytes) = progress.get(&v.name) {
                    item.downloaded_bytes = bytes;
                }
                item
            })
            .collect();
        downloading_list.sort_by(|a, b| a.name.cmp(&b.name));
        downloading_list.truncate(100);

        let failed = self.failed_files.read().await;
        let mut failed_list: Vec<RestorePendingItem> = failed.values()
            .cloned()
            .collect();
        failed_list.sort_by(|a, b| a.name.cmp(&b.name));

        let pending = self.pending_files.read().await;
        let pending_count = pending.len() as u64;

        RestoreProgress {
            is_running: true,
            downloading_items: downloading_list,
            failed_items: failed_list,
            pending_count,
            total_count: self.total_count.load(Ordering::Relaxed),
            success_count: self.success_count.load(Ordering::Relaxed),
            downloaded_bytes: self.downloaded_bytes.load(Ordering::Relaxed),
        }
    }
}

#[derive(Serialize)]
pub struct RestoreProgress {
    pub is_running: bool,
    pub downloading_items: Vec<RestorePendingItem>,
    pub failed_items: Vec<RestorePendingItem>,
    pub pending_count: u64,
    pub total_count: u64,
    pub success_count: u64,
    pub downloaded_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct RestoreConfig {
    pub user_id: String,
    pub storage_type: String,
    pub storage_config: serde_json::Value,
    pub encrypted: bool,
    pub backup_password: Option<String>,
    pub target_path: String,
}
