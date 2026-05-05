use actix_web::{web, HttpRequest, HttpResponse, Responder};
use actix_multipart::Multipart;
use futures_util::StreamExt;
use serde::{Deserialize, Deserializer, Serialize};
use std::path::{Path, PathBuf};
use std::fs;
use crate::app_state::AppState;
use crate::handlers::{ApiResponse, internal_error_response, get_current_user_id, check_admin, is_safe_name, is_recycle_bin_path_under_root};
use crate::middleware::get_session_id;
use crate::models::UserInfo;
use crate::error_logger;

const AVATAR_EXTENSIONS: [&str; 5] = ["jpg", "jpeg", "png", "gif", "webp"];

fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: String,
    pub username: String,
    pub root_path: Option<String>,
    pub recycle_bin_path: Option<String>,
    pub is_admin: bool,
    pub expire_at: Option<String>,
    pub remark: Option<String>,
    pub feature_order: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl From<UserInfo> for UserResponse {
    fn from(user: UserInfo) -> Self {
        UserResponse {
            id: user.id,
            username: user.username,
            root_path: user.root_path,
            recycle_bin_path: user.recycle_bin_path,
            is_admin: user.is_admin,
            expire_at: user.expire_at,
            remark: user.remark,
            feature_order: user.feature_order,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct GetUserRequest {
    id: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    id: String,
    password: Option<String>,
    root_path: Option<String>,
    #[serde(default, deserialize_with = "deserialize_some")]
    recycle_bin_path: Option<Option<String>>,
    is_admin: Option<bool>,
    expire_at: Option<String>,
    remark: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteUserRequest {
    id: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    username: String,
    password: String,
    root_path: Option<String>,
    recycle_bin_path: Option<String>,
    is_admin: Option<bool>,
    expire_at: Option<String>,
    remark: Option<String>,
}

pub async fn list_users(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    if let Err(response) = check_admin(&http_req, &app_state) {
        return response;
    }

    match app_state.user_model.list_users() {
        Ok(users) => {
            let responses: Vec<UserResponse> = users.into_iter().map(|u| u.into()).collect();
            HttpResponse::Ok().json(&responses)
        }
        Err(e) => {
            internal_error_response("/api/user/list", &e)
        }
    }
}

pub async fn get_user(
    req: web::Json<GetUserRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    if let Err(response) = check_admin(&http_req, &app_state) {
        return response;
    }

    match app_state.user_model.get_user_full(&req.id) {
        Ok(Some(user)) => {
            let response: UserResponse = user.into();
            HttpResponse::Ok().json(&response)
        }
        Ok(None) => {
            HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("USER_NOT_FOUND".to_string()),
            })
        }
        Err(e) => {
            internal_error_response("/api/user/get", &e)
        }
    }
}

pub async fn update_user(
    req: web::Json<UpdateUserRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let current_user_id = match check_admin(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    if req.is_admin == Some(false) && req.id == current_user_id {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("CANNOT_MODIFY_SELF_ADMIN".to_string()),
        });
    }

    let current_user = match app_state.user_model.get_user_full(&current_user_id) {
        Ok(Some(user)) => user,
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("USER_NOT_FOUND".to_string()),
            });
        }
        Err(e) => {
            return internal_error_response("/api/user/update", &e);
        }
    };

    if req.id == current_user_id {
        if let Some(new_expire_at) = &req.expire_at {
            let current_expire_at = current_user.expire_at.as_deref().unwrap_or("");
            if new_expire_at != current_expire_at {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("CANNOT_MODIFY_SELF_EXPIRE".to_string()),
                });
            }
        }
    }

    let recycle_bin_path = req.recycle_bin_path.as_ref().map(|v| v.as_deref().filter(|s| !s.trim().is_empty()));

    match app_state.user_model.get_user_full(&req.id) {
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("USER_NOT_FOUND".to_string()),
            });
        }
        Err(e) => {
            return internal_error_response("/api/user/update", &e);
        }
        Ok(Some(target_user)) => {
            if let Some(Some(rbp)) = &recycle_bin_path {
                let root_path = req.root_path.as_deref()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| target_user.root_path.as_deref().unwrap_or(""));
                if !root_path.is_empty() && is_recycle_bin_path_under_root(rbp, root_path) {
                    return HttpResponse::Ok().json(ApiResponse {
                        success: false,
                        fail_code: Some("RECYCLE_BIN_PATH_INVALID".to_string()),
                    });
                }
            }
        }
    }

    if let Some(ref pwd) = req.password {
        if !pwd.is_empty() && pwd.len() < 8 {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("PASSWORD_TOO_SHORT".to_string()),
            });
        }
        if !pwd.is_empty() && pwd.len() > 128 {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INVALID_PARAM".to_string()),
            });
        }
    }

    if let Some(ref rp) = req.root_path {
        if rp.contains("..") || !Path::new(rp).is_absolute() {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("PATH_INVALID".to_string()),
            });
        }
    }

    match app_state.user_model.update_user(
        &req.id,
        req.password.as_deref(),
        req.root_path.as_deref(),
        recycle_bin_path,
        req.is_admin,
        req.expire_at.as_deref(),
        req.remark.as_deref(),
    ) {
        Ok((password_updated, root_path_changed, admin_changed, recycle_bin_toggled)) => {
            if password_updated || root_path_changed || admin_changed || recycle_bin_toggled {
                let all_sessions = app_state.session_manager.get_all_session_ids();
                for session_id in all_sessions {
                    if let Some(uid) = app_state.session_manager.get_user_id(&session_id) {
                        if uid == req.id {
                            app_state.session_manager.invalidate(&session_id);
                        }
                    }
                }
            }
            HttpResponse::Ok().json(ApiResponse {
                success: true,
                fail_code: None,
            })
        }
        Err(e) => {
            internal_error_response("/api/user/update", &e)
        }
    }
}

