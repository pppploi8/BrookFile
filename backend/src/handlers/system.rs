use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::app_state::AppState;
use crate::handlers::{internal_error_response, check_admin, ApiResponse};
use crate::middleware::get_session_id;

#[derive(Debug, Serialize)]
pub struct SystemInfoResponse {
    pub initialized: bool,
    pub logged_in: bool,
    pub system_name: String,
    pub user: Option<UserInfoResponse>,
}

#[derive(Debug, Serialize)]
pub struct UserInfoResponse {
    pub id: String,
    pub username: String,
    pub is_admin: bool,
    pub feature_order: String,
    pub recycle_bin_enabled: bool,
    pub has_shares: bool,
}

#[derive(Debug, Serialize)]
pub struct BrowseResponse {
    pub folders: Vec<FolderInfo>,
    pub has_parent: bool,
    pub parent_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FolderInfo {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct BrowseRequest {
    pub path: Option<String>,
}

pub async fn get_system_info(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let initialized = match app_state.is_initialized() {
        Ok(value) => value,
        Err(e) => {
            return internal_error_response("/api/system/info", &e);
        }
    };

    let session_id = get_session_id(&http_req);
    let username = session_id
        .as_ref()
        .and_then(|id| app_state.session_manager.get(id, "username"));

    let user = match username {
        Some(ref uname) => {
            match app_state.user_model.get_user_by_username(uname) {
                Ok(Some(user_info)) => {
                    let has_shares = app_state.share_model.has_shares_by_user(&user_info.id).unwrap_or(false);
                    Some(UserInfoResponse {
                        id: user_info.id,
                        username: user_info.username,
                        is_admin: user_info.is_admin,
                        feature_order: user_info.feature_order,
                        recycle_bin_enabled: user_info.recycle_bin_path.is_some(),
                        has_shares,
                    })
                }
                _ => None,
            }
        }
        _ => None,
    };

    let logged_in = user.is_some();

    let system_name = match app_state.system_config_model.get_config("system_name") {
        Ok(Some(name)) => name,
        Ok(None) => {
            crate::error_logger::log_error("/api/system/info", "system_name not configured");
            "BrookFile".to_string()
        }
        Err(e) => {
            return internal_error_response("/api/system/info", &e);
        }
    };

    HttpResponse::Ok().json(SystemInfoResponse {
        initialized,
        logged_in,
        user,
        system_name,
    })
}

pub async fn browse_folders(
    body: web::Json<BrowseRequest>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    match app_state.is_initialized() {
        Ok(true) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("SYSTEM_ALREADY_INITIALIZED".to_string()),
            });
        }
        Ok(false) => {}
        Err(e) => return internal_error_response("/api/system/browse", &e),
    }

    if let Some(p) = body.path.as_deref() {
        if p.contains("..") {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INVALID_PARAM".to_string()),
            });
        }
    }

    let path = body.path.as_deref().unwrap_or("");

    let (folders, has_parent, parent_path) = if path.is_empty() {
        (get_root_folders(), false, None)
    } else {
        let parent = get_parent_path(path);
        (get_folders_in_path(path), parent.is_some(), parent)
    };

    HttpResponse::Ok().json(BrowseResponse {
        folders,
        has_parent,
        parent_path,
    })
}

fn get_parent_path(path: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        if path.ends_with(':') || path.ends_with(":\\") {
            return Some(String::new());
        }
    }
    let path_obj = std::path::Path::new(path);
    path_obj.parent().map(|p| {
        let p_str = p.to_string_lossy().to_string();
        #[cfg(target_os = "windows")]
        {
            if p_str.ends_with(':') {
                format!("{}\\", p_str)
            } else {
                p_str
            }
        }
        #[cfg(not(target_os = "windows"))]
        p_str
    })
}

#[cfg(target_os = "windows")]
fn get_root_folders() -> Vec<FolderInfo> {
    let mut drives = Vec::new();
    for letter in b'A'..=b'Z' {
        let drive = format!("{}:", letter as char);
        let path = format!("{}\\", drive);
        if std::path::Path::new(&path).exists() {
            drives.push(FolderInfo {
                name: drive,
                path,
            });
        }
    }
    drives
}

#[cfg(not(target_os = "windows"))]
fn get_root_folders() -> Vec<FolderInfo> {
    get_folders_in_path("/")
}

fn get_folders_in_path(path: &str) -> Vec<FolderInfo> {
    let mut folders = Vec::new();
    
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                if let Some(name) = entry_path.file_name() {
                    if let Some(name_str) = name.to_str() {
                        folders.push(FolderInfo {
                            name: name_str.to_string(),
                            path: entry_path.to_string_lossy().to_string(),
                        });
                    }
                }
            }
        }
    }
    
    folders
}

