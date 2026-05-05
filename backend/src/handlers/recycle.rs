use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::path::Path;
use crate::app_state::AppState;
use crate::handlers::{ApiResponse, get_current_user_id, get_user_root_path, internal_error_response, is_path_under_root, move_recursive};
use crate::handlers::file::{get_recycle_bin_path, cleanup_search_index_on_restore};

#[derive(Debug, Deserialize)]
pub struct RecycleListRequest {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct RecycleItemResponse {
    pub id: String,
    pub original_path: String,
    pub original_name: String,
    pub is_directory: bool,
    pub file_size: i64,
    pub deleted_at: String,
}

#[derive(Debug, Serialize)]
pub struct RecycleListApiResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<RecycleListData>,
}

#[derive(Debug, Serialize)]
pub struct RecycleListData {
    pub items: Vec<RecycleItemResponse>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Deserialize)]
pub struct RecycleRestoreRequest {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct RecycleBatchRestoreRequest {
    pub ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct RecycleDeleteRequest {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct RecycleBatchDeleteRequest {
    pub ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct RecycleBatchRestoreResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<BatchRestoreData>,
}

#[derive(Debug, Serialize)]
pub struct BatchRestoreData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict_items: Option<Vec<ConflictItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_paths: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct ConflictItem {
    pub id: String,
    pub original_path: String,
    pub original_name: String,
    pub is_directory: bool,
}

pub async fn list_recycle_bin(
    body: web::Json<RecycleListRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let recycle_bin_path = match get_recycle_bin_path(&user_id, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };
    if recycle_bin_path.is_none() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("RECYCLE_NOT_ENABLED".to_string()),
        });
    }

    let page = body.page.unwrap_or(1).max(1);
    let page_size = body.page_size.unwrap_or(20).max(1).min(1000);

    match app_state.recycle_bin_model.list(&user_id, page, page_size) {
        Ok((items, total)) => {
            let response_items: Vec<RecycleItemResponse> = items.iter().map(|item| {
                RecycleItemResponse {
                    id: item.id.clone(),
                    original_path: item.original_path.clone(),
                    original_name: item.original_name.clone(),
                    is_directory: item.is_directory,
                    file_size: item.file_size,
                    deleted_at: item.deleted_at.clone(),
                }
            }).collect();

            HttpResponse::Ok().json(RecycleListApiResponse {
                success: true,
                fail_code: None,
                data: Some(RecycleListData {
                    items: response_items,
                    total,
                    page,
                    page_size,
                }),
            })
        }
        Err(e) => internal_error_response("/api/recycle/list", &e),
    }
}

pub async fn restore_recycle_item(
    body: web::Json<RecycleRestoreRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let recycle_bin_path = match get_recycle_bin_path(&user_id, &app_state) {
        Ok(Some(p)) => p,
        Ok(None) => return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("RECYCLE_NOT_ENABLED".to_string()),
        }),
        Err(resp) => return resp,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };

    let item = match app_state.recycle_bin_model.get_by_id(&body.id, &user_id) {
        Ok(Some(item)) => item,
        Ok(None) => return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("RECYCLE_ITEM_NOT_FOUND".to_string()),
        }),
        Err(e) => return internal_error_response("/api/recycle/restore", &e),
    };

    let target_path = Path::new(&root_path).join(&item.original_path);
    if !is_path_under_root(&target_path, Path::new(&root_path)) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("RESTORE_FAILED".to_string()),
        });
    }

    if target_path.exists() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("RESTORE_PATH_OCCUPIED".to_string()),
        });
    }

    if let Some(parent) = target_path.parent() {
        if let Err(_) = std::fs::create_dir_all(parent) {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("RESTORE_FAILED".to_string()),
            });
        }
    }

    let source_path = Path::new(&recycle_bin_path).join(&item.id).join(&item.original_name);
    if let Err(_) = move_recursive(&source_path, &target_path) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("RESTORE_FAILED".to_string()),
        });
    }

    if let Err(e) = app_state.recycle_bin_model.delete_by_id(&item.id, &user_id) {
        let _ = move_recursive(&target_path, &source_path);
        return internal_error_response("/api/recycle/restore", &e);
    }

    let record_dir = Path::new(&recycle_bin_path).join(&item.id);
    let _ = std::fs::remove_dir_all(&record_dir);

    cleanup_search_index_on_restore(&item.original_path, &user_id, &app_state);

    HttpResponse::Ok().json(ApiResponse {
        success: true,
        fail_code: None,
    })
}

