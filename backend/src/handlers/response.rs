use serde::{Deserialize, Serialize};
use actix_web::{web, HttpRequest, HttpResponse};
use crate::app_state::AppState;
use crate::error_logger;
use crate::middleware::get_session_id;

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
}

pub fn internal_error_response(api_path: &str, message: &str) -> HttpResponse {
    error_logger::log_error(api_path, message);
    HttpResponse::Ok().json(ApiResponse {
        success: false,
        fail_code: Some("INTERNAL_ERROR".to_string()),
    })
}

pub fn get_current_user_id(http_req: &HttpRequest, app_state: &web::Data<AppState>) -> Result<String, HttpResponse> {
    let session_id = match get_session_id(http_req) {
        Some(id) => id,
        None => {
            return Err(HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("NOT_LOGGED_IN".to_string()),
            }));
        }
    };

    let user_id = app_state.session_manager.get(&session_id, "user_id");
    match user_id {
        Some(id) => Ok(id),
        None => Err(HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("NOT_LOGGED_IN".to_string()),
        })),
    }
}

/// 获取当前用户的根路径
pub fn get_user_root_path(http_req: &HttpRequest, app_state: &web::Data<AppState>) -> Result<String, HttpResponse> {
    let session_id = match get_session_id(http_req) {
        Some(id) => id,
        None => {
            return Err(HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("NOT_LOGGED_IN".to_string()),
            }));
        }
    };

    let root_path = match app_state.session_manager.get(&session_id, "root_path") {
        Some(path) => path,
        None => {
            return Err(HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("NO_ROOT_PATH".to_string()),
            }));
        }
    };

    Ok(root_path)
}

/// 检查当前用户是否为管理员
pub fn check_admin(http_req: &HttpRequest, app_state: &web::Data<AppState>) -> Result<String, HttpResponse> {
    let session_id = match get_session_id(http_req) {
        Some(id) => id,
        None => {
            return Err(HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("NOT_LOGGED_IN".to_string()),
            }));
        }
    };

    let is_admin = app_state.session_manager.get(&session_id, "is_admin");
    match is_admin.as_deref() {
        Some("true") => {
            let user_id = app_state.session_manager.get(&session_id, "user_id");
            match user_id {
                Some(id) => Ok(id),
                None => Err(HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("NOT_LOGGED_IN".to_string()),
                })),
            }
        }
        _ => Err(HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PERMISSION_DENIED".to_string()),
        })),
    }
}
