use crate::backup::task::{BackupTask, TaskProgress, TaskPhase, PendingItem};
use crate::database::Pool;
use crate::models::BackupRuleModel;
use crate::storage::{StorageBackend, Encryptor, Decryptor, encrypt_filename};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use walkdir::WalkDir;
use std::sync::atomic::Ordering;
use sha2::{Sha256, Digest};
use futures::stream::{self, StreamExt};
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};

const INDEX_FILE: &str = ".index";
const BLOCK_SIZE: usize = 4096;
const UPLOAD_CHUNK_SIZE: usize = 1024 * 1024; // 1MB
const CONCURRENT_UPLOADS: usize = 5;
const INDEX_SAVE_INTERVAL: Duration = Duration::from_secs(60); // 1分钟强制保存索引
const UPLOAD_RETRY_DELAY: Duration = Duration::from_secs(60); // 上传失败重试等待时间
const MAX_UPLOAD_RETRIES: u32 = 3; // 单个文件最大重试次数

#[derive(Serialize, Deserialize)]
struct IndexEntry {
    path: String,
    size: u64,
    sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    file_id: Option<String>,
}

// 索引内存数据结构：key = "size_sha256", value = [(路径, 是否已处理, 文件ID)]
// 文件ID：当原始文件名是.index时，生成随机ID存储，否则为None
type IndexMap = HashMap<String, Vec<(String, bool, Option<String>)>>;

struct PanicGuard {
    rule_id: String,
    running_tasks: Arc<RwLock<HashMap<String, Arc<BackupTask>>>>,
    disarmed: bool,
}

impl Drop for PanicGuard {
    fn drop(&mut self) {
        if !self.disarmed {
            let running_tasks = self.running_tasks.clone();
            let rule_id = self.rule_id.clone();
            std::thread::spawn(move || {
                if let Ok(rt) = tokio::runtime::Runtime::new() {
                    rt.block_on(async {
                        let mut tasks = running_tasks.write().await;
                        tasks.remove(&rule_id);
                    });
                }
            });
        }
    }
}

pub struct BackupManager {
    pool: Pool,
    running_tasks: Arc<RwLock<HashMap<String, Arc<BackupTask>>>>,
}

