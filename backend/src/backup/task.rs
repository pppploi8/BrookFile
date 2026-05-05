use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::RwLock as StdRwLock;
use std::collections::HashMap;
use chrono::Utc;
use uuid::Uuid;
use serde::Serialize;
use tokio::sync::RwLock as AsyncRwLock;

#[derive(Clone, Debug, PartialEq)]
pub enum TaskPhase {
    Scanning,
    Backup,
    Cleanup,
}

#[derive(Clone, Serialize)]
pub struct PendingItem {
    pub name: String,
    pub status: String,
    pub total_bytes: u64,
    pub uploaded_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub struct BackupTask {
    pub id: String,
    pub rule_id: String,
    pub mode: String,
    pub phase: Arc<StdRwLock<TaskPhase>>,
    pub cancel_flag: Arc<AtomicBool>,
    pub started_at: String,
    pub fail_reason: Arc<AsyncRwLock<Option<String>>>,
    pub pending_files: Arc<AsyncRwLock<Vec<PendingItem>>>,
    pub uploading_files: Arc<AsyncRwLock<HashMap<String, PendingItem>>>,
    pub pending_dirs: Arc<AsyncRwLock<Vec<PendingItem>>>,
    pub cleaning_dir: Arc<AsyncRwLock<Option<String>>>,
    pub backup_success_count: Arc<AtomicU64>,
    pub backup_fail_count: Arc<AtomicU64>,
    pub cleanup_deleted_count: Arc<AtomicU64>,
    pub consecutive_fail_count: Arc<AtomicU64>,
    pub task_failed: Arc<AtomicBool>,
    pub scanned_bytes: Arc<AtomicU64>,
}

impl Clone for BackupTask {
    fn clone(&self) -> Self {
        BackupTask {
            id: self.id.clone(),
            rule_id: self.rule_id.clone(),
            mode: self.mode.clone(),
            phase: Arc::clone(&self.phase),
            cancel_flag: Arc::clone(&self.cancel_flag),
            started_at: self.started_at.clone(),
            fail_reason: Arc::clone(&self.fail_reason),
            pending_files: Arc::clone(&self.pending_files),
            uploading_files: Arc::clone(&self.uploading_files),
            pending_dirs: Arc::clone(&self.pending_dirs),
            cleaning_dir: Arc::clone(&self.cleaning_dir),
            backup_success_count: Arc::clone(&self.backup_success_count),
            backup_fail_count: Arc::clone(&self.backup_fail_count),
            cleanup_deleted_count: Arc::clone(&self.cleanup_deleted_count),
            consecutive_fail_count: Arc::clone(&self.consecutive_fail_count),
            task_failed: Arc::clone(&self.task_failed),
            scanned_bytes: Arc::clone(&self.scanned_bytes),
        }
    }
}

impl BackupTask {
    pub fn new(rule_id: &str, mode: &str) -> Self {
        BackupTask {
            id: Uuid::new_v4().to_string(),
            rule_id: rule_id.to_string(),
            mode: mode.to_string(),
            phase: Arc::new(StdRwLock::new(TaskPhase::Scanning)),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            started_at: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            fail_reason: Arc::new(AsyncRwLock::new(None)),
            pending_files: Arc::new(AsyncRwLock::new(Vec::new())),
            uploading_files: Arc::new(AsyncRwLock::new(HashMap::new())),
            pending_dirs: Arc::new(AsyncRwLock::new(Vec::new())),
            cleaning_dir: Arc::new(AsyncRwLock::new(None)),
            backup_success_count: Arc::new(AtomicU64::new(0)),
            backup_fail_count: Arc::new(AtomicU64::new(0)),
            cleanup_deleted_count: Arc::new(AtomicU64::new(0)),
            consecutive_fail_count: Arc::new(AtomicU64::new(0)),
            task_failed: Arc::new(AtomicBool::new(false)),
            scanned_bytes: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::Relaxed)
    }

    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }

    pub async fn cancelled(&self) {
        while !self.cancel_flag.load(Ordering::Relaxed) {
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        }
    }

    pub fn get_phase(&self) -> TaskPhase {
        self.phase.read().map(|guard| guard.clone()).unwrap_or(TaskPhase::Backup)
    }

    pub fn set_phase(&self, phase: TaskPhase) {
        let _ = self.phase.write().map(|mut guard| *guard = phase);
    }

    pub async fn get_progress(&self) -> TaskProgress {
        let (phase, sub_phase) = match self.get_phase() {
            TaskPhase::Scanning => ("backup", Some("scanning")),
            TaskPhase::Backup => ("backup", None),
            TaskPhase::Cleanup => ("cleanup", None),
        };

        let mut items = Vec::new();

        let total_count = if phase == "backup" && sub_phase == Some("scanning") {
            0u64
        } else if phase == "backup" {
            let uploading = self.uploading_files.read().await;
            let mut uploading_list: Vec<PendingItem> = uploading.values()
                .filter(|v| v.status != "completed")
                .cloned()
                .collect();
            uploading_list.sort_by(|a, b| a.name.cmp(&b.name));
            items.extend(uploading_list);

            let pending = self.pending_files.read().await;
            let mut pending_list: Vec<PendingItem> = pending.iter().take(100usize.saturating_sub(items.len())).cloned().collect();
            items.append(&mut pending_list);

            uploading.len() as u64 + pending.len() as u64
        } else {
            let cleaning = self.cleaning_dir.read().await;
            if let Some(dir) = cleaning.as_ref() {
                items.push(PendingItem {
                    name: dir.clone(),
                    status: "cleaning".to_string(),
                    total_bytes: 0,
                    uploaded_bytes: 0,
                    error: None,
                });
            }

            let pending = self.pending_dirs.read().await;
            let pending_list: Vec<PendingItem> = pending.iter().take(100usize.saturating_sub(items.len())).cloned().collect();
            items.extend(pending_list);

            1 + pending.len() as u64
        };

        TaskProgress {
            is_running: true,
            phase: phase.to_string(),
            sub_phase: sub_phase.map(|s| s.to_string()),
            pending_items: items,
            total_count,
            scanned_bytes: self.scanned_bytes.load(Ordering::Relaxed),
        }
    }
}

#[derive(Serialize)]
pub struct TaskProgress {
    pub is_running: bool,
    pub phase: String,
    pub sub_phase: Option<String>,
    pub pending_items: Vec<PendingItem>,
    pub total_count: u64,
    pub scanned_bytes: u64,
}
