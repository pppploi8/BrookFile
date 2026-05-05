use actix_web::{web, HttpRequest, HttpResponse, Responder};
use actix_multipart::Multipart;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;
use crate::app_state::AppState;
use crate::handlers::{ApiResponse, internal_error_response, get_user_root_path, get_current_user_id, is_safe_path, is_path_under_root};

#[derive(Debug, Deserialize)]
pub struct UploadStartRequest {
    pub files: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct UploadStartResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uploads: Option<Vec<UploadInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_files: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct UploadInfo {
    pub id: String,
    pub file: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadCompleteRequest {
    pub upload_id: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadCancelRequest {
    pub upload_id: String,
}

pub async fn upload_start(
    body: web::Json<UploadStartRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if body.files.is_empty() {
        return HttpResponse::Ok().json(serde_json::json!({
            "success": false,
            "fail_code": "INVALID_PARAM"
        }));
    }

    let root_path_obj = Path::new(&root_path);

    let mut seen = std::collections::HashSet::new();
    for file_path in &body.files {
        if !seen.insert(file_path.clone()) {
            return HttpResponse::Ok().json(UploadStartResponse {
                success: false,
                fail_code: Some("INVALID_PARAM".to_string()),
                uploads: None,
                existing_files: None,
            });
        }
    }

    let mut existing_files = Vec::new();

    for file_path in &body.files {
        if !is_safe_path(file_path) {
            return HttpResponse::Ok().json(UploadStartResponse {
                success: false,
                fail_code: Some("INVALID_FILE_PATH".to_string()),
                uploads: None,
                existing_files: None,
            });
        }

        let full_path = root_path_obj.join(file_path);
        if !is_path_under_root(&full_path, root_path_obj) {
            return HttpResponse::Ok().json(UploadStartResponse {
                success: false,
                fail_code: Some("INVALID_FILE_PATH".to_string()),
                uploads: None,
                existing_files: None,
            });
        }

        if full_path.exists() {
            existing_files.push(file_path.clone());
            continue;
        }
        match app_state.upload_cache_model.file_path_exists(&user_id, file_path) {
            Ok(exists) => {
                if exists {
                    existing_files.push(file_path.clone());
                }
            }
            Err(e) => {
                return internal_error_response("/api/file/upload_start", &e);
            }
        }
    }

    if !existing_files.is_empty() {
        return HttpResponse::Ok().json(UploadStartResponse {
            success: false,
            fail_code: Some("FILES_ALREADY_EXIST".to_string()),
            uploads: None,
            existing_files: Some(existing_files),
        });
    }

    let mut uploads: Vec<UploadInfo> = Vec::new();

    for file_path in &body.files {
        let upload_id = Uuid::new_v4().to_string();
        
        let temp_dir = std::env::temp_dir();
        let temp_file_name = format!("brookfile_upload_{}", upload_id);
        let temp_file_path = temp_dir.join(&temp_file_name);

        match std::fs::File::create(&temp_file_path) {
            Ok(_) => {}
            Err(_) => {
                for prev in &uploads {
                    let prev_temp = std::env::temp_dir().join(format!("brookfile_upload_{}", prev.id));
                    let _ = std::fs::remove_file(prev_temp);
                    let _ = app_state.upload_cache_model.delete(&prev.id);
                }
                return HttpResponse::Ok().json(UploadStartResponse {
                    success: false,
                    fail_code: Some("CREATE_TEMP_FILE_FAILED".to_string()),
                    uploads: None,
                    existing_files: None,
                });
            }
        }

        let temp_file_path_str = temp_file_path.to_string_lossy().to_string();

        if let Err(e) = app_state.upload_cache_model.create(&upload_id, &user_id, file_path, &temp_file_path_str) {
            let _ = std::fs::remove_file(&temp_file_path);
            for prev in &uploads {
                let prev_temp = std::env::temp_dir().join(format!("brookfile_upload_{}", prev.id));
                let _ = std::fs::remove_file(prev_temp);
                let _ = app_state.upload_cache_model.delete(&prev.id);
            }
            if e == "FILE_ALREADY_EXISTS" {
                return HttpResponse::Ok().json(UploadStartResponse {
                    success: false,
                    fail_code: Some("FILES_ALREADY_EXIST".to_string()),
                    uploads: None,
                    existing_files: None,
                });
            }
            return internal_error_response("/api/file/upload_start", &e);
        }

        uploads.push(UploadInfo {
            id: upload_id,
            file: file_path.clone(),
        });
    }

    HttpResponse::Ok().json(UploadStartResponse {
        success: true,
        fail_code: None,
        uploads: Some(uploads),
        existing_files: None,
    })
}

pub async fn upload_chunk(
    mut payload: Multipart,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };
    let root_path_obj = Path::new(&root_path);

    let mut upload_id: Option<String> = None;
    let mut offset: Option<u64> = None;
    let mut chunk_data: Option<bytes::Bytes> = None;

    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(f) => f,
            Err(_) => {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("MULTIPART_PARSE_ERROR".to_string()),
                });
            }
        };

        let content_disposition = field.content_disposition();
        let field_name = content_disposition.get_name().unwrap_or("");

        match field_name {
            "upload_id" => {
                let mut data = Vec::new();
                while let Some(chunk) = field.next().await {
                    if let Ok(c) = chunk {
                        data.extend_from_slice(&c);
                    }
                }
                upload_id = Some(String::from_utf8_lossy(&data).to_string());
            }
            "offset" => {
                let mut data = Vec::new();
                while let Some(chunk) = field.next().await {
                    if let Ok(c) = chunk {
                        data.extend_from_slice(&c);
                    }
                }
                offset = String::from_utf8_lossy(&data).parse().ok();
            }
            "chunk" => {
                let mut data = bytes::BytesMut::new();
                while let Some(chunk) = field.next().await {
                    match chunk {
                        Ok(c) => {
                            data.extend_from_slice(&c);
                            if data.len() > 1024 * 1024 * 1024 {
                                return HttpResponse::Ok().json(ApiResponse {
                                    success: false,
                                    fail_code: Some("FILE_TOO_LARGE".to_string()),
                                });
                            }
                        }
                        Err(_) => break,
                    }
                }
                chunk_data = Some(data.freeze());
            }
            _ => {
                while let Some(chunk) = field.next().await {
                    let _ = chunk;
                }
            }
        }
    }

    let upload_id = match upload_id {
        Some(id) => id,
        None => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("MISSING_UPLOAD_ID".to_string()),
            });
        }
    };

    let offset = match offset {
        Some(o) => o,
        None => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("MISSING_OFFSET".to_string()),
            });
        }
    };

    let chunk_data = match chunk_data {
        Some(d) => d,
        None => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("MISSING_CHUNK".to_string()),
            });
        }
    };

    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let cache = match app_state.upload_cache_model.get_by_id(&upload_id) {
        Ok(Some(c)) => c,
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("UPLOAD_NOT_FOUND".to_string()),
            });
        }
        Err(e) => {
            return internal_error_response("/api/file/upload_chunk", &e);
        }
    };

    if cache.user_id != user_id {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("UPLOAD_NOT_FOUND".to_string()),
        });
    }

    let cache_target = root_path_obj.join(&cache.file_path);
    if !is_path_under_root(&cache_target, root_path_obj) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("UPLOAD_NOT_FOUND".to_string()),
        });
    }

    let temp_file_path = Path::new(&cache.temp_file_path);

    let current_size = match std::fs::metadata(temp_file_path) {
        Ok(meta) => meta.len(),
        Err(_) => 0,
    };
    if offset != current_size {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_OFFSET".to_string()),
        });
    }

    let file = match std::fs::OpenOptions::new()
        .write(true)
        .create(false)
        .open(temp_file_path)
    {
        Ok(f) => f,
        Err(_) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("FILE_WRITE_ERROR".to_string()),
            });
        }
    };

    use std::io::{Seek, SeekFrom, Write};
    let mut file = file;

    if let Err(_) = file.seek(SeekFrom::Start(offset)) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("FILE_WRITE_ERROR".to_string()),
        });
    }

    if let Err(_) = file.write_all(&chunk_data) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("FILE_WRITE_ERROR".to_string()),
        });
    }

    if let Err(e) = app_state.upload_cache_model.update_last_updated(&upload_id) {
        return internal_error_response("/api/file/upload_chunk", &e);
    }

    HttpResponse::Ok().json(ApiResponse {
        success: true,
        fail_code: None,
    })
}