impl BackupManager {
    pub fn new(pool: Pool) -> Self {
        BackupManager {
            pool,
            running_tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn start_task(&self, rule_id: &str, mode: &str) -> Result<String, String> {
        let mut tasks = self.running_tasks.write().await;
        
        if tasks.contains_key(rule_id) {
            return Err("TASK_ALREADY_RUNNING".to_string());
        }

        let model = BackupRuleModel::new(&self.pool);
        let mut rule = model.get_by_id(rule_id)?.ok_or("BACKUP_RULE_NOT_FOUND".to_string())?;
        
        // 组合完整路径：用户 root_path + 规则 local_path
        let user_root_path = model.get_user_root_path(&rule.user_id)?;
        if let Some(root) = user_root_path {
            if rule.local_path.is_empty() || rule.local_path == "/" {
                rule.local_path = root;
            } else {
                rule.local_path = format!("{}/{}", root.trim_end_matches('/'), rule.local_path.trim_start_matches('/'));
            }
        }
        
        let task = Arc::new(BackupTask::new(rule_id, mode));
        let task_id = task.id.clone();

        // cleanup_only 模式跳过扫描阶段，直接进入清理阶段
        if mode == "cleanup_only" {
            task.set_phase(TaskPhase::Cleanup);
        }

        model.create_backup_log(&task_id, rule_id, mode)?;

        tasks.insert(rule_id.to_string(), Arc::clone(&task));
        drop(tasks);

        let pool = self.pool.clone();
        let running_tasks = Arc::clone(&self.running_tasks);
        let rule_clone = rule.clone();
        let task_clone = Arc::clone(&task);
        let mode_clone = mode.to_string();

        tokio::spawn(async move {
            let mut guard = PanicGuard {
                rule_id: task_clone.rule_id.clone(),
                running_tasks: Arc::clone(&running_tasks),
                disarmed: false,
            };
            Self::execute_backup_task(pool, running_tasks, task_clone, rule_clone, mode_clone).await;
            guard.disarmed = true;
        });

        Ok(task_id)
    }

    pub async fn cancel_task(&self, rule_id: &str) -> Result<bool, String> {
        let tasks = self.running_tasks.read().await;
        
        match tasks.get(rule_id) {
            Some(task) => {
                task.cancel();
                Ok(true)
            }
            None => Err("TASK_NOT_RUNNING".to_string()),
        }
    }

    pub async fn get_task_progress(&self, rule_id: &str) -> Result<TaskProgress, String> {
        let tasks = self.running_tasks.read().await;
        
        match tasks.get(rule_id) {
            Some(task) => Ok(task.get_progress().await),
            None => Err("TASK_NOT_RUNNING".to_string()),
        }
    }

    pub async fn handle_startup_interrupted_tasks(&self) {
        let model = BackupRuleModel::new(&self.pool);
        let _ = model.interrupt_running_logs();
    }

    pub async fn is_task_running(&self, rule_id: &str) -> bool {
        let tasks = self.running_tasks.read().await;
        tasks.contains_key(rule_id)
    }

    async fn execute_backup_task(
        pool: Pool,
        running_tasks: Arc<RwLock<HashMap<String, Arc<BackupTask>>>>,
        task: Arc<BackupTask>,
        rule: crate::models::BackupRuleDetail,
        mode: String,
    ) {
        let task_id = task.id.clone();
        let rule_id = task.rule_id.clone();

        let result = if mode == "cleanup_only" {
            task.set_phase(TaskPhase::Cleanup);
            Self::execute_cleanup_phase(&task, &rule).await
        } else {
            let backup_result = Self::execute_backup_phase(&task, &rule).await;
            if task.is_cancelled() {
                Err("CANCELLED".to_string())
            } else if backup_result.is_err() {
                backup_result
            } else {
                task.set_phase(TaskPhase::Cleanup);
                Self::execute_cleanup_phase(&task, &rule).await
            }
        };

        let status = if task.task_failed.load(Ordering::Relaxed) {
            "failed"
        } else if task.is_cancelled() {
            "cancelled"
        } else if result.is_err() {
            "failed"
        } else {
            "completed"
        };

        let fail_reason = if status == "failed" {
            let task_fail_reason = task.fail_reason.read().await.clone();
            task_fail_reason.or_else(|| result.err())
        } else {
            None
        };

        let success = task.backup_success_count.load(Ordering::Relaxed);
        let fail = task.backup_fail_count.load(Ordering::Relaxed);
        let deleted = task.cleanup_deleted_count.load(Ordering::Relaxed);

        let model = BackupRuleModel::new(&pool);
        model.finish_backup_log(&task_id, status, success, fail, deleted, fail_reason.as_deref()).ok();
        model.update_rule_last_backup_time(&rule_id).ok();

        let mut tasks = running_tasks.write().await;
        tasks.remove(&rule_id);
    }

    fn calculate_file_sha256(
        path: &std::path::Path,
        scanned_bytes: &std::sync::atomic::AtomicU64,
    ) -> std::io::Result<String> {
        use std::io::Read;
        
        let mut file = std::fs::File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 64 * 1024];
        
        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
            scanned_bytes.fetch_add(bytes_read as u64, Ordering::Relaxed);
        }
        
        Ok(format!("{:x}", hasher.finalize()))
    }

