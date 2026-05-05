use actix_web::{web, HttpRequest, HttpResponse, Responder};
use actix_files::NamedFile;
use actix_web::http::header::{ContentDisposition, DispositionParam, DispositionType, ExtendedValue, Charset};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::handlers::{
    get_current_user_id, get_user_root_path, internal_error_response, is_path_under_root,
    is_safe_path, ApiResponse,
};
use crate::handlers::file::calc_dir_size;
use crate::handlers::compress::compress_folder;
use crate::models::ShareInfo;

const SHARE_CODE_CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
const TOKEN_TTL: std::time::Duration = std::time::Duration::from_secs(3600);

fn hash_password(password: &str, salt: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(salt.as_bytes()).unwrap();
    mac.update(password.as_bytes());
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

fn generate_salt() -> String {
    rand::thread_rng()
        .sample_iter(rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

fn generate_share_code() -> String {
    let mut rng = rand::thread_rng();
    (0..8)
        .map(|_| {
            let idx = rng.gen_range(0..SHARE_CODE_CHARS.len());
            SHARE_CODE_CHARS[idx] as char
        })
        .collect()
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

fn get_file_name_from_path(file_path: &str) -> String {
    Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string()
}

fn store_share_token(app_state: &web::Data<AppState>, share_code: &str) -> String {
    let token = Uuid::new_v4().to_string();
    let mut tokens = app_state.share_tokens.lock().unwrap_or_else(|e| e.into_inner());
    let now = std::time::Instant::now();
    tokens.retain(|_, entry| entry.expires_at > now);
    tokens.insert(token.clone(), crate::app_state::ShareTokenEntry {
        share_code: share_code.to_string(),
        expires_at: now + TOKEN_TTL,
    });
    token
}

fn validate_share_token(app_state: &web::Data<AppState>, share_code: &str, token: &str) -> bool {
    let mut tokens = app_state.share_tokens.lock().unwrap_or_else(|e| e.into_inner());
    let now = std::time::Instant::now();
    tokens.retain(|_, entry| entry.expires_at > now);
    match tokens.remove(token) {
        Some(entry) => entry.share_code == share_code,
        None => false,
    }
}

fn attachment_content_disposition(filename: &str) -> ContentDisposition {
    ContentDisposition {
        disposition: DispositionType::Attachment,
        parameters: vec![
            DispositionParam::Filename(filename.to_string()),
            DispositionParam::FilenameExt(ExtendedValue {
                charset: Charset::Ext("UTF-8".to_string()),
                language_tag: None,
                value: filename.as_bytes().to_vec(),
            }),
        ],
    }
}

fn is_time_expired(expire_at: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(expire_at)
        .map(|dt| dt.with_timezone(&chrono::Utc) < chrono::Utc::now())
        .unwrap_or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(expire_at, "%Y-%m-%d %H:%M:%S")
                .map(|t| t < chrono::Utc::now().naive_utc())
                .unwrap_or(true)
        })
}

fn is_share_expired(share: &ShareInfo) -> bool {
    if share.expire_type == "time" {
        if let Some(ref expire_at) = share.expire_at {
            if is_time_expired(expire_at) {
                return true;
            }
        }
    }
    if share.expire_type == "count" {
        if let Some(max) = share.max_downloads {
            if share.download_count >= max {
                return true;
            }
        }
    }
    false
}

fn check_share_access(share: &ShareInfo, check_count: bool) -> Option<HttpResponse> {
    if share.expire_type == "time" {
        if let Some(ref expire_at) = share.expire_at {
            if is_time_expired(expire_at) {
                return Some(HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("SHARE_EXPIRED".to_string()),
                }));
            }
        }
    }
    if check_count && share.expire_type == "count" {
        if let Some(max) = share.max_downloads {
            if share.download_count >= max {
                return Some(HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("SHARE_OVER_LIMIT".to_string()),
                }));
            }
        }
    }
    None
}

fn get_file_size(path: &Path, is_directory: bool) -> i64 {
    if is_directory {
        calc_dir_size(path)
    } else {
        path.metadata().map(|m| m.len() as i64).unwrap_or(0)
    }
}

fn compute_status(share: &ShareInfo, user_root: &str) -> String {
    if is_share_expired(share) {
        return "expired".to_string();
    }
    let file_path = Path::new(user_root).join(&share.file_path);
    if !file_path.exists() {
        return "file_missing".to_string();
    }
    "active".to_string()
}

fn make_share_item(share: ShareInfo, user_root: &str) -> ShareItem {
    let status = compute_status(&share, user_root);
    ShareItem {
        id: share.id,
        file_name: share.file_name,
        file_path: share.file_path,
        is_directory: share.is_directory,
        share_code: share.share_code,
        expire_type: share.expire_type,
        expire_at: share.expire_at,
        max_downloads: share.max_downloads,
        download_count: share.download_count,
        share_mode: share.share_mode,
        has_password: share.password.is_some(),
        status,
        created_at: share.created_at,
    }
}

