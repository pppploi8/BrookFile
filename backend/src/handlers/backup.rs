use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use crate::app_state::AppState;
use crate::handlers::{ApiResponse, internal_error_response, get_current_user_id};
use crate::models::{BackupRuleListItem, BackupRuleDetail, CreateBackupRuleData, UpdateBackupRuleData, BackupLogItem};

#[derive(Serialize)]
pub struct BackupRuleListItemResponse {
    pub id: String,
    pub name: String,
    pub storage_type: String,
    pub local_path: String,
    pub cycle: String,
    pub backup_time: serde_json::Value,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_backup_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_backup_time: Option<String>,
    pub created_at: Option<String>,
}

impl From<BackupRuleListItem> for BackupRuleListItemResponse {
    fn from(item: BackupRuleListItem) -> Self {
        BackupRuleListItemResponse {
            id: item.id,
            name: item.name,
            storage_type: item.storage_type,
            local_path: item.local_path,
            cycle: item.cycle,
            backup_time: item.backup_time,
            status: item.status,
            next_backup_time: item.next_backup_time,
            last_backup_time: item.last_backup_time,
            created_at: item.created_at,
        }
    }
}

#[derive(Serialize)]
pub struct BackupRuleDetailResponse {
    pub id: String,
    pub name: String,
    pub storage_type: String,
    pub storage_config: serde_json::Value,
    pub local_path: String,
    pub encrypted: bool,
    pub cycle: String,
    pub backup_time: serde_json::Value,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_backup_time: Option<String>,
    pub created_at: Option<String>,
}

impl BackupRuleDetailResponse {
    pub fn from_detail(detail: BackupRuleDetail) -> Self {
        BackupRuleDetailResponse {
            id: detail.id,
            name: detail.name,
            storage_type: detail.storage_type,
            storage_config: detail.storage_config,
            local_path: detail.local_path,
            encrypted: detail.encrypted,
            cycle: detail.cycle,
            backup_time: detail.backup_time,
            status: detail.status,
            last_backup_time: detail.last_backup_time,
            created_at: detail.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateBackupRuleRequest {
    pub name: String,
    pub storage_type: String,
    pub storage_config: serde_json::Value,
    pub local_path: String,
    #[serde(default)]
    pub encrypted: bool,
    pub backup_password: Option<String>,
    pub cycle: String,
    pub backup_time: serde_json::Value,
}

impl From<CreateBackupRuleRequest> for CreateBackupRuleData {
    fn from(req: CreateBackupRuleRequest) -> Self {
        CreateBackupRuleData {
            name: req.name,
            storage_type: req.storage_type,
            storage_config: req.storage_config,
            local_path: req.local_path,
            encrypted: req.encrypted,
            backup_password: req.backup_password,
            cycle: req.cycle,
            backup_time: req.backup_time,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateBackupRuleRequest {
    pub id: String,
    pub name: String,
    pub storage_type: String,
    pub storage_config: serde_json::Value,
    pub local_path: String,
    #[serde(default)]
    pub encrypted: bool,
    pub backup_password: Option<String>,
    pub cycle: String,
    pub backup_time: serde_json::Value,
}

impl From<UpdateBackupRuleRequest> for UpdateBackupRuleData {
    fn from(req: UpdateBackupRuleRequest) -> Self {
        UpdateBackupRuleData {
            id: req.id,
            name: req.name,
            storage_type: req.storage_type,
            storage_config: req.storage_config,
            local_path: req.local_path,
            encrypted: req.encrypted,
            backup_password: req.backup_password,
            cycle: req.cycle,
            backup_time: req.backup_time,
        }
    }
}

pub async fn list_backup_rules(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    match app_state.backup_rule_model.list_by_user(&user_id) {
        Ok(rules) => {
            let mut responses: Vec<BackupRuleListItemResponse> = Vec::with_capacity(rules.len());
            for mut rule in rules {
                if app_state.backup_manager.is_task_running(&rule.id).await {
                    rule.status = "running".to_string();
                } else {
                    rule.status = app_state.backup_rule_model.get_last_log_status(&rule.id).unwrap_or_else(|| "idle".to_string());
                }
                if let Some(next_time) = app_state.backup_scheduler.get_next_run_time(&rule.id).await {
                    rule.next_backup_time = Some(next_time);
                }
                responses.push(rule.into());
            }
            HttpResponse::Ok().json(&responses)
        }
        Err(e) => internal_error_response("/api/backup/list", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct GetBackupRuleRequest {
    id: String,
}

pub async fn get_backup_rule(
    req: web::Json<GetBackupRuleRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    match app_state.backup_rule_model.get_by_id(&req.id) {
        Ok(Some(mut rule)) => {
            if rule.user_id != user_id {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("BACKUP_RULE_NOT_FOUND".to_string()),
                });
            }
            if app_state.backup_manager.is_task_running(&rule.id).await {
                rule.status = "running".to_string();
            } else {
                rule.status = app_state.backup_rule_model.get_last_log_status(&rule.id).unwrap_or_else(|| "idle".to_string());
            }
            if let Some(obj) = rule.storage_config.as_object_mut() {
                obj.remove("password");
            }
            let response = BackupRuleDetailResponse::from_detail(rule);
            HttpResponse::Ok().json(&response)
        }
        Ok(None) => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("BACKUP_RULE_NOT_FOUND".to_string()),
        }),
        Err(e) => internal_error_response("/api/backup/get", &e),
    }
}

pub async fn create_backup_rule(
    req: web::Json<CreateBackupRuleRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let data: CreateBackupRuleData = req.into_inner().into();
    let cycle = data.cycle.clone();
    let backup_time = data.backup_time.clone();
    
    match app_state.backup_rule_model.create(&user_id, &data) {
        Ok(rule_id) => {
            app_state.backup_scheduler.schedule_rule(&rule_id, &cycle, backup_time).await;
            HttpResponse::Ok().json(ApiResponse {
                success: true,
                fail_code: None,
            })
        }
        Err(e) if e == "INVALID_PARAM" => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
        }),
        Err(e) if e == "NAME_EMPTY" => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("NAME_EMPTY".to_string()),
        }),
        Err(e) if e == "LOCAL_PATH_EMPTY" => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("LOCAL_PATH_EMPTY".to_string()),
        }),
        Err(e) if e == "INVALID_STORAGE_TYPE" => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_STORAGE_TYPE".to_string()),
        }),
        Err(e) if e == "INVALID_CYCLE" => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_CYCLE".to_string()),
        }),
        Err(e) => internal_error_response("/api/backup/create", &e),
    }
}

