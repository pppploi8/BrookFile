use actix_web::{web, HttpRequest, HttpResponse, Responder};
use actix_multipart::Multipart;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::Path;
use crate::app_state::AppState;
use crate::handlers::{ApiResponse, internal_error_response, get_current_user_id, get_user_root_path, is_safe_path, is_safe_name, is_path_under_root};
use crate::models::{CreateVaultData, ImportVaultData, VaultInfo};

#[derive(Serialize)]
pub struct VaultListItemResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub path: String,
    pub filename: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl From<VaultInfo> for VaultListItemResponse {
    fn from(item: VaultInfo) -> Self {
        VaultListItemResponse {
            id: item.id,
            name: item.name,
            description: item.description,
            path: item.path,
            filename: item.filename,
            created_at: item.created_at,
            updated_at: item.updated_at,
        }
    }
}

#[derive(Serialize)]
pub struct VaultListResponse {
    pub vaults: Vec<VaultListItemResponse>,
}

pub async fn list_vaults(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    match app_state.vault_model.list_by_user(&user_id) {
        Ok(vaults) => {
            let responses: Vec<VaultListItemResponse> = vaults.into_iter().map(|v| v.into()).collect();
            HttpResponse::Ok().json(VaultListResponse { vaults: responses })
        }
        Err(e) => internal_error_response("/api/vault/list", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateVaultRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub path: String,
    pub filename: String,
    pub file_data: String,
}

#[derive(Serialize)]
pub struct CreateVaultResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

pub async fn create_vault(
    req: web::Json<CreateVaultRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };

    let path = if req.path == "/" { "" } else { &req.path };

    if req.name.is_empty() {
        return HttpResponse::Ok().json(CreateVaultResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
            id: None,
        });
    }

    if req.filename.is_empty() {
        return HttpResponse::Ok().json(CreateVaultResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            id: None,
        });
    }

    if !is_safe_path(path) {
        return HttpResponse::Ok().json(CreateVaultResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            id: None,
        });
    }

    if !is_safe_name(&req.filename) {
        return HttpResponse::Ok().json(CreateVaultResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            id: None,
        });
    }

    let file_data = match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &req.file_data) {
        Ok(data) => data,
        Err(_) => {
            return HttpResponse::Ok().json(CreateVaultResponse {
                success: false,
                fail_code: Some("INVALID_FILE_DATA".to_string()),
                id: None,
            });
        }
    };

    let root_path_obj = Path::new(&root_path);
    let file_dir = root_path_obj.join(path);
    let file_path = file_dir.join(&req.filename);

    if root_path.is_empty() || !is_path_under_root(&file_path, root_path_obj) {
        return HttpResponse::Ok().json(CreateVaultResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            id: None,
        });
    }

    match app_state.vault_model.is_file_imported(&user_id, path, &req.filename) {
        Ok(true) => {
            return HttpResponse::Ok().json(CreateVaultResponse {
                success: false,
                fail_code: Some("VAULT_ALREADY_EXISTS".to_string()),
                id: None,
            });
        }
        Ok(false) => {}
        Err(e) => return internal_error_response("/api/vault/create", &e),
    }

    if file_path.exists() {
        return HttpResponse::Ok().json(CreateVaultResponse {
            success: false,
            fail_code: Some("FILE_ALREADY_EXISTS".to_string()),
            id: None,
        });
    }

    if let Some(parent) = file_path.parent() {
        if !parent.exists() {
            if let Err(_) = std::fs::create_dir_all(parent) {
                return HttpResponse::Ok().json(CreateVaultResponse {
                    success: false,
                    fail_code: Some("FILE_WRITE_ERROR".to_string()),
                    id: None,
                });
            }
        }
    }

    match std::fs::write(&file_path, &file_data) {
        Ok(_) => {}
        Err(_) => {
            return HttpResponse::Ok().json(CreateVaultResponse {
                success: false,
                fail_code: Some("FILE_WRITE_ERROR".to_string()),
                id: None,
            });
        }
    }

    let data = CreateVaultData {
        name: req.name.clone(),
        description: req.description.clone(),
        path: path.to_string(),
        filename: req.filename.clone(),
    };

    match app_state.vault_model.create(&user_id, &data) {
        Ok(vault_id) => HttpResponse::Ok().json(CreateVaultResponse {
            success: true,
            fail_code: None,
            id: Some(vault_id),
        }),
        Err(e) => {
            let _ = std::fs::remove_file(&file_path);
            internal_error_response("/api/vault/create", &e)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateVaultRequest {
    pub id: String,
    pub file_data: Option<String>,
}

pub async fn update_vault(
    req: web::Json<UpdateVaultRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let vault = match app_state.vault_model.get_by_id(&req.id, &user_id) {
        Ok(Some(v)) => v,
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("VAULT_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/vault/update", &e),
    };

    let file_data_b64 = match &req.file_data {
        Some(data) => data,
        None => {
            return HttpResponse::Ok().json(ApiResponse {
                success: true,
                fail_code: None,
            });
        }
    };

    let file_data = match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, file_data_b64) {
        Ok(data) => data,
        Err(_) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INVALID_FILE_DATA".to_string()),
            });
        }
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };

    let root_path_obj = Path::new(&root_path);
    let file_path = root_path_obj.join(&vault.path).join(&vault.filename);

    if let Some(parent) = file_path.parent() {
        if !parent.exists() {
            if let Err(_) = std::fs::create_dir_all(parent) {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("FILE_WRITE_ERROR".to_string()),
                });
            }
        }
    }

    match std::fs::write(&file_path, &file_data) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        }),
        Err(_) => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("FILE_WRITE_ERROR".to_string()),
        }),
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateVaultMetaRequest {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
}