#[derive(Debug, Deserialize)]
pub struct GetDownloadTokenRequest {
    pub share_code: String,
    pub password_hash: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GetDownloadTokenResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_token: Option<String>,
}

pub async fn get_share_download_token(
    body: web::Json<GetDownloadTokenRequest>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let share = match app_state.share_model.get_by_code(&body.share_code) {
        Ok(Some(s)) => s,
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("SHARE_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/share/get_download_token", &e),
    };

    if let Some(resp) = check_share_access(&share, true) {
        return resp;
    }

    let user_root = match app_state.user_model.get_user_full(&share.user_id) {
        Ok(Some(u)) => u.root_path.unwrap_or_default(),
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("SHARE_FILE_MISSING".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/share/get_download_token", &e),
    };

    let file_full_path = Path::new(&user_root).join(&share.file_path);
    if !file_full_path.exists() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("SHARE_FILE_MISSING".to_string()),
        });
    }

    if share.password.is_some() {
        match body.password_hash.as_deref() {
            None => {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("SHARE_PASSWORD_REQUIRED".to_string()),
                });
            }
            Some(pwd_hash) => {
                let stored = share.password.as_deref().unwrap();
                let parts: Vec<&str> = stored.splitn(2, ':').collect();
                if parts.len() != 2 {
                    return internal_error_response("/api/share/get_download_token", "invalid password format");
                }
                let stored_hash = parts[1];
                if !constant_time_eq(pwd_hash.as_bytes(), stored_hash.as_bytes()) {
                    return HttpResponse::Ok().json(ApiResponse {
                        success: false,
                        fail_code: Some("SHARE_PASSWORD_WRONG".to_string()),
                    });
                }
            }
        }
    }

    let token = store_share_token(&app_state, &share.share_code);

    HttpResponse::Ok().json(GetDownloadTokenResponse {
        success: true,
        download_token: Some(token),
    })
}

#[derive(Debug, Deserialize)]
pub struct GetShareInfoRequest {
    pub share_code: String,
}

#[derive(Debug, Serialize)]
pub struct GetShareInfoResponse {
    pub success: bool,
    pub file_name: String,
    pub is_directory: bool,
    pub file_size: i64,
    pub share_mode: String,
    pub need_password: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password_salt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expire_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expire_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_downloads: Option<i64>,
    pub download_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

pub async fn get_share_info(
    body: web::Json<GetShareInfoRequest>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let share = match app_state.share_model.get_by_code(&body.share_code) {
        Ok(Some(s)) => s,
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("SHARE_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/share/info", &e),
    };

    if let Some(resp) = check_share_access(&share, true) {
        return resp;
    }

    let need_password = share.password.is_some();
    let password_salt = share.password.as_deref().map(|p| {
        p.splitn(2, ':').next().unwrap_or("").to_string()
    });

    let user_root = match app_state.user_model.get_user_full(&share.user_id) {
        Ok(Some(u)) => u.root_path.unwrap_or_default(),
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("SHARE_FILE_MISSING".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/share/info", &e),
    };

    let file_full_path = Path::new(&user_root).join(&share.file_path);

    if !file_full_path.exists() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("SHARE_FILE_MISSING".to_string()),
        });
    }

    let file_size = get_file_size(&file_full_path, share.is_directory);

    HttpResponse::Ok().json(GetShareInfoResponse {
        success: true,
        file_name: share.file_name,
        is_directory: share.is_directory,
        file_size,
        share_mode: share.share_mode,
        need_password,
        password_salt,
        expire_type: Some(share.expire_type),
        expire_at: share.expire_at,
        max_downloads: share.max_downloads,
        download_count: share.download_count,
        created_at: Some(share.created_at),
    })
}

#[derive(Debug, Deserialize)]
pub struct DownloadQuery {
    pub token: Option<String>,
}

