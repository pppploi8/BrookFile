use crate::restore::task::{RestoreTask, RestoreProgress, RestoreConfig, RestorePendingItem};
use crate::storage::{StorageBackend, Decryptor};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::sync::atomic::Ordering;
use futures::stream::{self, StreamExt};
use std::path::Path;
use chrono::Utc;
use serde::Deserialize;

pub struct RestoreStartError {
    pub fail_code: &'static str,
    pub message: Option<String>,
}

const INDEX_FILE: &str = ".index";
const INFO_FILE: &str = ".info";
const MAX_CONCURRENT_DOWNLOADS: usize = 5;
const CLEANUP_INTERVAL_SECS: u64 = 60;
const IDLE_TIMEOUT_SECS: i64 = 300;

#[derive(Deserialize)]
struct IndexEntry {
    path: String,
    size: u64,
    sha256: String,
    #[serde(default)]
    file_id: Option<String>,
}

type IndexEntries = Vec<(String, u64, String, Option<String>)>;

struct FileCleanupGuard<'a> {
    path: &'a Path,
    armed: bool,
}

impl<'a> Drop for FileCleanupGuard<'a> {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(self.path);
        }
    }
}

fn is_safe_relative_path(relative_path: &str) -> bool {
    let mut depth = 0i32;
    for component in Path::new(relative_path).components() {
        match component {
            std::path::Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            std::path::Component::Normal(_) => depth += 1,
            std::path::Component::RootDir | std::path::Component::Prefix(_) => return false,
            std::path::Component::CurDir => {}
        }
    }
    true
}

pub struct RestoreManager {
    running_tasks: Arc<RwLock<HashMap<String, Arc<RestoreTask>>>>,
}

impl RestoreManager {
    pub fn new() -> Self {
        let manager = RestoreManager {
            running_tasks: Arc::new(RwLock::new(HashMap::new())),
        };

        // 启动自动清理任务
        let running_tasks = Arc::clone(&manager.running_tasks);
        tokio::spawn(async move {
            Self::cleanup_loop(running_tasks).await;
        });

        manager
    }

    /// 启动恢复任务
    /// 验证存储连接、密码、解析 .index 都在此阶段完成
    /// 失败直接返回错误，不创建任务
    pub async fn start_task(&self, config: &RestoreConfig) -> Result<String, RestoreStartError> {
        let backend = match Self::create_backend(&config.storage_type, &config.storage_config) {
            Ok(b) => b,
            Err(e) => return Err(RestoreStartError { fail_code: "STORAGE_CONNECTION_ERROR", message: Some(e) }),
        };

        if let Err(e) = backend.list_files("").await {
            return Err(RestoreStartError { fail_code: "STORAGE_CONNECTION_ERROR", message: Some(e.to_string()) });
        }

        let derived_password = if config.encrypted {
            let info = Self::load_info(&backend).await;
            let derived = if let Some(info) = info {
                crate::models::backup_rule::encrypt_password_with_params(
                    config.backup_password.as_ref().unwrap(),
                    &info.kdf,
                )
            } else {
                crate::models::backup_rule::encrypt_password_string(
                    config.backup_password.as_ref().unwrap(),
                )
            };
            match derived {
                Ok(d) => Some(d),
                Err(e) => return Err(RestoreStartError { fail_code: "DECRYPTION_FAILED", message: Some(e) }),
            }
        } else {
            None
        };

        let index_entries = Self::load_index(&backend, config.encrypted, derived_password.as_deref()).await?;

        if index_entries.is_empty() {
            return Err(RestoreStartError { fail_code: "INDEX_NOT_FOUND", message: None });
        }

        let task = Arc::new(RestoreTask::new(
            config.user_id.clone(),
            config.clone(),
            config.target_path.clone(),
            config.encrypted,
            derived_password,
        ));
        let task_id = task.id.clone();

        {
            let mut pending = task.pending_files.write().await;
            *pending = index_entries.iter().map(|(path, size, sha256, file_id)| RestorePendingItem {
                name: path.clone(),
                status: "pending".to_string(),
                total_bytes: *size,
                downloaded_bytes: 0,
                error: None,
                file_id: file_id.clone(),
                sha256: sha256.clone(),
            }).collect();
        }
        task.total_count.store(index_entries.len() as u64, Ordering::Relaxed);

        {
            let mut tasks = self.running_tasks.write().await;
            tasks.insert(task_id.clone(), Arc::clone(&task));
        }

        let task_clone = Arc::clone(&task);
        let backend = Arc::new(backend);

        tokio::spawn(async move {
            Self::execute_restore_task(task_clone, backend).await;
        });

        Ok(task_id)
    }