pub async fn delete_user(
    req: web::Json<DeleteUserRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let current_user_id = match check_admin(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    if req.id == current_user_id {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("CANNOT_DELETE_SELF".to_string()),
        });
    }

    let user = match app_state.user_model.get_user_full(&req.id) {
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("USER_NOT_FOUND".to_string()),
            });
        }
        Err(e) => {
            return internal_error_response("/api/user/delete", &e);
        }
        Ok(Some(user)) => user,
    };

    if let Ok(notebooks) = app_state.notebook_model.list_by_user(&req.id) {
        for nb in &notebooks {
            if let Err(e) = app_state.search_manager.delete_search_db(&nb.id) {
                error_logger::log_error("delete_user/delete_search_db", &e);
            }
        }
    }

    if let Ok(rules) = app_state.backup_rule_model.list_by_user(&req.id) {
        for rule in &rules {
            app_state.backup_scheduler.remove_rule(&rule.id).await;
        }
    }

    match app_state.user_model.delete_user(&req.id, &app_state.backup_rule_model, &app_state.recycle_bin_model, &app_state.vault_model, &app_state.webdav_config_model, &app_state.notebook_model, &app_state.session_manager) {
        Ok(_) => {
            if let Some(ref root_path) = user.root_path {
                if !root_path.is_empty() {
                    let can_remove = match app_state.user_model.list_users() {
                        Ok(users) => {
                            let deleted_path = Path::new(root_path);
                            !users.iter().any(|u| {
                                if u.id == req.id {
                                    return false;
                                }
                                if let Some(ref other_rp) = u.root_path {
                                    if other_rp.is_empty() {
                                        return false;
                                    }
                                    let other_path = Path::new(other_rp);
                                    deleted_path.starts_with(other_path) || other_path.starts_with(deleted_path)
                                } else {
                                    false
                                }
                            })
                        }
                        Err(_) => false,
                    };
                    if can_remove {
                        let _ = std::fs::remove_dir_all(root_path);
                    }
                }
            }

            let headicons_dir = PathBuf::from("headicons");
            if let Ok(entries) = fs::read_dir(&headicons_dir) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        if let Some(stem) = path.file_stem() {
                            if stem.to_string_lossy() == req.id {
                                let _ = fs::remove_file(&path);
                            }
                        }
                    }
                }
            }

            HttpResponse::Ok().json(ApiResponse {
                success: true,
                fail_code: None,
            })
        }
        Err(e) => {
            internal_error_response("/api/user/delete", &e)
        }
    }
}

