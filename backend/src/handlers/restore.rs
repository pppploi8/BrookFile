use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::path::Path;
use crate::app_state::AppState;
use crate::handlers::{ApiResponse, internal_error_response, get_user_root_path, get_current_user_id, is_safe_path, is_path_under_root};
use crate::restore::RestoreConfig;

/// 检查目标目录请求
#[derive(Debug, Deserialize)]
pub struct CheckRestoreTargetRequest {
    pub local_path: String,
}

/// 检查目标目录响应
#[derive(Serialize)]
pub struct CheckRestoreTargetResponse {
    pub is_empty: bool,
    pub file_count: usize,
    pub files: Vec<String>,
}

/// 检查目标目录是否为空
pub async fn check_restore_target(
    req: web::Json<CheckRestoreTargetRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };

    if !req.local_path.is_empty() && req.local_path != "/" && !is_safe_path(&req.local_path) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    let full_path = if req.local_path.is_empty() || req.local_path == "/" {
        root_path.clone()
    } else {
        format!("{}/{}", root_path.trim_end_matches('/'), req.local_path.trim_start_matches('/'))
    };

    let root_path_obj = Path::new(&root_path);
    let full_path_obj = Path::new(&full_path);
    if !is_path_under_root(full_path_obj, root_path_obj) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    match crate::restore::RestoreManager::check_target_directory(&full_path) {
        Ok((is_empty, file_count, files)) => {
            HttpResponse::Ok().json(CheckRestoreTargetResponse {
                is_empty,
                file_count,
                files,
            })
        }
        Err(e) => internal_error_response("/api/restore/check", &e),
    }
}

/// 启动恢复请求
#[derive(Debug, Deserialize)]
pub struct StartRestoreRequest {
    pub storage_type: String,
    pub storage_config: serde_json::Value,
    #[serde(default)]
    pub encrypted: bool,
    pub backup_password: Option<String>,
    pub local_path: String,
}

/// 启动恢复响应
#[derive(Serialize)]
pub struct StartRestoreResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// 启动恢复任务
pub async fn start_restore(
    req: web::Json<StartRestoreRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };

    if !req.local_path.is_empty() && req.local_path != "/" && !is_safe_path(&req.local_path) {
        return HttpResponse::Ok().json(StartRestoreResponse {
            task_id: None,
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            message: None,
        });
    }

    let full_path = if req.local_path.is_empty() || req.local_path == "/" {
        root_path.clone()
    } else {
        format!("{}/{}", root_path.trim_end_matches('/'), req.local_path.trim_start_matches('/'))
    };

    let root_path_obj = Path::new(&root_path);
    let full_path_obj = Path::new(&full_path);
    if !is_path_under_root(full_path_obj, root_path_obj) {
        return HttpResponse::Ok().json(StartRestoreResponse {
            task_id: None,
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            message: None,
        });
    }

    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    if req.encrypted && req.backup_password.as_ref().map_or(true, |p| p.is_empty()) {
        return HttpResponse::Ok().json(StartRestoreResponse {
            task_id: None,
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
            message: None,
        });
    }

    let config = RestoreConfig {
        user_id,
        storage_type: req.storage_type.clone(),
        storage_config: req.storage_config.clone(),
        encrypted: req.encrypted,
        backup_password: req.backup_password.clone(),
        target_path: full_path,
    };

    match app_state.restore_manager.start_task(&config).await {
        Ok(task_id) => HttpResponse::Ok().json(StartRestoreResponse {
            task_id: Some(task_id),
            success: true,
            fail_code: None,
            message: None,
        }),
        Err(e) => {
            crate::error_logger::log_error("/api/restore/start", &e.message.as_deref().unwrap_or(e.fail_code));
            HttpResponse::Ok().json(StartRestoreResponse {
                task_id: None,
                success: false,
                fail_code: Some(e.fail_code.to_string()),
                message: e.message,
            })
        }
    }
}

/// 获取恢复进度请求
#[derive(Debug, Deserialize)]
pub struct GetRestoreProgressRequest {
    pub task_id: String,
}

/// 获取恢复进度
pub async fn get_restore_progress(
    req: web::Json<GetRestoreProgressRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    match app_state.restore_manager.get_task_progress(&req.task_id, &user_id).await {
        Ok(progress) => {
            match serde_json::to_value(&progress) {
                Ok(json_value) => HttpResponse::Ok().json(json_value),
                Err(e) => internal_error_response("/api/restore/progress", &format!("JSON serialization failed: {}", e)),
            }
        }
        Err(e) if e == "TASK_NOT_RUNNING" => {
            // 返回一个空的进度响应，表示任务未运行
            HttpResponse::Ok().json(serde_json::json!({
                "is_running": false,
                "downloading_items": [],
                "failed_items": [],
                "pending_count": 0,
                "total_count": 0,
                "success_count": 0,
                "downloaded_bytes": 0
            }))
        }
        Err(e) => internal_error_response("/api/restore/progress", &e),
    }
}

/// 取消恢复请求
#[derive(Debug, Deserialize)]
pub struct CancelRestoreRequest {
    pub task_id: String,
}

/// 取消恢复任务
pub async fn cancel_restore(
    req: web::Json<CancelRestoreRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    match app_state.restore_manager.cancel_task(&req.task_id, &user_id).await {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        }),
        Err(e) if e == "TASK_NOT_RUNNING" => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("TASK_NOT_RUNNING".to_string()),
        }),
        Err(e) => internal_error_response("/api/restore/cancel", &e),
    }
}

/// 重试失败文件请求
#[derive(Debug, Deserialize)]
pub struct RetryRestoreFileRequest {
    pub task_id: String,
    pub file_path: String,
}

/// 重试失败文件
pub async fn retry_restore_file(
    req: web::Json<RetryRestoreFileRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    match app_state.restore_manager.retry_file(&req.task_id, &req.file_path, &user_id).await {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        }),
        Err(e) if e == "TASK_NOT_RUNNING" => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("TASK_NOT_RUNNING".to_string()),
        }),
        Err(e) if e == "FILE_NOT_IN_FAILED_LIST" => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("FILE_NOT_IN_FAILED_LIST".to_string()),
        }),
        Err(e) if e == "ALREADY_RETRYING" => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("ALREADY_RETRYING".to_string()),
        }),
        Err(e) => internal_error_response("/api/restore/retry_file", &e),
    }
}