    /// 取消恢复任务
    pub async fn cancel_task(&self, task_id: &str, user_id: &str) -> Result<bool, String> {
        let tasks = self.running_tasks.read().await;

        match tasks.get(task_id) {
            Some(task) => {
                if task.user_id != user_id {
                    return Err("TASK_NOT_RUNNING".to_string());
                }
                task.cancel();
                Ok(true)
            }
            None => Err("TASK_NOT_RUNNING".to_string()),
        }
    }

    pub async fn get_task_progress(&self, task_id: &str, user_id: &str) -> Result<RestoreProgress, String> {
        let tasks = self.running_tasks.read().await;

        let task = match tasks.get(task_id) {
            Some(t) if t.user_id == user_id => Arc::clone(t),
            _ => return Err("TASK_NOT_RUNNING".to_string()),
        };

        drop(tasks);

        // 更新最后查询时间
        {
            let mut last_query = task.last_query_time.write().await;
            *last_query = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        }

        // 获取进度
        let mut progress = task.get_progress().await;

        // 如果任务已完成，标记 is_running: false（但不删除任务，等用户手动 cleanup）
        if task.is_completed() {
            progress.is_running = false;
        }

        Ok(progress)
    }

    /// 重试单个失败文件
    pub async fn retry_file(&self, task_id: &str, file_path: &str, user_id: &str) -> Result<bool, String> {
        let tasks = self.running_tasks.read().await;

        let task = match tasks.get(task_id) {
            Some(t) if t.user_id == user_id => Arc::clone(t),
            _ => return Err("TASK_NOT_RUNNING".to_string()),
        };

        drop(tasks);

        // 检查文件是否在失败列表中
        {
            let failed = task.failed_files.read().await;
            if !failed.contains_key(file_path) {
                return Err("FILE_NOT_IN_FAILED_LIST".to_string());
            }
        }

        // 检查是否正在重试中
        {
            let downloading = task.downloading_files.read().await;
            if downloading.contains_key(file_path) {
                return Err("ALREADY_RETRYING".to_string());
            }
        }

        // 从失败列表移到下载列表
        let item = {
            let mut failed = task.failed_files.write().await;
            failed.remove(file_path)
        };

        if let Some(mut item) = item {
            item.status = "retrying".to_string();
            item.downloaded_bytes = 0;
            item.error = None;

            {
                let mut downloading = task.downloading_files.write().await;
                downloading.insert(file_path.to_string(), item.clone());
            }

            let task_clone = Arc::clone(&task);
            let file_path_owned = file_path.to_string();
            let target_path = task_clone.target_path.clone();
            let encrypted = task_clone.encrypted;
            let password = task_clone.backup_password.clone();
            let storage_type = task_clone.config.storage_type.clone();
            let storage_config = task_clone.config.storage_config.clone();

            tokio::spawn(async move {
                let backend = match Self::create_backend(&storage_type, &storage_config) {
                    Ok(b) => Arc::new(b),
                    Err(e) => {
                        Self::mark_file_failed(&task_clone, &file_path_owned, item.total_bytes, 0, &e, &item.sha256, item.file_id.as_deref()).await;
                        return;
                    }
                };

                Self::download_single_file(
                    task_clone,
                    &file_path_owned,
                    item.total_bytes,
                    item.file_id.as_deref(),
                    &item.sha256,
                    backend,
                    &target_path,
                    encrypted,
                    password.as_deref(),
                ).await;
            });

            Ok(true)
        } else {
            Err("FILE_NOT_IN_FAILED_LIST".to_string())
        }
    }

    /// 自动清理循环
    async fn cleanup_loop(running_tasks: Arc<RwLock<HashMap<String, Arc<RestoreTask>>>>) {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(CLEANUP_INTERVAL_SECS)).await;