pub async fn create_user(
    req: web::Json<CreateUserRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    if let Err(response) = check_admin(&http_req, &app_state) {
        return response;
    }

    if req.username.is_empty() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("USERNAME_EMPTY".to_string()),
        });
    }

    if req.password.is_empty() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PASSWORD_EMPTY".to_string()),
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

    if let Some(ref exp) = req.expire_at {
        if !exp.is_empty() {
            if chrono::DateTime::parse_from_rfc3339(exp).is_err() {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("INVALID_PARAM".to_string()),
                });
            }
        }
    }

    match app_state.user_model.get_user_by_username(&req.username) {
        Ok(Some(_)) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("USERNAME_ALREADY_EXISTS".to_string()),
            });
        }
        Ok(None) => {}
        Err(e) => {
            return internal_error_response("/api/user/create", &e);
        }
    }

    let root_path = req.root_path.clone().unwrap_or_default();
    if root_path.contains("..") || (!root_path.is_empty() && !Path::new(&root_path).is_absolute()) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PATH_INVALID".to_string()),
        });
    }
    let recycle_bin_path = req.recycle_bin_path.as_deref().filter(|s| !s.trim().is_empty());
    let is_admin = req.is_admin.unwrap_or(false);

    if let Some(rbp) = recycle_bin_path {
        if !root_path.is_empty() && is_recycle_bin_path_under_root(rbp, &root_path) {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("RECYCLE_BIN_PATH_INVALID".to_string()),
            });
        }
    }

    match app_state.user_model.create_user_full(
        &req.username,
        &req.password,
        &root_path,
        recycle_bin_path,
        is_admin,
        req.expire_at.as_deref(),
        req.remark.as_deref(),
    ) {
        Ok(_) => {
            HttpResponse::Ok().json(ApiResponse {
                success: true,
                fail_code: None,
            })
        }
        Err(e) => {
            internal_error_response("/api/user/create", &e)
        }
    }
}

pub async fn upload_avatar(
    mut payload: Multipart,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let headicons_dir = PathBuf::from("headicons");
    if !headicons_dir.exists() {
        if let Err(e) = fs::create_dir_all(&headicons_dir) {
            return internal_error_response("/api/user/upload_avatar", &format!("Failed to create headicons directory: {}", e));
        }
    }

    let mut file_saved = false;

    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(f) => f,
            Err(e) => {
                return internal_error_response("/api/user/upload_avatar", &format!("Multipart error: {}", e));
            }
        };

        let content_disposition = field.content_disposition();
        let field_name = content_disposition.get_name().unwrap_or("");

        if field_name == "avatar" {
            let filename = content_disposition.get_filename().unwrap_or("avatar.jpg");
            let extension = filename.rsplit('.').next().unwrap_or("jpg").to_lowercase();

            if !AVATAR_EXTENSIONS.contains(&extension.as_str()) {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("INVALID_FILE_TYPE".to_string()),
                });
            }

            let avatar_path = headicons_dir.join(format!("{}.{}", user_id, extension));
            
            let mut file_data = Vec::new();
            while let Some(chunk) = field.next().await {
                let data = match chunk {
                    Ok(d) => d,
                    Err(e) => {
                        return internal_error_response("/api/user/upload_avatar", &format!("Failed to read chunk: {}", e));
                    }
                };
                file_data.extend_from_slice(&data);
                if file_data.len() > 5 * 1024 * 1024 {
                    return HttpResponse::Ok().json(ApiResponse {
                        success: false,
                        fail_code: Some("FILE_TOO_LARGE".to_string()),
                    });
                }
            }

            if let Err(e) = fs::write(&avatar_path, &file_data) {
                return internal_error_response("/api/user/upload_avatar", &format!("Failed to write file: {}", e));
            }

            if let Ok(entries) = fs::read_dir(&headicons_dir) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        if let Some(stem) = path.file_stem() {
                            if stem.to_string_lossy() == user_id && path != avatar_path {
                                let _ = fs::remove_file(&path);
                            }
                        }
                    }
                }
            }

            file_saved = true;
            break;
        }
    }

    if file_saved {
        HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        })
    } else {
        HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("NO_FILE_UPLOADED".to_string()),
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct GetAvatarRequest {
    id: String,
}

pub async fn get_avatar(
    req: web::Json<GetAvatarRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let current_user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let session_id = match get_session_id(&http_req) {
        Some(id) => id,
        None => return internal_error_response("/api/user/get_avatar", "No session"),
    };
    let is_admin = app_state.session_manager.get(&session_id, "is_admin");
    if is_admin.as_deref() != Some("true") && req.id != current_user_id {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PERMISSION_DENIED".to_string()),
        });
    }

    if !is_safe_name(&req.id) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
        });
    }

    let headicons_dir = PathBuf::from("headicons");

    for ext in AVATAR_EXTENSIONS {
        let avatar_path = headicons_dir.join(format!("{}.{}", req.id, ext));
        if avatar_path.exists() {
            let mime_type = match ext {
                "jpg" | "jpeg" => "image/jpeg",
                "png" => "image/png",
                "gif" => "image/gif",
                "webp" => "image/webp",
                _ => "application/octet-stream",
            };

            match fs::read(&avatar_path) {
                Ok(data) => {
                    return HttpResponse::Ok()
                        .content_type(mime_type)
                        .body(data);
                }
                Err(e) => {
                    return internal_error_response("/api/user/get_avatar", &format!("Failed to read avatar: {}", e));
                }
            }
        }
    }

    HttpResponse::Ok().json(ApiResponse {
        success: false,
        fail_code: Some("AVATAR_NOT_FOUND".to_string()),
    })
}