    async fn execute_backup_phase(
        task: &BackupTask,
        rule: &crate::models::BackupRuleDetail,
    ) -> Result<(), String> {
        let backend = Self::create_backend(&rule.storage_type, &rule.storage_config)?;

        let local_path = std::path::Path::new(&rule.local_path);
        if !local_path.exists() {
            return Err("LOCAL_PATH_NOT_EXISTS".to_string());
        }

        let mut index = Self::load_index(&backend, rule.encrypted, rule.backup_password.as_deref()).await?;

        let mut files_to_upload: Vec<(String, u64, String)> = Vec::new();
        
        for entry in WalkDir::new(local_path).into_iter().filter_map(|e| e.ok()) {
            if task.is_cancelled() {
                return Err("CANCELLED".to_string());
            }

            let path = entry.path();
            if path.is_dir() {
                continue;
            }

            let relative_path = path.strip_prefix(local_path)
                .map_err(|_| "PATH_ERROR".to_string())?
                .to_string_lossy()
                .replace('\\', "/");

            let path_owned = path.to_path_buf();
            let scanned_bytes = Arc::clone(&task.scanned_bytes);
            let (size, sha256) = tokio::task::spawn_blocking(move || {
                let metadata = std::fs::metadata(&path_owned)?;
                let size = metadata.len();
                let sha256 = Self::calculate_file_sha256(&path_owned, &scanned_bytes)?;
                Ok::<_, std::io::Error>((size, sha256))
            }).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;

            let index_key = format!("{}_{}", size, sha256);
            
            if let Some(existing_items) = index.get_mut(&index_key) {
                // 检查列表中是否已有相同路径且已处理
                let already_processed = existing_items.iter().any(|(path, processed, _)| {
                    path == &relative_path && *processed
                });

                if already_processed {
                    // 已处理过，跳过
                    continue;
                }

                // 检查列表中是否已有相同路径但未处理
                let mut found_unprocessed = false;
                for (path, processed, _) in existing_items.iter_mut() {
                    if path == &relative_path && !*processed {
                        // 找到未处理的相同路径，标记为已处理
                        *processed = true;
                        found_unprocessed = true;
                        break;
                    }
                }

                if found_unprocessed {
                    continue;
                }

                // 检查列表中是否有未处理的路径可以移动
                let mut moved = false;
                for (path, processed, file_id) in existing_items.iter_mut() {
                    if !*processed {
                        // 选择第一个未处理的路径进行移动
                        // 如果文件有ID，使用ID作为远程文件名
                        let remote_from = if let Some(ref id) = file_id {
                            id.clone()
                        } else if rule.encrypted {
                            encrypt_filename(path, rule.backup_password.as_ref().unwrap())?
                        } else {
                            path.clone()
                        };
                        let remote_to = if rule.encrypted {
                            encrypt_filename(&relative_path, rule.backup_password.as_ref().unwrap())?
                        } else {
                            relative_path.clone()
                        };
                        if let Err(e) = backend.move_file(&remote_from, &remote_to).await {
                            crate::error_logger::log_error("BACKUP_MOVE", &format!("Failed to move file: {}", e));
                            continue;
                        }
                        *processed = true;
                        *file_id = None;
                        moved = true;
                        break;
                    }
                }

                // 将新路径添加到列表，标记为已处理
                existing_items.push((relative_path.clone(), true, None));

                if moved {
                    continue;
                }
            }

            files_to_upload.push((relative_path, size, sha256));
        }

        for entries in index.values_mut() {
            entries.retain(|(_, processed, _)| *processed);
        }
        index.retain(|_, entries| !entries.is_empty());

        task.set_phase(TaskPhase::Backup);

        {
            let mut pending = task.pending_files.write().await;
            *pending = files_to_upload.iter().map(|(path, size, _)| PendingItem {
                name: path.clone(),
                status: "pending".to_string(),
                total_bytes: *size,
                uploaded_bytes: 0,
                error: None,
            }).collect();
        }

        let task_clone = Arc::new((*task).clone());
        let backend_clone = backend.as_ref();
        let encrypted = rule.encrypted;
        let password = rule.backup_password.clone();
        let local_path_base = rule.local_path.clone();

        let index_for_save = Arc::new(RwLock::new(index));

        let has_changes = Arc::new(RwLock::new(false));

        let created_dirs = Arc::new(RwLock::new(HashSet::<String>::new()));
        
        if files_to_upload.is_empty() {
            let final_index = index_for_save.read().await.clone();
            Self::save_index(&backend, &final_index, encrypted, rule.backup_password.as_deref()).await?;
            return Ok(());
        }

        let upload_tasks: Vec<_> = files_to_upload.into_iter().map(|(relative_path, size, sha256)| {
            let task = task_clone.clone();
            let backend = backend_clone;
            let encrypted = encrypted;
            let password = password.clone();
            let local_path_base = local_path_base.clone();
            let index = index_for_save.clone();
            let has_changes_clone = has_changes.clone();
            let created_dirs = created_dirs.clone();
            
            async move {
                if task.task_failed.load(Ordering::Relaxed) {
                    return Err((relative_path, "TASK_ALREADY_FAILED".to_string()));
                }
                
                let local_file_path = std::path::Path::new(&local_path_base).join(&relative_path);
                
                // 只有根目录下的.index文件才生成随机ID，子目录的.index当作普通文件处理
                let is_root_index = relative_path == ".index";
                let file_id = if is_root_index {
                    Some(uuid::Uuid::new_v4().to_string())
                } else {
                    None
                };
                
                // 构造远程路径：如果是.index文件，使用随机ID作为文件名
                let remote_path = if let Some(ref id) = file_id {
                    if encrypted {
                        encrypt_filename(id, password.as_ref().unwrap()).map_err(|e| (relative_path.clone(), e))?
                    } else {
                        id.clone()
                    }
                } else if encrypted {
                    encrypt_filename(&relative_path, password.as_ref().unwrap()).map_err(|e| (relative_path.clone(), e))?
                } else {
                    relative_path.clone()
                };

                // 上传前确保目录存在
                if let Some(parent_dir) = std::path::Path::new(&remote_path).parent().and_then(|p| p.to_str()) {
                    if !parent_dir.is_empty() && parent_dir != "." && parent_dir != "/" {
                        let need_create = {
                            let dirs = created_dirs.write().await;
                            !dirs.contains(parent_dir)
                        };
                        
                        if need_create {
                            if let Err(e) = backend.mkdir(parent_dir).await {
                                crate::error_logger::log_error("BACKUP_MKDIR", &format!("Failed to create dir {}: {}", parent_dir, e));
                            } else {
                                let mut dirs = created_dirs.write().await;
                                dirs.insert(parent_dir.to_string());
                            }
                        }
                    }
                }

                // 重试上传逻辑 - 每个文件独立重试3次
                let mut last_error = String::new();
                let mut success = false;
                
                for retry_count in 0..MAX_UPLOAD_RETRIES {
                    if task.task_failed.load(Ordering::Relaxed) {
                        return Err((relative_path, "TASK_ALREADY_FAILED".to_string()));
                    }

                    if task.is_cancelled() {
                        return Err((relative_path, "CANCELLED".to_string()));
                    }

                    // 非首次尝试时，先显示等待重试状态，再等待重试延迟
                    if retry_count > 0 {
                        {
                            let mut uploading = task.uploading_files.write().await;
                            if let Some(item) = uploading.get_mut(&relative_path) {
                                item.status = format!("waiting_retry ({}/{})", retry_count, MAX_UPLOAD_RETRIES - 1);
                            }
                        }
                        tokio::time::sleep(UPLOAD_RETRY_DELAY).await;
                    }

                    {
                        let mut uploading = task.uploading_files.write().await;
                        uploading.insert(relative_path.clone(), PendingItem {
                            name: relative_path.clone(),
                            status: if retry_count > 0 { 
                                format!("retrying ({}/{})", retry_count, MAX_UPLOAD_RETRIES - 1)
                            } else {
                                "uploading".to_string()
                            },
                            total_bytes: size,
                            uploaded_bytes: 0,
                            error: None,
                        });
                    }
                    
                    {
                        let mut pending = task.pending_files.write().await;
                        pending.retain(|p| p.name != relative_path);
                    }

                    let result = Self::upload_file(
                        backend,
                        &local_file_path.to_string_lossy(),
                        &remote_path,
                        encrypted,
                        password.as_deref(),
                        task.clone(),
                        &relative_path,
                    ).await;

                    match result {
                        Ok(_) => {
                            {
                                let mut uploading = task.uploading_files.write().await;
                                if let Some(item) = uploading.get_mut(&relative_path) {
                                    item.status = "completed".to_string();
                                }
                            }
                            success = true;
                            break;
                        }
                        Err(e) => {
                            last_error = e.to_string();
                            
                            let is_retryable = matches!(e, crate::storage::StorageError::ConnectionError(_));
                            
                            if !is_retryable {
                                {
                                    let mut uploading = task.uploading_files.write().await;
                                    if let Some(item) = uploading.get_mut(&relative_path) {
                                        item.status = "failed".to_string();
                                    }
                                }
                                let error_msg = format!("服务器错误，终止任务: {}", last_error);
                                let mut fail_reason = task.fail_reason.write().await;
                                *fail_reason = Some(error_msg);
                                drop(fail_reason);
                                task.task_failed.store(true, Ordering::Relaxed);
                                return Err((relative_path, last_error));
                            }
                        }
                    }
                }

                if success {
                    task.consecutive_fail_count.store(0, Ordering::Relaxed);
                    task.backup_success_count.fetch_add(1, Ordering::Relaxed);
                    let index_key = format!("{}_{}", size, sha256);
                    let mut index_guard = index.write().await;
                    index_guard
                        .entry(index_key)
                        .or_insert_with(Vec::new)
                        .push((relative_path.clone(), true, file_id));
                    drop(index_guard);
                    let mut changes_guard = has_changes_clone.write().await;
                    *changes_guard = true;
                    drop(changes_guard);
                    Ok(())
                } else {
                    task.backup_fail_count.fetch_add(1, Ordering::Relaxed);
                    
                    {
                        let mut uploading = task.uploading_files.write().await;
                        if let Some(item) = uploading.get_mut(&relative_path) {
                            item.status = "failed".to_string();
                            item.error = Some(last_error.clone());
                        }
                    }
                    
                    let consecutive_fails = task.consecutive_fail_count.fetch_add(1, Ordering::Relaxed) + 1;
                    
                    if consecutive_fails >= CONCURRENT_UPLOADS as u64 {
                        let error_msg = format!("连续{}个文件上传失败（每个文件重试{}次后仍失败），终止任务。最后错误: {}", 
                            consecutive_fails, MAX_UPLOAD_RETRIES, last_error);
                        let mut fail_reason = task.fail_reason.write().await;
                        *fail_reason = Some(error_msg);
                        drop(fail_reason);
                        task.task_failed.store(true, Ordering::Relaxed);
                        task.cancel();
                    }
                    
                    Err((relative_path, last_error))
                }
            }
        }).collect();

        let start_time = Instant::now();
        let mut last_save_time = start_time;
        
        let mut upload_stream = stream::iter(upload_tasks).buffer_unordered(CONCURRENT_UPLOADS);

        loop {
            tokio::select! {
                result = upload_stream.next() => {
                match result {
                    Some(Ok(_)) => {
                        let now = Instant::now();
                        if now.duration_since(last_save_time) >= INDEX_SAVE_INTERVAL {
                            let changes_guard = has_changes.read().await;
                            let need_save = *changes_guard;
                            drop(changes_guard);
                            
                            if need_save {
                                let index_guard = index_for_save.read().await;
                                let index_clone = index_guard.clone();
                                drop(index_guard);
                                
                                if let Err(_) = Self::save_index(&backend, &index_clone, encrypted, rule.backup_password.as_deref()).await {
                                } else {
                                    last_save_time = now;
                                    let mut changes_guard = has_changes.write().await;
                                    *changes_guard = false;
                                }
                            }
                        }
                    }
                    Some(Err(_)) => {}
                    None => {
                        break;
                    }
                }
            }
            _ = task_clone.cancelled() => {
                break;
            }
            }
        }

        if task.is_cancelled() {
            return Err("CANCELLED".to_string());
        }

        if task.task_failed.load(Ordering::Relaxed) {
            let remaining_files = {
                let pending = task.pending_files.read().await;
                pending.len() as u64
            };
            if remaining_files > 0 {
                task.backup_fail_count.fetch_add(remaining_files, Ordering::Relaxed);
                let mut pending = task.pending_files.write().await;
                pending.clear();
            }
        }

        let final_index = index_for_save.read().await.clone();
        Self::save_index(&backend, &final_index, encrypted, rule.backup_password.as_deref()).await?;

        if task.task_failed.load(Ordering::Relaxed) {
            let fail_reason = task.fail_reason.read().await.clone();
            return Err(fail_reason.unwrap_or_else(|| "UPLOAD_FAILED".to_string()));
        }

        Ok(())
    }