            let tasks_to_remove = {
                let tasks = running_tasks.read().await;
                let now = Utc::now();

                let mut to_remove = Vec::new();
                for (id, task) in tasks.iter() {
                    if !task.is_completed() {
                        continue;
                    }

                    let last_query = task.last_query_time.read().await.clone();
                    if let Ok(last_query_time) = chrono::NaiveDateTime::parse_from_str(&last_query, "%Y-%m-%d %H:%M:%S") {
                        let last_query_datetime = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                            last_query_time,
                            *now.offset()
                        );
                        let elapsed = now.signed_duration_since(last_query_datetime);
                        if elapsed.num_seconds() >= IDLE_TIMEOUT_SECS {
                            to_remove.push(id.clone());
                        }
                    }
                }
                to_remove
            };

            if !tasks_to_remove.is_empty() {
                let mut tasks = running_tasks.write().await;
                for id in tasks_to_remove {
                    tasks.remove(&id);
                }
            }
        }
    }

    async fn download_single_file(
        task: Arc<RestoreTask>,
        relative_path: &str,
        size: u64,
        file_id: Option<&str>,
        sha256: &str,
        backend: Arc<Box<dyn StorageBackend>>,
        target_path: &str,
        encrypted: bool,
        password: Option<&str>,
    ) {
        if task.is_cancelled() {
            Self::mark_file_failed(&task, relative_path, size, 0, "CANCELLED", sha256, file_id).await;
            return;
        }

        if !is_safe_relative_path(relative_path) {
            Self::mark_file_failed(&task, relative_path, size, 0, "PATH_TRAVERSAL", sha256, file_id).await;
            return;
        }

        let local_file_path = Path::new(target_path).join(relative_path);
        let mut cleanup_guard = FileCleanupGuard { path: &local_file_path, armed: false };
        if let Some(parent) = local_file_path.parent() {
            if !parent.exists() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    Self::mark_file_failed(&task, relative_path, size, 0, &format!("Failed to create directory: {}", e), sha256, file_id).await;
                    return;
                }
            }
        }

        let remote_path = if let Some(id) = file_id {
            if encrypted {
                match crate::storage::encrypt_filename(id, password.unwrap()) {
                    Ok(p) => p,
                    Err(_) => {
                        Self::mark_file_failed(&task, relative_path, size, 0, "ENCRYPTION_ERROR", sha256, file_id).await;
                        return;
                    }
                }
            } else {
                id.to_string()
            }
        } else if encrypted {
            match crate::storage::encrypt_filename(relative_path, password.unwrap()) {
                Ok(p) => p,
                Err(_) => {
                    Self::mark_file_failed(&task, relative_path, size, 0, "ENCRYPTION_ERROR", sha256, file_id).await;
                    return;
                }
            }
        } else {
            relative_path.to_string()
        };

        let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(16);
        let relative_path_for_progress = relative_path.to_string();
        let task_for_progress = Arc::clone(&task);

        let progress_callback = Arc::new(move |downloaded: u64, _total: u64| {
            if let Ok(mut progress) = task_for_progress.download_progress.write() {
                progress.insert(relative_path_for_progress.clone(), downloaded);
            }
        });

        let backend_clone = Arc::clone(&backend);
        let remote_path_clone = remote_path.clone();
        let download_handle = tokio::spawn(async move {
            backend_clone.download_stream(&remote_path_clone, tx, progress_callback).await
        });

        let mut file = match tokio::fs::File::create(&local_file_path).await {
            Ok(f) => f,
            Err(e) => {
                Self::mark_file_failed(&task, relative_path, size, 0, &format!("Failed to create file: {}", e), sha256, file_id).await;
                return;
            }
        };
        cleanup_guard.armed = true;

        use tokio::io::AsyncWriteExt;

        let mut decryptor = if encrypted {
            match Decryptor::new(password.unwrap()) {
                Ok(d) => Some(d),
                Err(_) => {
                    Self::mark_file_failed(&task, relative_path, size, 0, "Decryptor init failed", sha256, file_id).await;
                    return;
                }
            }
        } else {
            None
        };

        let mut decrypt_buffer = Vec::new();
        const DECRYPT_BLOCK_SIZE: usize = 4096;
        let mut total_decrypted_bytes: u64 = 0;

        while let Some(chunk) = rx.recv().await {
            decrypt_buffer.extend_from_slice(&chunk);

            if let Some(ref mut dec) = decryptor {
                while decrypt_buffer.len() >= DECRYPT_BLOCK_SIZE {
                    let block: Vec<u8> = decrypt_buffer.drain(..DECRYPT_BLOCK_SIZE).collect();
                    let decrypted = match dec.decrypt_block(&block) {
                        Ok(d) => d,
                        Err(_) => {
                            Self::mark_file_failed(&task, relative_path, size, total_decrypted_bytes, "Decryption failed", sha256, file_id).await;
                            return;
                        }
                    };
                    if let Err(e) = file.write_all(&decrypted).await {
                        Self::mark_file_failed(&task, relative_path, size, total_decrypted_bytes, &format!("Failed to write file: {}", e), sha256, file_id).await;
                        return;
                    }
                    total_decrypted_bytes += decrypted.len() as u64;
                    task.downloaded_bytes.fetch_add(decrypted.len() as u64, Ordering::Relaxed);
                }
            } else {
                if let Err(e) = file.write_all(&chunk).await {
                        Self::mark_file_failed(&task, relative_path, size, total_decrypted_bytes, &format!("Failed to write file: {}", e), sha256, file_id).await;
                    return;
                }
                total_decrypted_bytes += chunk.len() as u64;
                task.downloaded_bytes.fetch_add(chunk.len() as u64, Ordering::Relaxed);
                decrypt_buffer.clear();
            }
        }

        if let Some(ref mut dec) = decryptor {
            if !decrypt_buffer.is_empty() {
                    let decrypted = match dec.decrypt_block(&decrypt_buffer) {
                    Ok(d) => d,
                    Err(_) => {
                        Self::mark_file_failed(&task, relative_path, size, total_decrypted_bytes, "Decryption failed", sha256, file_id).await;
                        return;
                    }
                };
                if let Err(e) = file.write_all(&decrypted).await {
                    Self::mark_file_failed(&task, relative_path, size, total_decrypted_bytes, &format!("Failed to write file: {}", e), sha256, file_id).await;
                    return;
                }
                total_decrypted_bytes += decrypted.len() as u64;
                task.downloaded_bytes.fetch_add(decrypted.len() as u64, Ordering::Relaxed);
            }
        }

        let local_path = local_file_path.clone();
        let _ = file.flush().await;
        drop(file);

        if !sha256.is_empty() {
            let local_path_clone = local_path.clone();
            let computed = tokio::task::spawn_blocking(move || {
                Self::calculate_file_sha256(&local_path_clone)
            }).await;
            if let Ok(Ok(computed)) = computed {
                if computed != sha256 {
                    let _ = std::fs::remove_file(&local_path);
                    cleanup_guard.armed = false;
                    Self::mark_file_failed(&task, relative_path, size, total_decrypted_bytes, "SHA-256 verification failed", sha256, file_id).await;
                    return;
                }
            }
        }

        match download_handle.await {
            Ok(Ok(_)) => {
                task.success_count.fetch_add(1, Ordering::Relaxed);
                cleanup_guard.armed = false;
                {
                    let mut downloading = task.downloading_files.write().await;
                    downloading.remove(relative_path);
                }
                if let Ok(mut progress) = task.download_progress.write() {
                    progress.remove(relative_path);
                }
            }
            Ok(Err(e)) => {
                Self::mark_file_failed(&task, relative_path, size, total_decrypted_bytes, &e.to_string(), sha256, file_id).await;
            }
            Err(e) => {
                Self::mark_file_failed(&task, relative_path, size, total_decrypted_bytes, &format!("Download task panicked: {}", e), sha256, file_id).await;
            }
        }
    }

    /// 执行恢复任务
    async fn execute_restore_task(
        task: Arc<RestoreTask>,
        backend: Arc<Box<dyn StorageBackend>>,
    ) {
        let files_to_download: Vec<(String, u64, Option<String>, String)> = {
            let pending = task.pending_files.read().await;
            pending.iter().map(|item| (item.name.clone(), item.total_bytes, item.file_id.clone(), item.sha256.clone())).collect()
        };

        if files_to_download.is_empty() {
            task.mark_completed();
            return;
        }

        let download_tasks: Vec<_> = files_to_download.into_iter().map(|(relative_path, size, file_id, sha256)| {
            let task = Arc::clone(&task);
            let backend = Arc::clone(&backend);
            let target_path = task.target_path.clone();
            let encrypted = task.encrypted;
            let password = task.backup_password.clone();

            async move {
                if task.is_cancelled() {
                    return Err((relative_path, "CANCELLED".to_string()));
                }

                if !is_safe_relative_path(&relative_path) {
                    return Err((relative_path, "PATH_TRAVERSAL".to_string()));
                }

                {
                    let mut pending = task.pending_files.write().await;
                    pending.retain(|p| p.name != relative_path);
                }

                let local_file_path = Path::new(&target_path).join(&relative_path);
                if let Ok(metadata) = tokio::fs::metadata(&local_file_path).await {
                    if metadata.len() == size {
                        let lfp = local_file_path.clone();
                        let computed_sha256 = tokio::task::spawn_blocking(move || {
                            Self::calculate_file_sha256(&lfp)
                        }).await;
                        if let Ok(Ok(sha)) = computed_sha256 {
                            if sha == sha256 {
                                task.success_count.fetch_add(1, Ordering::Relaxed);
                                return Ok(());
                            }
                        }
                    }
                }

                {
                    let mut downloading = task.downloading_files.write().await;
                    downloading.insert(relative_path.clone(), RestorePendingItem {
                        name: relative_path.clone(),
                        status: "downloading".to_string(),
                        total_bytes: size,
                        downloaded_bytes: 0,
                        error: None,
                        file_id: file_id.clone(),
                        sha256: sha256.clone(),
                    });
                }

                let remote_path = if let Some(ref id) = file_id {
                    if encrypted {
                        match crate::storage::encrypt_filename(id, password.as_ref().unwrap()) {
                            Ok(p) => p,
                            Err(e) => {
                                Self::mark_file_failed(&task, &relative_path, size, 0, "ENCRYPTION_ERROR", &sha256, file_id.as_deref()).await;
                                return Err((relative_path, e));
                            }
                        }
                    } else {
                        id.clone()
                    }
                } else if encrypted {
                    match crate::storage::encrypt_filename(&relative_path, password.as_ref().unwrap()) {
                        Ok(p) => p,
                        Err(e) => {
                            Self::mark_file_failed(&task, &relative_path, size, 0, "ENCRYPTION_ERROR", &sha256, file_id.as_deref()).await;
                            return Err((relative_path, e));
                        }
                    }
                } else {
                    relative_path.clone()
                };

                let local_file_path = Path::new(&target_path).join(&relative_path);
                let mut cleanup_guard = FileCleanupGuard { path: &local_file_path, armed: false };

                if let Some(parent) = local_file_path.parent() {
                    if !parent.exists() {
                        if let Err(e) = tokio::fs::create_dir_all(parent).await {
                            {
                                let mut downloading = task.downloading_files.write().await;
                                downloading.remove(&relative_path);
                            }
                            {
                                let mut failed = task.failed_files.write().await;
                                failed.insert(relative_path.clone(), RestorePendingItem {
                                    name: relative_path.clone(),
                                    status: "failed".to_string(),
                                    total_bytes: size,
                                    downloaded_bytes: 0,
                                    error: Some(format!("Failed to create directory: {}", e)),
                                    file_id: file_id.map(|s| s.to_string()),
                                    sha256: sha256.to_string(),
                                });
                            }
                            return Err((relative_path, e.to_string()));
                        }
                    }
                }

                let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(16);
                let relative_path_for_progress = relative_path.clone();
                let task_for_progress = task.clone();

                let progress_callback = Arc::new(move |downloaded: u64, _total: u64| {
                    if let Ok(mut progress) = task_for_progress.download_progress.write() {
                        progress.insert(relative_path_for_progress.clone(), downloaded);
                    }
                });

                let download_handle = tokio::spawn(async move {
                    backend.download_stream(&remote_path, tx, progress_callback).await
                });

                let mut file = tokio::fs::File::create(&local_file_path)
                    .await
                    .map_err(|e| (relative_path.clone(), format!("Failed to create file: {}", e)))?;
                cleanup_guard.armed = true;

                use tokio::io::AsyncWriteExt;

                let mut decryptor = if encrypted {
                    Some(Decryptor::new(password.as_ref().unwrap()).map_err(|e| (relative_path.clone(), e))?)
                } else {
                    None
                };

                let mut decrypt_buffer = Vec::new();
                const DECRYPT_BLOCK_SIZE: usize = 4096;
                let mut total_decrypted_bytes: u64 = 0;

                while let Some(chunk) = rx.recv().await {
                    decrypt_buffer.extend_from_slice(&chunk);

                    if let Some(ref mut dec) = decryptor {
                        while decrypt_buffer.len() >= DECRYPT_BLOCK_SIZE {
                            let block: Vec<u8> = decrypt_buffer.drain(..DECRYPT_BLOCK_SIZE).collect();
                            let decrypted = dec.decrypt_block(&block).map_err(|e| (relative_path.clone(), e))?;
                            if let Err(e) = file.write_all(&decrypted).await {
                                Self::mark_file_failed(&task, &relative_path, size, total_decrypted_bytes, &format!("Failed to write file: {}", e), &sha256, file_id.as_deref()).await;
                                return Err((relative_path, format!("Failed to write file: {}", e)));
                            }
                            total_decrypted_bytes += decrypted.len() as u64;
                            task.downloaded_bytes.fetch_add(decrypted.len() as u64, Ordering::Relaxed);
                        }
                    } else {
                        if let Err(e) = file.write_all(&chunk).await {
                            Self::mark_file_failed(&task, &relative_path, size, total_decrypted_bytes, &format!("Failed to write file: {}", e), &sha256, file_id.as_deref()).await;
                            return Err((relative_path, format!("Failed to write file: {}", e)));
                        }
                        total_decrypted_bytes += chunk.len() as u64;
                        task.downloaded_bytes.fetch_add(chunk.len() as u64, Ordering::Relaxed);
                        decrypt_buffer.clear();
                    }
                }

                if let Some(ref mut dec) = decryptor {
                    if !decrypt_buffer.is_empty() {
                        let decrypted = dec.decrypt_block(&decrypt_buffer).map_err(|e| (relative_path.clone(), e))?;
                        if let Err(e) = file.write_all(&decrypted).await {
                            Self::mark_file_failed(&task, &relative_path, size, total_decrypted_bytes, &format!("Failed to write file: {}", e), &sha256, file_id.as_deref()).await;
                            return Err((relative_path, format!("Failed to write file: {}", e)));
                        }
                        total_decrypted_bytes += decrypted.len() as u64;
                        task.downloaded_bytes.fetch_add(decrypted.len() as u64, Ordering::Relaxed);
                    }
                }

                file.flush()
                    .await
                    .map_err(|e| (relative_path.clone(), format!("Failed to flush file: {}", e)))?;

                drop(file);

                if !sha256.is_empty() {
                    let lfp = local_file_path.clone();
                    let computed = tokio::task::spawn_blocking(move || {
                        Self::calculate_file_sha256(&lfp)
                    }).await;
                    if let Ok(Ok(computed)) = computed {
                        if computed != sha256 {
                            let _ = std::fs::remove_file(&local_file_path);
                            cleanup_guard.armed = false;
                            Self::mark_file_failed(&task, &relative_path, size, total_decrypted_bytes, "SHA-256 verification failed", &sha256, file_id.as_deref()).await;
                            return Err((relative_path, "SHA-256 verification failed".to_string()));
                        }
                    }
                }

                match download_handle.await {
                    Ok(Ok(_)) => {
                        task.success_count.fetch_add(1, Ordering::Relaxed);
                        cleanup_guard.armed = false;
                        {
                            let mut downloading = task.downloading_files.write().await;
                            downloading.remove(&relative_path);
                        }
                        if let Ok(mut progress) = task.download_progress.write() {
                            progress.remove(&relative_path);
                        }
                        Ok(())
                    }
                    Ok(Err(e)) => {
                        Self::mark_file_failed(&task, &relative_path, size, total_decrypted_bytes, &e.to_string(), &sha256, file_id.as_deref()).await;
                        Err((relative_path, e.to_string()))
                    }
                    Err(e) => {
                        Self::mark_file_failed(&task, &relative_path, size, total_decrypted_bytes, &format!("Download task panicked: {}", e), &sha256, file_id.as_deref()).await;
                        Err((relative_path, format!("Download task panicked: {}", e)))
                    }
                }
            }
        }).collect();

        // 并发执行下载任务
        let mut download_stream = stream::iter(download_tasks).buffer_unordered(MAX_CONCURRENT_DOWNLOADS);

        loop {
            tokio::select! {
                result = download_stream.next() => {
                    match result {
                        Some(Ok(_)) => {}
                        Some(Err((_path, _error))) => {}
                        None => break,
                    }
                }
            }
        }

        // 任务完成，标记为已完成（不立即删除，等待 get_task_progress 返回一次 is_running: false）
        task.mark_completed();
    }

    /// 从远程加载索引文件
    async fn load_index(
        backend: &Box<dyn StorageBackend>,
        encrypted: bool,
        password: Option<&str>,
    ) -> Result<IndexEntries, RestoreStartError> {
        let index_path = if encrypted {
            crate::storage::encrypt_filename(INDEX_FILE, password.unwrap())
                .map_err(|e| RestoreStartError { fail_code: "STORAGE_CONNECTION_ERROR", message: Some(e) })?
        } else {
            INDEX_FILE.to_string()
        };

        let data = match backend.download_file(&index_path).await {
            Ok(d) => d,
            Err(_) => {
                return Err(RestoreStartError { fail_code: "INDEX_NOT_FOUND", message: None });
            }
        };

        let content = if encrypted {
            let mut decryptor = Decryptor::new(password.unwrap())
                .map_err(|_| RestoreStartError { fail_code: "DECRYPTION_FAILED", message: None })?;
            let decrypted = decryptor.decrypt_block(&data)
                .map_err(|_| RestoreStartError { fail_code: "DECRYPTION_FAILED", message: None })?;
            String::from_utf8(decrypted)
                .map_err(|_| RestoreStartError { fail_code: "DECRYPTION_FAILED", message: None })?
        } else {
            String::from_utf8(data)
                .map_err(|_| RestoreStartError { fail_code: "INDEX_NOT_FOUND", message: None })?
        };

        let mut entries = IndexEntries::new();
        for line in content.lines() {
            if let Ok(entry) = serde_json::from_str::<IndexEntry>(line) {
                entries.push((entry.path, entry.size, entry.sha256, entry.file_id));
            }
        }

        Ok(entries)
    }

    async fn load_info(backend: &Box<dyn StorageBackend>) -> Option<crate::models::backup_rule::BackupInfo> {
        let data = backend.download_file(INFO_FILE).await.ok()?;
        serde_json::from_slice(&data).ok()
    }

    /// 创建存储后端
    fn create_backend(storage_type: &str, config: &serde_json::Value) -> Result<Box<dyn StorageBackend>, String> {
        crate::storage::create_backend(storage_type, config)
    }

    /// 检查目标目录是否为空
    pub fn check_target_directory(path: &str) -> Result<(bool, usize, Vec<String>), String> {
        let target = Path::new(path);

        if !target.exists() {
            return Ok((true, 0, Vec::new()));
        }

        let mut files = Vec::new();
        let mut entry_count = 0usize;
        if let Ok(entries) = std::fs::read_dir(target) {
            for entry in entries.flatten() {
                entry_count += 1;
                if files.len() < 10 {
                    files.push(entry.file_name().to_string_lossy().to_string());
                }
            }
        }

        Ok((entry_count == 0, entry_count, files))
    }

    async fn mark_file_failed(task: &Arc<RestoreTask>, relative_path: &str, total_bytes: u64, downloaded_bytes: u64, error: &str, sha256: &str, file_id: Option<&str>) {
        {
            let mut downloading = task.downloading_files.write().await;
            downloading.remove(relative_path);
        }
        {
            let mut failed = task.failed_files.write().await;
            failed.insert(relative_path.to_string(), RestorePendingItem {
                name: relative_path.to_string(),
                status: "failed".to_string(),
                total_bytes,
                downloaded_bytes,
                error: Some(error.to_string()),
                file_id: file_id.map(|s| s.to_string()),
                sha256: sha256.to_string(),
            });
        }
        if let Ok(mut progress) = task.download_progress.write() {
            progress.remove(relative_path);
        }
    }

    fn calculate_file_sha256(path: &std::path::Path) -> Result<String, std::io::Error> {
        use sha2::{Sha256, Digest};
        let mut file = std::fs::File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 4096];
        loop {
            let bytes_read = std::io::Read::read(&mut file, &mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }
        Ok(format!("{:x}", hasher.finalize()))
    }
}