pub async fn download_share_file(
    share_code: web::Path<String>,
    query: web::Query<DownloadQuery>,
    app_state: web::Data<AppState>,
    req: HttpRequest,
) -> impl Responder {
    let code = share_code.into_inner();

    let share = match app_state.share_model.get_by_code(&code) {
        Ok(Some(s)) => s,
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("SHARE_NOT_FOUND".to_string()),
            });
        }
        Err(e) => {
            crate::error_logger::log_error("/api/share/file", &e);
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INTERNAL_ERROR".to_string()),
            });
        }
    };

    if let Some(resp) = check_share_access(&share, true) {
        return resp;
    }
    if share.share_mode == "page" || share.password.is_some() {
        match query.token.as_deref() {
            Some(token) => {
                if !validate_share_token(&app_state, &share.share_code, token) {
                    return HttpResponse::Ok().json(ApiResponse {
                        success: false,
                        fail_code: Some("SHARE_DOWNLOAD_DENIED".to_string()),
                    });
                }
            }
            None => {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("SHARE_DOWNLOAD_DENIED".to_string()),
                });
            }
        }
    }

    let user_root = match app_state.user_model.get_user_full(&share.user_id) {
        Ok(Some(u)) => u.root_path.unwrap_or_default(),
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("SHARE_FILE_MISSING".to_string()),
            });
        }
        Err(e) => {
            crate::error_logger::log_error("/api/share/file", &e);
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INTERNAL_ERROR".to_string()),
            });
        }
    };

    let file_full_path = Path::new(&user_root).join(&share.file_path);

    if !file_full_path.exists() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("SHARE_FILE_MISSING".to_string()),
        });
    }

    if share.is_directory {
        match app_state.share_model.increment_download_count(&code) {
            Ok(true) => {}
            Ok(false) => {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("SHARE_OVER_LIMIT".to_string()),
                });
            }
            Err(e) => {
                crate::error_logger::log_error("/api/share/file", &e);
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("INTERNAL_ERROR".to_string()),
                });
            }
        }

        let dir_path_clone = file_full_path.clone();
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<bytes::Bytes, String>>(8);

        tokio::spawn(async move {
            if let Err(e) = compress_folder(&dir_path_clone, tx.clone()).await {
                let _ = tx.send(Err(e)).await;
            }
        });

        let filename = format!("{}.zip", share.file_name);
        let stream = async_stream::stream! {
            let mut rx = rx;
            while let Some(item) = rx.recv().await {
                match item {
                    Ok(bytes) => yield Ok::<bytes::Bytes, actix_web::Error>(bytes),
                    Err(_) => {
                        yield Err(actix_web::error::ErrorInternalServerError("compress failed"));
                        break;
                    }
                }
            }
        };

        return HttpResponse::Ok()
            .content_type("application/zip")
            .insert_header(attachment_content_disposition(&filename))
            .streaming(stream);
    } else {
        match app_state.share_model.increment_download_count(&code) {
            Ok(true) => {}
            Ok(false) => {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("SHARE_OVER_LIMIT".to_string()),
                });
            }
            Err(e) => {
                crate::error_logger::log_error("/api/share/file", &e);
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("INTERNAL_ERROR".to_string()),
                });
            }
        }

        match NamedFile::open(&file_full_path) {
            Ok(file) => {
                let filename = share.file_name.clone();
                let response = file
                    .set_content_disposition(attachment_content_disposition(&filename))
                    .into_response(&req);
                return response;
            }
            Err(e) => {
                if file_full_path.exists() {
                    crate::error_logger::log_error("/api/share/file", &format!("Failed to open file {:?}: {}", file_full_path, e));
                    return HttpResponse::InternalServerError().finish();
                }
                return HttpResponse::NotFound().finish();
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateShareRequest {
    pub file_path: String,
    pub expire_type: String,
    pub expire_at: Option<String>,
    pub max_downloads: Option<i64>,
    pub share_mode: String,
    pub password: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateShareResponse {
    pub success: bool,
    pub share_code: String,
    pub share_url: String,
    pub direct_url: String,
}

pub async fn create_share(
    body: web::Json<CreateShareRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };
    let root_path_obj = Path::new(&root_path);

    let target_path = root_path_obj.join(&body.file_path);

    if body.file_path.is_empty() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PATH_INVALID".to_string()),
        });
    }

    if !is_safe_path(&body.file_path) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PATH_INVALID".to_string()),
        });
    }

    if !is_path_under_root(&target_path, root_path_obj) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PATH_INVALID".to_string()),
        });
    }

    if !target_path.exists() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("FILE_NOT_FOUND".to_string()),
        });
    }

    if body.expire_type != "permanent" && body.expire_type != "time" && body.expire_type != "count" {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PARAM_INVALID".to_string()),
        });
    }

    if body.share_mode != "page" && body.share_mode != "direct" {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PARAM_INVALID".to_string()),
        });
    }

    if body.expire_type != "time" && body.expire_at.is_some() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PARAM_INVALID".to_string()),
        });
    }

    if body.expire_type != "count" && body.max_downloads.is_some() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PARAM_INVALID".to_string()),
        });
    }

    if body.expire_type == "time" {
        match &body.expire_at {
            None => {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("PARAM_INVALID".to_string()),
                });
            }
            Some(s) => {
                if chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S").is_err()
                    && chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").is_err()
                    && chrono::DateTime::parse_from_rfc3339(s).is_err()
                {
                    return HttpResponse::Ok().json(ApiResponse {
                        success: false,
                        fail_code: Some("PARAM_INVALID".to_string()),
                    });
                }
            }
        }
    }

    if body.expire_type == "count" && body.max_downloads.map_or(true, |v| v <= 0) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PARAM_INVALID".to_string()),
        });
    }

    if body.share_mode == "direct" && body.password.as_ref().map_or(false, |p| !p.is_empty()) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("SHARE_DIRECT_NO_PASSWORD".to_string()),
        });
    }

    let password = body.password.as_deref().filter(|s| !s.is_empty());

    match app_state.share_model.get_by_path(&user_id, &body.file_path) {
        Ok(Some(existing)) => {
            let status = compute_status(&existing, &root_path);
            if status == "active" {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("SHARE_ALREADY_EXISTS".to_string()),
                });
            }
            if let Err(e) = app_state.share_model.delete_by_ids(&user_id, &[existing.id.clone()]) {
                return internal_error_response("/api/share/create", &e);
            }
        }
        Ok(None) => {}
        Err(e) => return internal_error_response("/api/share/create", &e),
    }

    let stale_shares: Vec<ShareInfo> = app_state.share_model.list_by_user(&user_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.file_path == body.file_path && compute_status(s, &root_path) != "active")
        .collect();
    if !stale_shares.is_empty() {
        let stale_ids: Vec<String> = stale_shares.iter().map(|s| s.id.clone()).collect();
        let _ = app_state.share_model.delete_by_ids(&user_id, &stale_ids);
    }

    let share_code = loop {
        let code = generate_share_code();
        match app_state.share_model.get_by_code(&code) {
            Ok(None) => break code,
            Ok(Some(_)) => continue,
            Err(e) => return internal_error_response("/api/share/create", &e),
        }
    };

    let hashed_password = password.map(|pwd| {
        let salt = generate_salt();
        let hash = hash_password(pwd, &salt);
        format!("{}:{}", salt, hash)
    });

    let file_name = get_file_name_from_path(&body.file_path);
    let is_directory = target_path.is_dir();

    match app_state.share_model.create(
        &user_id,
        &body.file_path,
        &file_name,
        is_directory,
        &share_code,
        &body.expire_type,
        body.expire_at.as_deref(),
        body.max_downloads,
        &body.share_mode,
        hashed_password.as_deref(),
    ) {
        Ok(_) => HttpResponse::Ok().json(CreateShareResponse {
            success: true,
            share_url: format!("/s/{}", share_code),
            direct_url: format!("/api/share/file/{}", share_code),
            share_code,
        }),
        Err(e) => internal_error_response("/api/share/create", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct GetShareByPathRequest {
    pub file_path: String,
}

#[derive(Debug, Serialize)]
pub struct ShareItem {
    pub id: String,
    pub file_name: String,
    pub file_path: String,
    pub is_directory: bool,
    pub share_code: String,
    pub expire_type: String,
    pub expire_at: Option<String>,
    pub max_downloads: Option<i64>,
    pub download_count: i64,
    pub share_mode: String,
    pub has_password: bool,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct GetShareByPathResponse {
    pub success: bool,
    pub share: Option<ShareItem>,
}

pub async fn get_share_by_path(
    body: web::Json<GetShareByPathRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };

    match app_state.share_model.get_by_path(&user_id, &body.file_path) {
        Ok(Some(share)) => {
            HttpResponse::Ok().json(GetShareByPathResponse {
                success: true,
                share: Some(make_share_item(share, &root_path)),
            })
        }
        Ok(None) => HttpResponse::Ok().json(GetShareByPathResponse {
            success: true,
            share: None,
        }),
        Err(e) => internal_error_response("/api/share/get_by_path", &e),
    }
}

#[derive(Debug, Serialize)]
pub struct ListSharesResponse {
    pub success: bool,
    pub shares: Vec<ShareItem>,
}

pub async fn list_shares(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };

    match app_state.share_model.list_by_user(&user_id) {
        Ok(shares) => {
            let items: Vec<ShareItem> = shares
                .into_iter()
                .map(|s| make_share_item(s, &root_path))
                .collect();
            HttpResponse::Ok().json(ListSharesResponse {
                success: true,
                shares: items,
            })
        }
        Err(e) => internal_error_response("/api/share/list", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct DeleteSharesRequest {
    pub ids: Vec<String>,
}

pub async fn delete_shares(
    body: web::Json<DeleteSharesRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    match app_state.share_model.delete_by_ids(&user_id, &body.ids) {
        Ok(()) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        }),
        Err(e) => internal_error_response("/api/share/delete", &e),
    }
}