    async fn upload_file(
        backend: &dyn StorageBackend,
        local_path: &str,
        remote_path: &str,
        encrypted: bool,
        password: Option<&str>,
        task: Arc<BackupTask>,
        relative_path: &str,
    ) -> Result<(), crate::storage::StorageError> {
        use tokio::io::AsyncReadExt;

        let file = tokio::fs::File::open(local_path).await
            .map_err(|e| crate::storage::StorageError::Other(e.to_string()))?;
        
        let metadata = file.metadata().await
            .map_err(|e| crate::storage::StorageError::Other(e.to_string()))?;
        let file_size = metadata.len();

        let (tx, rx) = tokio::sync::mpsc::channel::<Vec<u8>>(16);
        let relative_path_owned = relative_path.to_string();
        
        let read_task = async move {
            let mut reader = tokio::io::BufReader::new(file);
            let mut buffer = vec![0u8; BLOCK_SIZE];
            let mut upload_buffer = Vec::with_capacity(UPLOAD_CHUNK_SIZE + BLOCK_SIZE);
            let mut actual_total_size = 0u64;
            let mut raw_bytes_uploaded = 0u64;
            
            let mut encryptor = if encrypted {
                Some(Encryptor::new(password.unwrap()).map_err(|e| crate::storage::StorageError::Other(e))?)
            } else {
                None
            };

            loop {
                let bytes_read = match reader.read(&mut buffer).await {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(e) => {
                        return Err(crate::storage::StorageError::Other(e.to_string()));
                    }
                };

                let encrypted_data = if let Some(ref mut enc) = encryptor {
                    enc.encrypt_block(&buffer[..bytes_read]).map_err(|e| crate::storage::StorageError::Other(e))?
                } else {
                    buffer[..bytes_read].to_vec()
                };
                
                upload_buffer.extend_from_slice(&encrypted_data);
                raw_bytes_uploaded += bytes_read as u64;
                
                if upload_buffer.len() >= UPLOAD_CHUNK_SIZE {
                    let chunk = std::mem::take(&mut upload_buffer);
                    actual_total_size += chunk.len() as u64;
                    
                    {
                        let mut uploading = task.uploading_files.write().await;
                        if let Some(item) = uploading.get_mut(&relative_path_owned) {
                            item.uploaded_bytes = raw_bytes_uploaded;
                        }
                    }

                    if tx.send(chunk).await.is_err() {
                        break;
                    }
                    
                    upload_buffer = Vec::with_capacity(UPLOAD_CHUNK_SIZE + BLOCK_SIZE);
                }
            }

            if !upload_buffer.is_empty() {
                actual_total_size += upload_buffer.len() as u64;
                
                {
                    let mut uploading = task.uploading_files.write().await;
                    if let Some(item) = uploading.get_mut(&relative_path_owned) {
                        item.uploaded_bytes = raw_bytes_uploaded;
                    }
                }

                let _ = tx.send(upload_buffer).await;
            }

            Ok::<_, crate::storage::StorageError>(actual_total_size)
        };

        let progress_callback = Arc::new(|_current: u64, _total: u64| {});

        let remote_path_owned = remote_path.to_string();
        let upload_task = async move {
            backend.upload_stream(&remote_path_owned, file_size, rx, progress_callback).await
        };

        let (read_result, upload_result) = tokio::join!(read_task, upload_task);
        
        read_result?;
        upload_result
    }