pub async fn update_vault_meta(
    req: web::Json<UpdateVaultMetaRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    if let Some(ref name) = req.name {
        if name.is_empty() {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INVALID_PARAM".to_string()),
            });
        }
    }

    match app_state.vault_model.update(&req.id, &user_id, &req.name, &req.description) {
        Ok(true) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        }),
        Ok(false) => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("VAULT_NOT_FOUND".to_string()),
        }),
        Err(e) => internal_error_response("/api/vault/update_meta", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct DeleteVaultRequest {
    pub id: String,
}

pub async fn delete_vault(
    req: web::Json<DeleteVaultRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    match app_state.vault_model.delete(&req.id, &user_id) {
        Ok(true) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        }),
        Ok(false) => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("VAULT_NOT_FOUND".to_string()),
        }),
        Err(e) => internal_error_response("/api/vault/delete", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct ImportVaultRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub file_path: String,
}

#[derive(Serialize)]
pub struct ImportVaultResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

pub async fn import_vault(
    req: web::Json<ImportVaultRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };

    if req.name.is_empty() {
        return HttpResponse::Ok().json(ImportVaultResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
            id: None,
        });
    }

    let file_path = &req.file_path;

    if !is_safe_path(file_path) {
        return HttpResponse::Ok().json(ImportVaultResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            id: None,
        });
    }

    let root_path_obj = Path::new(&root_path);
    let target_file = root_path_obj.join(file_path);

    if root_path.is_empty() || !is_path_under_root(&target_file, root_path_obj) {
        return HttpResponse::Ok().json(ImportVaultResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            id: None,
        });
    }

    if !target_file.is_file() {
        return HttpResponse::Ok().json(ImportVaultResponse {
            success: false,
            fail_code: Some("FILE_NOT_FOUND".to_string()),
            id: None,
        });
    }

    let (path, filename) = match file_path.rfind('/') {
        Some(idx) => (file_path[..idx].to_string(), file_path[idx + 1..].to_string()),
        None => (String::new(), file_path.to_string()),
    };
    if filename.is_empty() || !is_safe_name(&filename) {
        return HttpResponse::Ok().json(ImportVaultResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            id: None,
        });
    }

    match app_state.vault_model.is_file_imported(&user_id, &path, &filename) {
        Ok(true) => {
            return HttpResponse::Ok().json(ImportVaultResponse {
                success: false,
                fail_code: Some("VAULT_ALREADY_EXISTS".to_string()),
                id: None,
            });
        }
        Ok(false) => {}
        Err(e) => {
            return internal_error_response("/api/vault/import", &e);
        }
    }

    let data = ImportVaultData {
        name: req.name.clone(),
        description: req.description.clone(),
        path,
        filename,
    };

    match app_state.vault_model.import(&user_id, &data) {
        Ok(vault_id) => HttpResponse::Ok().json(ImportVaultResponse {
            success: true,
            fail_code: None,
            id: Some(vault_id),
        }),
        Err(e) => internal_error_response("/api/vault/import", &e),
    }
}

pub async fn upload_single(
    mut payload: Multipart,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };
    let root_path_obj = Path::new(&root_path);

    let mut file_path: Option<String> = None;
    let mut file_data: Option<Vec<u8>> = None;

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
            "path" => {
                let mut bytes = Vec::new();
                while let Some(chunk) = field.next().await {
                    match chunk {
                        Ok(data) => bytes.extend_from_slice(&data),
                        Err(_) => {
                            return HttpResponse::Ok().json(ApiResponse {
                                success: false,
                                fail_code: Some("MULTIPART_PARSE_ERROR".to_string()),
                            });
                        }
                    }
                }
                file_path = Some(String::from_utf8_lossy(&bytes).to_string());
            }
            "file" => {
                let mut bytes = Vec::new();
                while let Some(chunk) = field.next().await {
                    match chunk {
                        Ok(data) => {
                            bytes.extend_from_slice(&data);
                            if bytes.len() > 1024 * 1024 * 1024 {
                                return HttpResponse::Ok().json(ApiResponse {
                                    success: false,
                                    fail_code: Some("FILE_TOO_LARGE".to_string()),
                                });
                            }
                        }
                        Err(_) => {
                            return HttpResponse::Ok().json(ApiResponse {
                                success: false,
                                fail_code: Some("MULTIPART_PARSE_ERROR".to_string()),
                            });
                        }
                    }
                }
                file_data = Some(bytes);
            }
            _ => {}
        }
    }

    let path = match file_path {
        Some(p) if !p.is_empty() => p,
        _ => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INVALID_FILE_PATH".to_string()),
            });
        }
    };

    if !is_safe_path(&path) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    let file_name = Path::new(&path).file_name().and_then(|n| n.to_str()).unwrap_or("");
    if !file_name.is_empty() && !is_safe_name(file_name) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    let data = match file_data {
        Some(d) => d,
        None => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("MULTIPART_PARSE_ERROR".to_string()),
            });
        }
    };

    let target = root_path_obj.join(&path);

    if !is_path_under_root(&target, root_path_obj) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    if let Some(parent) = target.parent() {
        if let Err(_) = std::fs::create_dir_all(parent) {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("FILE_WRITE_ERROR".to_string()),
            });
        }
    }

    match std::fs::write(&target, &data) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        }),
        Err(_) => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("FILE_WRITE_ERROR".to_string()),
        }),
    }
}
