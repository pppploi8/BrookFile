use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;
use std::sync::{Mutex, OnceLock};
use crate::app_state::AppState;
use crate::handlers::{ApiResponse, internal_error_response, is_recycle_bin_path_under_root};

fn init_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Debug, Deserialize)]
pub struct InitRequest {
    username: String,
    password: String,
    system_name: String,
    root_path: String,
    recycle_bin_path: Option<String>,
}

pub async fn init_system(
    req: web::Json<InitRequest>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let _lock = init_lock().lock().unwrap_or_else(|e| e.into_inner());
    match app_state.is_initialized() {
        Ok(true) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("SYSTEM_ALREADY_INITIALIZED".to_string()),
            });
        }
        Ok(false) => {}
        Err(e) => return internal_error_response("/api/system/init", &e),
    }

    if req.root_path.trim().is_empty() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("ROOT_PATH_EMPTY".to_string()),
        });
    }

    let root_path_obj = std::path::Path::new(&req.root_path);
    if !root_path_obj.is_absolute() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("ROOT_PATH_EMPTY".to_string()),
        });
    }

    if req.root_path.contains("..") {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("ROOT_PATH_EMPTY".to_string()),
        });
    }

    if req.system_name.trim().is_empty() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
        });
    }

    if req.username.is_empty() || req.password.is_empty() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
        });
    }

    if req.username.len() > 64 || req.password.len() > 128 {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
        });
    }

    if req.password.len() < 8 {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PASSWORD_TOO_SHORT".to_string()),
        });
    }

    let recycle_bin_path = req.recycle_bin_path.as_deref().filter(|s| !s.trim().is_empty());

    if let Some(rbp) = recycle_bin_path {
        let rbp_path = std::path::Path::new(rbp);
        if !rbp_path.is_absolute() || rbp.contains("..") {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("RECYCLE_BIN_PATH_INVALID".to_string()),
            });
        }
        if is_recycle_bin_path_under_root(rbp, &req.root_path) {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("RECYCLE_BIN_PATH_INVALID".to_string()),
            });
        }
    }

    match app_state.user_model.create_user(&req.username, &req.password, &req.root_path, recycle_bin_path, true) {
        Ok(_) => {
            if let Err(e) = app_state.system_config_model.set_config("system_name", &req.system_name) {
                let _ = app_state.user_model.delete_user_by_username(&req.username);
                return internal_error_response("/api/system/init", &e);
            }
            if let Err(e) = app_state.set_initialized(true) {
                let _ = app_state.user_model.delete_user_by_username(&req.username);
                let _ = app_state.system_config_model.delete_config("system_name");
                return internal_error_response("/api/system/init", &e);
            }
            HttpResponse::Ok().json(ApiResponse {
                success: true,
                fail_code: None,
            })
        }
        Err(e) => {
            internal_error_response("/api/system/init", &e)
        }
    }
}