    async fn execute_cleanup_phase(
        task: &BackupTask,
        rule: &crate::models::BackupRuleDetail,
    ) -> Result<(), String> {
        let backend = Self::create_backend(&rule.storage_type, &rule.storage_config)?;

        let index = Self::load_index(&backend, rule.encrypted, rule.backup_password.as_deref()).await?;

        if index.is_empty() {
            return Ok(());
        }

        let mut indexed_remote_paths: HashSet<String> = HashSet::new();
        for items in index.values() {
            for (path, _, file_id) in items {
                let remote = if let Some(ref id) = file_id {
                    if rule.encrypted {
                        encrypt_filename(id, rule.backup_password.as_ref().unwrap())?
                    } else {
                        id.clone()
                    }
                } else if rule.encrypted {
                    encrypt_filename(path, rule.backup_password.as_ref().unwrap())?
                } else {
                    path.clone()
                };
                indexed_remote_paths.insert(remote);
            }
        }
        let index_remote_name = if rule.encrypted {
            encrypt_filename(INDEX_FILE, rule.backup_password.as_ref().unwrap())?
        } else {
            INDEX_FILE.to_string()
        };

        let mut all_remote_dirs: Vec<String> = Vec::new();
        Self::collect_all_dirs_recursive(&*backend, "", &mut all_remote_dirs).await;

        all_remote_dirs.sort_by(|a, b| {
            let a_depth = a.matches('/').count();
            let b_depth = b.matches('/').count();
            a_depth.cmp(&b_depth)
        });

        {
            let mut pending_dirs = task.pending_dirs.write().await;
            *pending_dirs = all_remote_dirs.iter().map(|dir| PendingItem {
                name: dir.clone(),
                status: "pending".to_string(),
                total_bytes: 0,
                uploaded_bytes: 0,
                error: None,
            }).collect();
        }

        let mut empty_dirs: Vec<String> = Vec::new();

        for dir in all_remote_dirs {
            if task.is_cancelled() {
                return Err("CANCELLED".to_string());
            }

            {
                let mut cleaning_dir = task.cleaning_dir.write().await;
                *cleaning_dir = Some(dir.clone());
            }
            {
                let mut pending_dirs = task.pending_dirs.write().await;
                pending_dirs.retain(|p| p.name != dir);
            }

            let files = match backend.list_files(&dir).await {
                Ok(f) => f,
                Err(_) => continue,
            };

            if files.is_empty() {
                empty_dirs.push(dir);
                continue;
            }

            let mut dir_has_content = false;

            for file in files {
                if task.is_cancelled() {
                    return Err("CANCELLED".to_string());
                }

                let remote_path = if dir.is_empty() { file.path.clone() } else { format!("{}/{}", dir, file.path) };

                if file.is_dir {
                    continue;
                }

                if remote_path == index_remote_name || indexed_remote_paths.contains(&remote_path) {
                    dir_has_content = true;
                    continue;
                }

                let _ = backend.delete_file(&remote_path).await;
                task.cleanup_deleted_count.fetch_add(1, Ordering::Relaxed);
            }

            if !dir_has_content {
                empty_dirs.push(dir);
            }
        }

        empty_dirs.sort_by(|a, b| {
            let a_depth = a.matches('/').count();
            let b_depth = b.matches('/').count();
            b_depth.cmp(&a_depth)
        });
        for dir in empty_dirs {
            if task.is_cancelled() {
                return Err("CANCELLED".to_string());
            }
            let _ = backend.delete_file(&dir).await;
        }

        {
            let mut cleaning_dir = task.cleaning_dir.write().await;
            *cleaning_dir = None;
        }

        Ok(())
    }