pub async fn upload_complete(
    body: web::Json<UploadCompleteRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };
    let root_path_obj = Path::new(&root_path);
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let cache = match app_state.upload_cache_model.get_by_id(&body.upload_id) {
        Ok(Some(c)) => c,
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("UPLOAD_NOT_FOUND".to_string()),
            });
        }
        Err(e) => {
            return internal_error_response("/api/file/upload_complete", &e);
        }
    };

    if cache.user_id != user_id {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("UPLOAD_NOT_FOUND".to_string()),
        });
    }

    let temp_file_path = Path::new(&cache.temp_file_path);
    let target_path = root_path_obj.join(&cache.file_path);

    if !is_safe_path(&cache.file_path) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    if !is_path_under_root(&target_path, root_path_obj) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    if let Some(parent) = target_path.parent() {
        if !parent.exists() {
            if let Err(_) = std::fs::create_dir_all(parent) {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("FILE_MOVE_ERROR".to_string()),
                });
            }
        }
    }

    if let Err(_) = std::fs::rename(temp_file_path, &target_path) {
        if let Err(_) = std::fs::copy(temp_file_path, &target_path) {
            let _ = std::fs::remove_file(&target_path);
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("FILE_MOVE_ERROR".to_string()),
            });
        }
        let _ = std::fs::remove_file(temp_file_path);
    }

    if let Err(e) = app_state.upload_cache_model.delete(&body.upload_id) {
        crate::error_logger::log_error("/api/file/upload_complete", &format!("Failed to delete upload cache after successful file move: {}", e));
    }

    HttpResponse::Ok().json(ApiResponse {
        success: true,
        fail_code: None,
    })
}

pub async fn upload_cancel(
    body: web::Json<UploadCancelRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let cache = match app_state.upload_cache_model.get_by_id(&body.upload_id) {
        Ok(Some(c)) => c,
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("UPLOAD_NOT_FOUND".to_string()),
            });
        }
        Err(e) => {
            return internal_error_response("/api/file/upload_cancel", &e);
        }
    };

    if cache.user_id != user_id {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("UPLOAD_NOT_FOUND".to_string()),
        });
    }

    let temp_file_path = Path::new(&cache.temp_file_path);
    if temp_file_path.exists() {
        let _ = std::fs::remove_file(temp_file_path);
    }

    if let Err(e) = app_state.upload_cache_model.delete(&body.upload_id) {
        return internal_error_response("/api/file/upload_cancel", &e);
    }

    HttpResponse::Ok().json(ApiResponse {
        success: true,
        fail_code: None,
    })
}