pub async fn batch_restore_recycle_items(
    body: web::Json<RecycleBatchRestoreRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let recycle_bin_path = match get_recycle_bin_path(&user_id, &app_state) {
        Ok(Some(p)) => p,
        Ok(None) => return HttpResponse::Ok().json(RecycleBatchRestoreResponse {
            success: false,
            fail_code: Some("RECYCLE_NOT_ENABLED".to_string()),
            data: None,
        }),
        Err(resp) => return resp,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };

    if body.ids.is_empty() {
        return HttpResponse::Ok().json(RecycleBatchRestoreResponse {
            success: true,
            fail_code: None,
            data: None,
        });
    }

    let items = match app_state.recycle_bin_model.get_by_ids(&body.ids, &user_id) {
        Ok(items) => items,
        Err(e) => return internal_error_response("/api/recycle/batch_restore", &e),
    };

    if items.len() != body.ids.len() {
        return HttpResponse::Ok().json(RecycleBatchRestoreResponse {
            success: false,
            fail_code: Some("RECYCLE_ITEM_NOT_FOUND".to_string()),
            data: None,
        });
    }

    let mut conflict_items = Vec::new();
    {
        let mut seen_paths = std::collections::HashSet::new();
        for item in &items {
            if !seen_paths.insert(&item.original_path) {
                if !conflict_items.iter().any(|c: &ConflictItem| c.original_path == item.original_path) {
                    conflict_items.push(ConflictItem {
                        id: item.id.clone(),
                        original_path: item.original_path.clone(),
                        original_name: item.original_name.clone(),
                        is_directory: item.is_directory,
                    });
                }
            }
        }
        for item in &items {
            let target_path = Path::new(&root_path).join(&item.original_path);
            if is_path_under_root(&target_path, Path::new(&root_path)) && target_path.exists() {
                if !conflict_items.iter().any(|c| c.id == item.id) {
                    conflict_items.push(ConflictItem {
                        id: item.id.clone(),
                        original_path: item.original_path.clone(),
                        original_name: item.original_name.clone(),
                        is_directory: item.is_directory,
                    });
                }
            }
        }
    }

    if !conflict_items.is_empty() {
        return HttpResponse::Ok().json(RecycleBatchRestoreResponse {
            success: false,
            fail_code: Some("RESTORE_PATH_OCCUPIED".to_string()),
            data: Some(BatchRestoreData {
                conflict_items: Some(conflict_items),
                failed_paths: None,
            }),
        });
    }

    let mut failed_paths: Vec<String> = Vec::new();

    for item in &items {
        let target_path = Path::new(&root_path).join(&item.original_path);

        if !is_path_under_root(&target_path, Path::new(&root_path)) {
            failed_paths.push(item.original_path.clone());
            continue;
        }

        if let Some(parent) = target_path.parent() {
            if let Err(_) = std::fs::create_dir_all(parent) {
                failed_paths.push(item.original_path.clone());
                continue;
            }
        }

        let source_path = Path::new(&recycle_bin_path).join(&item.id).join(&item.original_name);
        if let Err(_) = move_recursive(&source_path, &target_path) {
            failed_paths.push(item.original_path.clone());
            continue;
        }

        if let Err(_) = app_state.recycle_bin_model.delete_by_id(&item.id, &user_id) {
            let _ = move_recursive(&target_path, &source_path);
            failed_paths.push(item.original_path.clone());
            continue;
        }

        let record_dir = Path::new(&recycle_bin_path).join(&item.id);
        let _ = std::fs::remove_dir_all(&record_dir);

        cleanup_search_index_on_restore(&item.original_path, &user_id, &app_state);
    }

    if !failed_paths.is_empty() {
        return HttpResponse::Ok().json(RecycleBatchRestoreResponse {
            success: false,
            fail_code: Some("RESTORE_FAILED".to_string()),
            data: Some(BatchRestoreData {
                conflict_items: None,
                failed_paths: Some(failed_paths),
            }),
        });
    }

    HttpResponse::Ok().json(RecycleBatchRestoreResponse {
        success: true,
        fail_code: None,
        data: None,
    })
}