    async fn collect_all_dirs_recursive(
        backend: &dyn StorageBackend,
        dir: &str,
        result: &mut Vec<String>,
    ) {
        result.push(dir.to_string());
        let files = match backend.list_files(dir).await {
            Ok(f) => f,
            Err(_) => return,
        };
        for file in files {
            if file.is_dir {
                let subdir = if dir.is_empty() { file.path } else { format!("{}/{}", dir, file.path) };
                Box::pin(Self::collect_all_dirs_recursive(backend, &subdir, result)).await;
            }
        }
    }

    async fn load_index(
        backend: &Box<dyn StorageBackend>,
        encrypted: bool,
        password: Option<&str>,
    ) -> Result<IndexMap, String> {
        let index_path = if encrypted {
            encrypt_filename(INDEX_FILE, password.unwrap())?
        } else {
            INDEX_FILE.to_string()
        };

        let data = match backend.download_file(&index_path).await {
            Ok(d) => d,
            Err(_) => {
                return Ok(IndexMap::new());
            }
        };

        let content = if encrypted {
            let mut decryptor = Decryptor::new(password.unwrap()).map_err(|e| e.to_string())?;
            let decrypted = decryptor.decrypt_block(&data).map_err(|e| e.to_string())?;
            String::from_utf8(decrypted).map_err(|e| e.to_string())?
        } else {
            String::from_utf8(data).map_err(|e| e.to_string())?
        };

        let mut index = IndexMap::new();
        for line in content.lines() {
            if let Ok(entry) = serde_json::from_str::<IndexEntry>(line) {
                let key = format!("{}_{}", entry.size, entry.sha256);
                index.entry(key).or_insert_with(Vec::new).push((entry.path, false, entry.file_id));
            }
        }

        Ok(index)
    }