pub async fn delete_avatar(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let headicons_dir = PathBuf::from("headicons");
    let mut deleted = false;

    for ext in AVATAR_EXTENSIONS {
        let avatar_path = headicons_dir.join(format!("{}.{}", user_id, ext));
        if avatar_path.exists() {
            match fs::remove_file(&avatar_path) {
                Ok(_) => {
                    deleted = true;
                    break;
                }
                Err(e) => {
                    return internal_error_response("/api/user/delete_avatar", &format!("Failed to delete avatar: {}", e));
                }
            }
        }
    }

    if deleted {
        HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        })
    } else {
        HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("AVATAR_NOT_FOUND".to_string()),
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    old_password: String,
    new_password: String,
}

pub async fn change_password(
    req: web::Json<ChangePasswordRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    if req.old_password.is_empty() || req.new_password.is_empty() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PASSWORD_EMPTY".to_string()),
        });
    }

    if req.old_password.len() > 128 {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
        });
    }

    if req.new_password.len() > 128 {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
        });
    }

    if req.new_password.len() < 8 {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PASSWORD_TOO_SHORT".to_string()),
        });
    }

    match app_state.user_model.verify_password_by_id(&user_id, &req.old_password) {
        Ok(true) => {
            match app_state.user_model.change_password(
                &user_id,
                &req.new_password,
                &app_state.session_manager,
            ) {
                Ok(_) => {
                    HttpResponse::Ok().json(ApiResponse {
                        success: true,
                        fail_code: None,
                    })
                }
                Err(e) => {
                    internal_error_response("/api/user/change_password", &e)
                }
            }
        }
        Ok(false) => {
            HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("OLD_PASSWORD_INCORRECT".to_string()),
            })
        }
        Err(e) => {
            internal_error_response("/api/user/change_password", &e)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateFeatureOrderRequest {
    feature_order: String,
}

pub async fn update_feature_order(
    req: web::Json<UpdateFeatureOrderRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let valid_features = ["file", "note", "password"];
    let features: Vec<&str> = req.feature_order.split(',').map(|s| s.trim()).collect();

    if features.len() != valid_features.len() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FEATURE_ORDER".to_string()),
        });
    }

    let mut sorted_features = features.clone();
    sorted_features.sort();
    let mut sorted_valid: Vec<&str> = valid_features.to_vec();
    sorted_valid.sort();

    if sorted_features != sorted_valid {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FEATURE_ORDER".to_string()),
        });
    }

    match app_state.user_model.update_feature_order(&user_id, &req.feature_order) {
        Ok(_) => {
            HttpResponse::Ok().json(ApiResponse {
                success: true,
                fail_code: None,
            })
        }
        Err(e) => {
            internal_error_response("/api/user/update_feature_order", &e)
        }
    }
}