const LOGO_EXTENSIONS: [&str; 6] = ["jpg", "jpeg", "png", "gif", "webp", "svg"];

#[derive(Debug, Serialize)]
pub struct SystemSettingsResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    pub system_name: String,
    pub session_timeout_days: u64,
    pub max_login_devices: u64,
    pub notebook_fulltext_search: bool,
    pub has_logo: bool,
}

pub async fn get_settings(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    if let Err(response) = check_admin(&http_req, &app_state) {
        return response;
    }

    let system_name = match app_state.system_config_model.get_config("system_name") {
        Ok(Some(v)) => v,
        _ => "BrookFile".to_string(),
    };

    let session_timeout_days: u64 = match app_state.system_config_model.get_config("session_timeout_days") {
        Ok(Some(v)) => v.parse().unwrap_or(7),
        _ => 7,
    };

    let max_login_devices: u64 = match app_state.system_config_model.get_config("max_login_devices") {
        Ok(Some(v)) => v.parse().unwrap_or(3),
        _ => 3,
    };

    let notebook_fulltext_search = match app_state.system_config_model.get_config("notebook_fulltext_search") {
        Ok(Some(v)) => v == "true",
        _ => true,
    };

    let has_logo = LOGO_EXTENSIONS.iter().any(|ext| {
        let exe_dir = std::env::current_exe().unwrap_or_default();
        let exe_parent = exe_dir.parent().unwrap_or(std::path::Path::new("."));
        exe_parent.join(format!("system_logo.{}", ext)).exists()
    });

    HttpResponse::Ok().json(SystemSettingsResponse {
        success: true,
        fail_code: None,
        system_name,
        session_timeout_days,
        max_login_devices,
        notebook_fulltext_search,
        has_logo,
    })
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
    pub system_name: String,
    pub session_timeout_days: u64,
    pub max_login_devices: u64,
    pub notebook_fulltext_search: bool,
}

pub async fn update_settings(
    http_req: HttpRequest,
    body: web::Json<UpdateSettingsRequest>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    if let Err(response) = check_admin(&http_req, &app_state) {
        return response;
    }

    if body.system_name.trim().is_empty() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
        });
    }

    if body.session_timeout_days == 0 {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
        });
    }

    if body.max_login_devices == 0 {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
        });
    }

    if let Err(e) = app_state.system_config_model.set_config("system_name", body.system_name.trim()) {
        return internal_error_response("/api/system/update_settings", &e);
    }
    if let Err(e) = app_state.system_config_model.set_config("session_timeout_days", &body.session_timeout_days.to_string()) {
        return internal_error_response("/api/system/update_settings", &e);
    }
    if let Err(e) = app_state.system_config_model.set_config("max_login_devices", &body.max_login_devices.to_string()) {
        return internal_error_response("/api/system/update_settings", &e);
    }
    app_state.session_manager.update_config(body.session_timeout_days, body.max_login_devices);
    let old_fulltext = match app_state.system_config_model.get_config("notebook_fulltext_search") {
        Ok(Some(v)) => v == "true",
        _ => !body.notebook_fulltext_search,
    };
    if let Err(e) = app_state.system_config_model.set_config("notebook_fulltext_search", if body.notebook_fulltext_search { "true" } else { "false" }) {
        return internal_error_response("/api/system/update_settings", &e);
    }
    if old_fulltext != body.notebook_fulltext_search {
        if let Err(e) = app_state.system_config_model.set_config("notebook_needs_rebuild", "true") {
            return internal_error_response("/api/system/update_settings", &e);
        }
    }

    HttpResponse::Ok().json(ApiResponse {
        success: true,
        fail_code: None,
    })
}