pub async fn delete_recycle_item(
    body: web::Json<RecycleDeleteRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let recycle_bin_path = match get_recycle_bin_path(&user_id, &app_state) {
        Ok(Some(p)) => p,
        Ok(None) => return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("RECYCLE_NOT_ENABLED".to_string()),
        }),
        Err(resp) => return resp,
    };

    let item = match app_state.recycle_bin_model.get_by_id(&body.id, &user_id) {
        Ok(Some(item)) => item,
        Ok(None) => return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("RECYCLE_ITEM_NOT_FOUND".to_string()),
        }),
        Err(e) => return internal_error_response("/api/recycle/delete", &e),
    };

    let record_dir = Path::new(&recycle_bin_path).join(&item.id);
    if record_dir.exists() {
        if let Err(_) = std::fs::remove_dir_all(&record_dir) {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("DELETE_FAILED".to_string()),
            });
        }
    }

    if let Err(e) = app_state.recycle_bin_model.delete_by_id(&item.id, &user_id) {
        return internal_error_response("/api/recycle/delete", &e);
    }

    HttpResponse::Ok().json(ApiResponse {
        success: true,
        fail_code: None,
    })
}

pub async fn batch_delete_recycle_items(
    body: web::Json<RecycleBatchDeleteRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let recycle_bin_path = match get_recycle_bin_path(&user_id, &app_state) {
        Ok(Some(p)) => p,
        Ok(None) => return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("RECYCLE_NOT_ENABLED".to_string()),
        }),
        Err(resp) => return resp,
    };

    let items = match app_state.recycle_bin_model.get_by_ids(&body.ids, &user_id) {
        Ok(items) => items,
        Err(e) => return internal_error_response("/api/recycle/batch_delete", &e),
    };

    if items.len() != body.ids.len() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("RECYCLE_ITEM_NOT_FOUND".to_string()),
        });
    }

    let mut failed_ids: Vec<String> = Vec::new();
    for item in &items {
        let record_dir = Path::new(&recycle_bin_path).join(&item.id);
        if record_dir.exists() {
            if std::fs::remove_dir_all(&record_dir).is_err() {
                failed_ids.push(item.original_path.clone());
                continue;
            }
        }

        if let Err(_) = app_state.recycle_bin_model.delete_by_id(&item.id, &user_id) {
            failed_ids.push(item.original_path.clone());
            continue;
        }
    }

    if !failed_ids.is_empty() {
        return HttpResponse::Ok().json(RecycleBatchRestoreResponse {
            success: false,
            fail_code: Some("DELETE_FAILED".to_string()),
            data: Some(BatchRestoreData {
                conflict_items: None,
                failed_paths: Some(failed_ids),
            }),
        });
    }

    HttpResponse::Ok().json(ApiResponse {
        success: true,
        fail_code: None,
    })
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct EmptyRecycleBinRequest {}

pub async fn empty_recycle_bin(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let recycle_bin_path = match get_recycle_bin_path(&user_id, &app_state) {
        Ok(Some(p)) => p,
        Ok(None) => return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("RECYCLE_NOT_ENABLED".to_string()),
        }),
        Err(resp) => return resp,
    };

    let page_size: i64 = 10000;
    let mut page: i64 = 1;
    let mut all_items: Vec<crate::models::recycle_bin::RecycleBinItem> = Vec::new();
    loop {
        let (items, total) = match app_state.recycle_bin_model.list(&user_id, page, page_size) {
            Ok(result) => result,
            Err(e) => return internal_error_response("/api/recycle/empty", &e),
        };

        all_items.extend(items);

        if all_items.len() < page_size as usize || page * page_size >= total {
            break;
        }
        page += 1;
    }

    let mut failed = false;
    for item in &all_items {
        let record_dir = Path::new(&recycle_bin_path).join(&item.id);
        if record_dir.exists() {
            if std::fs::remove_dir_all(&record_dir).is_err() {
                failed = true;
                continue;
            }
        }
        if let Err(e) = app_state.recycle_bin_model.delete_by_id(&item.id, &user_id) {
            crate::error_logger::log_error("/api/recycle/empty", &format!("DB delete failed for item {}: {}", item.id, e));
            failed = true;
        }
    }

    if failed {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("DELETE_FAILED".to_string()),
        });
    }

    HttpResponse::Ok().json(ApiResponse {
        success: true,
        fail_code: None,
    })
}
