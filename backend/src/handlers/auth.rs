use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use chrono::{DateTime, Utc};
use crate::app_state::AppState;
use crate::handlers::ApiResponse;
use crate::middleware::get_session_id;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

fn validate_login_input(username: &str, password: &str) -> Option<&'static str> {
    if username.is_empty() || password.is_empty() {
        return Some("INVALID_USERNAME_OR_PASSWORD");
    }
    if username.len() > 64 || password.len() > 128 {
        return Some("INVALID_USERNAME_OR_PASSWORD");
    }
    None
}

pub async fn login(
    req: web::Json<LoginRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    if let Some(fail_code) = validate_login_input(&req.username, &req.password) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some(fail_code.to_string()),
        });
    }

    let delay_secs = app_state.session_manager.get_login_delay(&req.username);
    if delay_secs > 0 {
        tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
    }

    match app_state.user_model.verify_password(&req.username, &req.password) {
        Ok(true) => {
            match app_state.user_model.get_user_by_username(&req.username) {
                Ok(Some(user)) => {
                    if let Some(expire_at) = &user.expire_at {
                        match DateTime::parse_from_rfc3339(expire_at) {
                            Ok(expire_time) => {
                                if expire_time.with_timezone(&Utc) < Utc::now() {
                                    return HttpResponse::Ok().json(ApiResponse {
                                        success: false,
                                        fail_code: Some("ACCOUNT_EXPIRED".to_string()),
                                    });
                                }
                            }
                            Err(e) => {
                                crate::error_logger::log_error("/api/auth/login", &format!("Failed to parse expire_at: {}", e));
                                return HttpResponse::Ok().json(ApiResponse {
                                    success: false,
                                    fail_code: Some("INTERNAL_ERROR".to_string()),
                                });
                            }
                        }
                    }
                    
                    app_state.session_manager.clear_login_failures(&req.username);
                    
                    let session_id = match get_session_id(&http_req) {
                        Some(id) => id,
                        None => {
                            return HttpResponse::Ok().json(ApiResponse {
                                success: false,
                                fail_code: Some("INTERNAL_ERROR".to_string()),
                            });
                        }
                    };

                    let new_session_id = match app_state.session_manager.regenerate(&session_id) {
                        Some(id) => id,
                        None => {
                            return HttpResponse::Ok().json(ApiResponse {
                                success: false,
                                fail_code: Some("INTERNAL_ERROR".to_string()),
                            });
                        }
                    };

                    app_state.session_manager.set(&new_session_id, "user_id", &user.id);
                    app_state.session_manager.set(&new_session_id, "username", &req.username);
                    app_state.session_manager.set(&new_session_id, "is_admin", if user.is_admin { "true" } else { "false" });
                    if let Some(root_path) = &user.root_path {
                        app_state.session_manager.set(&new_session_id, "root_path", root_path);
                    }

                    HttpResponse::Ok().json(ApiResponse {
                        success: true,
                        fail_code: None,
                    })
                }
                Ok(None) => {
                    crate::error_logger::log_error("/api/auth/login", "User verified but not found by username");
                    HttpResponse::Ok().json(ApiResponse {
                        success: false,
                        fail_code: Some("INTERNAL_ERROR".to_string()),
                    })
                }
                Err(e) => {
                    crate::error_logger::log_error("/api/auth/login", &e.to_string());
                    HttpResponse::Ok().json(ApiResponse {
                        success: false,
                        fail_code: Some("INTERNAL_ERROR".to_string()),
                    })
                }
            }
        }
        Ok(false) => {
            app_state.session_manager.record_login_failure(&req.username);
            HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INVALID_USERNAME_OR_PASSWORD".to_string()),
            })
        }
        Err(e) => {
            crate::error_logger::log_error("/api/auth/login", &e.to_string());
            HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INTERNAL_ERROR".to_string()),
            })
        }
    }
}

pub async fn ping() -> impl Responder {
    HttpResponse::Ok().json(ApiResponse {
        success: true,
        fail_code: None,
    })
}

pub async fn logout(http_req: HttpRequest, app_state: web::Data<AppState>) -> impl Responder {
    if let Some(session_id) = get_session_id(&http_req) {
        app_state.session_manager.invalidate(&session_id);
    }
    
    HttpResponse::Ok().json(ApiResponse {
        success: true,
        fail_code: None,
    })
}