    async fn save_index(
        backend: &Box<dyn StorageBackend>,
        index: &IndexMap,
        encrypted: bool,
        password: Option<&str>,
    ) -> Result<(), String> {
        // 文件格式: JSON Lines (每行一个 JSON 对象: path, size, sha256, file_id?)
        // file_id 可选，用于存储原始文件名为 .index 的文件
        let mut lines = Vec::new();
        for (key, items) in index {
            let parts: Vec<&str> = key.split('_').collect();
            if parts.len() >= 2 {
                let size: u64 = parts[0].parse().unwrap_or(0);
                let sha256 = parts[1..].join("_");
                for (path, _, file_id) in items {
                    let entry = IndexEntry {
                        path: path.clone(),
                        size,
                        sha256: sha256.clone(),
                        file_id: file_id.clone(),
                    };
                    lines.push(serde_json::to_string(&entry).unwrap_or_default());
                }
            }
        }
        let mut content = lines.join("\n");
        if !content.is_empty() {
            content.push('\n');
        }

        let data = if encrypted {
            let mut encryptor = Encryptor::new(password.unwrap()).map_err(|e| e.to_string())?;
            encryptor.encrypt_block(content.as_bytes()).map_err(|e| e.to_string())?
        } else {
            content.into_bytes()
        };

        let index_path = if encrypted {
            encrypt_filename(INDEX_FILE, password.unwrap())?
        } else {
            INDEX_FILE.to_string()
        };

        let (tx, rx) = tokio::sync::mpsc::channel::<Vec<u8>>(1);
        let total_size = data.len() as u64;
        
        let send_task = async move {
            let _ = tx.send(data).await;
            Ok::<_, crate::storage::StorageError>(())
        };
        
        let upload_task = backend.upload_stream(&index_path, total_size, rx, Arc::new(|_, _| {}));
        
        let (send_result, upload_result) = tokio::join!(send_task, upload_task);
        send_result.map_err(|e| e.to_string())?;
        upload_result.map_err(|e| e.to_string())?;

        Ok(())
    }

    fn create_backend(storage_type: &str, config: &serde_json::Value) -> Result<Box<dyn StorageBackend>, String> {
        crate::storage::create_backend(storage_type, config)
    }
}