pub async fn rebuild_notebook_index(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    if let Err(response) = check_admin(&http_req, &app_state) {
        return response;
    }

    if !app_state.search_manager.is_enabled() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("FULLTEXT_SEARCH_DISABLED".to_string()),
        });
    }

    let search_mgr = Arc::clone(&app_state.search_manager);
    let pool = app_state.notebook_model.pool.clone();
    std::thread::spawn(move || {
        let notebook_model = crate::models::NotebookModel::new(&pool);
        let user_model = crate::models::UserModel::new(&pool);
        match notebook_model.list_all_non_encrypted() {
            Ok(notebooks) => {
                for (user_id, nb) in &notebooks {
                    if let Ok(Some(user)) = user_model.get_user_full(user_id) {
                        if let Some(rp) = user.root_path {
                            let full_path = format!("{}/{}", rp, nb.path);
                            if let Err(e) = search_mgr.rebuild_notebook_index(&nb.id, &full_path) {
                                crate::error_logger::log_error("rebuild_notebook_index", &e);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                crate::error_logger::log_error("rebuild_notebook_index", &format!("list_all_non_encrypted: {}", e));
            }
        }
    });

    HttpResponse::Ok().json(ApiResponse {
        success: true,
        fail_code: None,
    })
}

pub async fn upload_system_logo(
    mut payload: Multipart,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    if let Err(response) = check_admin(&http_req, &app_state) {
        return response;
    }

    let exe_dir = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => return internal_error_response("/api/system/upload_logo", &format!("get exe dir: {}", e)),
    };
    let exe_parent = exe_dir.parent().unwrap_or(std::path::Path::new(".")).to_path_buf();

    let mut file_saved = false;

    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(f) => f,
            Err(e) => return internal_error_response("/api/system/upload_logo", &format!("multipart: {}", e)),
        };

        let cd = field.content_disposition();
        let field_name = cd.get_name().unwrap_or("");

        if field_name == "logo" {
            let filename = cd.get_filename().unwrap_or("logo.png");
            let extension = filename.rsplit('.').next().unwrap_or("png").to_lowercase();

            if !LOGO_EXTENSIONS.contains(&extension.as_str()) {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("INVALID_FILE_TYPE".to_string()),
                });
            }

            let logo_path = exe_parent.join(format!("system_logo.{}", extension));
            let mut file_data = Vec::new();

            while let Some(chunk) = field.next().await {
                let data = match chunk {
                    Ok(d) => d,
                    Err(e) => return internal_error_response("/api/system/upload_logo", &format!("read chunk: {}", e)),
                };
                file_data.extend_from_slice(&data);
                if file_data.len() > 2 * 1024 * 1024 {
                    return HttpResponse::Ok().json(ApiResponse {
                        success: false,
                        fail_code: Some("FILE_TOO_LARGE".to_string()),
                    });
                }
            }

            if let Err(e) = std::fs::write(&logo_path, &file_data) {
                return internal_error_response("/api/system/upload_logo", &format!("write: {}", e));
            }

            for ext in &LOGO_EXTENSIONS {
                if *ext != extension {
                    let old = exe_parent.join(format!("system_logo.{}", ext));
                    if old.exists() {
                        let _ = std::fs::remove_file(&old);
                    }
                }
            }

            file_saved = true;
            break;
        }
    }

    if file_saved {
        HttpResponse::Ok().json(ApiResponse { success: true, fail_code: None })
    } else {
        HttpResponse::Ok().json(ApiResponse { success: false, fail_code: Some("NO_FILE_UPLOADED".to_string()) })
    }
}

pub async fn get_system_logo() -> impl Responder {
    let exe_dir = match std::env::current_exe() {
        Ok(e) => e,
        Err(_) => return HttpResponse::NotFound().finish(),
    };
    let exe_parent = exe_dir.parent().unwrap_or(std::path::Path::new("."));

    for ext in &LOGO_EXTENSIONS {
        let logo_path = exe_parent.join(format!("system_logo.{}", ext));
        if logo_path.exists() {
            let mime = match *ext {
                "jpg" | "jpeg" => "image/jpeg",
                "png" => "image/png",
                "gif" => "image/gif",
                "webp" => "image/webp",
                "svg" => "image/svg+xml",
                _ => "application/octet-stream",
            };
            match std::fs::read(&logo_path) {
                Ok(data) => return HttpResponse::Ok().content_type(mime).body(data),
                Err(_) => return HttpResponse::NotFound().finish(),
            }
        }
    }

    HttpResponse::NotFound().finish()
}

pub async fn delete_system_logo(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    if let Err(response) = check_admin(&http_req, &app_state) {
        return response;
    }

    let exe_dir = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => return internal_error_response("/api/system/delete_logo", &format!("get exe dir: {}", e)),
    };
    let exe_parent = exe_dir.parent().unwrap_or(std::path::Path::new("."));

    let mut deleted = false;
    for ext in &LOGO_EXTENSIONS {
        let logo_path = exe_parent.join(format!("system_logo.{}", ext));
        if logo_path.exists() {
            match std::fs::remove_file(&logo_path) {
                Ok(_) => { deleted = true; break; }
                Err(e) => return internal_error_response("/api/system/delete_logo", &format!("delete: {}", e)),
            }
        }
    }

    HttpResponse::Ok().json(ApiResponse {
        success: deleted,
        fail_code: if deleted { None } else { Some("LOGO_NOT_FOUND".to_string()) },
    })
}