pub async fn update_backup_rule(
    req: web::Json<UpdateBackupRuleRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let data: UpdateBackupRuleData = req.into_inner().into();
    let rule_id = data.id.clone();
    let cycle = data.cycle.clone();
    let backup_time = data.backup_time.clone();

    match app_state.backup_rule_model.get_by_id(&rule_id) {
        Ok(Some(rule)) => {
            if rule.user_id != user_id {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("BACKUP_RULE_NOT_FOUND".to_string()),
                });
            }
        }
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("BACKUP_RULE_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/backup/update", &e),
    }

    if app_state.backup_manager.is_task_running(&rule_id).await {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("BACKUP_RUNNING".to_string()),
        });
    }

    match app_state.backup_rule_model.update(&user_id, &data) {
        Ok(true) => {
            app_state.backup_scheduler.schedule_rule(&rule_id, &cycle, backup_time).await;
            HttpResponse::Ok().json(ApiResponse {
                success: true,
                fail_code: None,
            })
        }
        Ok(false) => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("BACKUP_RULE_NOT_FOUND".to_string()),
        }),
        Err(e) if e == "INVALID_PARAM" || e == "NAME_EMPTY" || e == "LOCAL_PATH_EMPTY"
            || e == "INVALID_STORAGE_TYPE" || e == "INVALID_CYCLE" => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
        }),
        Err(e) => internal_error_response("/api/backup/update", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct DeleteBackupRuleRequest {
    id: String,
}

pub async fn delete_backup_rule(
    req: web::Json<DeleteBackupRuleRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    match app_state.backup_rule_model.get_by_id(&req.id) {
        Ok(Some(rule)) => {
            if rule.user_id != user_id {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("BACKUP_RULE_NOT_FOUND".to_string()),
                });
            }
        }
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("BACKUP_RULE_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/backup/delete", &e),
    }

    if app_state.backup_manager.is_task_running(&req.id).await {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("BACKUP_RUNNING".to_string()),
        });
    }

    match app_state.backup_rule_model.delete(&req.id, &user_id) {
        Ok(true) => {
            app_state.backup_scheduler.remove_rule(&req.id).await;
            HttpResponse::Ok().json(ApiResponse {
                success: true,
                fail_code: None,
            })
        }
        Ok(false) => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("BACKUP_RULE_NOT_FOUND".to_string()),
        }),
        Err(e) => internal_error_response("/api/backup/delete", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct StartBackupRequest {
    rule_id: String,
    mode: String,
}

#[derive(serde::Serialize)]
pub struct StartBackupResponse {
    success: bool,
    task_id: String,
}

pub async fn start_backup(
    req: web::Json<StartBackupRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    match app_state.backup_rule_model.get_by_id(&req.rule_id) {
        Ok(Some(rule)) => {
            if rule.user_id != user_id {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("BACKUP_RULE_NOT_FOUND".to_string()),
                });
            }
        }
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("BACKUP_RULE_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/backup/start", &e),
    };

    if req.mode != "full" && req.mode != "cleanup_only" {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
        });
    }

    match app_state.backup_manager.start_task(&req.rule_id, &req.mode).await {
        Ok(task_id) => HttpResponse::Ok().json(StartBackupResponse { success: true, task_id }),
        Err(e) if e == "TASK_ALREADY_RUNNING" => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("TASK_ALREADY_RUNNING".to_string()),
        }),
        Err(e) => internal_error_response("/api/backup/start", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct CancelBackupRequest {
    rule_id: String,
}

pub async fn cancel_backup(
    req: web::Json<CancelBackupRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    match app_state.backup_rule_model.get_by_id(&req.rule_id) {
        Ok(Some(rule)) => {
            if rule.user_id != user_id {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("BACKUP_RULE_NOT_FOUND".to_string()),
                });
            }
        }
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("BACKUP_RULE_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/backup/cancel", &e),
    };

    match app_state.backup_manager.cancel_task(&req.rule_id).await {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        }),
        Err(e) if e == "TASK_NOT_RUNNING" => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("TASK_NOT_RUNNING".to_string()),
        }),
        Err(e) => internal_error_response("/api/backup/cancel", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct GetBackupProgressRequest {
    rule_id: String,
}

#[derive(serde::Serialize)]
pub struct BackupProgressResponse {
    is_running: bool,
    phase: String,
    sub_phase: Option<String>,
    pending_items: Vec<serde_json::Value>,
    total_count: u64,
    scanned_bytes: u64,
}

pub async fn get_backup_progress(
    req: web::Json<GetBackupProgressRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    // 验证备份规则是否存在
    match app_state.backup_rule_model.get_by_id(&req.rule_id) {
        Ok(Some(rule)) => {
            if rule.user_id != user_id {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("BACKUP_RULE_NOT_FOUND".to_string()),
                });
            }
        }
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("BACKUP_RULE_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/backup/progress", &e),
    }

    match app_state.backup_manager.get_task_progress(&req.rule_id).await {
        Ok(progress) => {
            match serde_json::to_value(&progress) {
                Ok(json_value) => HttpResponse::Ok().json(json_value),
                Err(e) => {
                    crate::error_logger::log_error("/api/backup/progress", &format!("JSON serialization failed: {}", e));
                    internal_error_response("/api/backup/progress", &format!("JSON serialization failed: {}", e))
                }
            }
        }
        Err(e) if e == "TASK_NOT_RUNNING" => HttpResponse::Ok().json(BackupProgressResponse {
            is_running: false,
            phase: "backup".to_string(),
            sub_phase: None,
            pending_items: vec![],
            total_count: 0,
            scanned_bytes: 0,
        }),
        Err(e) => internal_error_response("/api/backup/progress", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct GetBackupLogsRequest {
    rule_id: String,
    page: u32,
    page_size: u32,
}

#[derive(serde::Serialize)]
pub struct GetBackupLogsResponse {
    total: u32,
    page: u32,
    page_size: u32,
    items: Vec<BackupLogItem>,
}

pub async fn get_backup_logs(
    req: web::Json<GetBackupLogsRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let _rule = match app_state.backup_rule_model.get_by_id(&req.rule_id) {
        Ok(Some(r)) => {
            if r.user_id != user_id {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("BACKUP_RULE_NOT_FOUND".to_string()),
                });
            }
            r
        }
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("BACKUP_RULE_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/backup/logs", &e),
    };

    let (items, total) = match app_state.backup_rule_model.list_logs_by_rule(&req.rule_id, req.page, req.page_size) {
        Ok(result) => result,
        Err(e) => return internal_error_response("/api/backup/logs", &e),
    };

    HttpResponse::Ok().json(GetBackupLogsResponse {
        total,
        page: req.page,
        page_size: req.page_size,
        items,
    })
}
