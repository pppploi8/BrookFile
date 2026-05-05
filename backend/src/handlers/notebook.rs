use actix_web::{web, HttpRequest, HttpResponse, Responder};
use actix_multipart::Multipart;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use sha2::{Sha256, Digest};
use base64::Engine;
use crate::app_state::AppState;
use crate::handlers::{ApiResponse, internal_error_response, get_current_user_id, get_user_root_path, is_safe_path, is_safe_name, is_path_under_root, delete_file_with_recycle, get_recycle_bin_path};
use crate::models::NotebookInfo;
use crate::search::{SearchResult, SearchManager};
use crate::error_logger;

#[derive(Debug, Deserialize)]
pub struct SearchNotesRequest {
    pub keyword: String,
    pub notebook_id: Option<String>,
}

#[derive(Serialize)]
pub struct SearchNotesResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    pub results: Vec<SearchResult>,
}

pub async fn search_notes(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
    body: web::Json<SearchNotesRequest>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    if body.keyword.trim().is_empty() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
        });
    }

    let notebooks = match app_state.notebook_model.list_by_user(&user_id) {
        Ok(list) => list,
        Err(e) => return internal_error_response("/api/notebook/search", &e),
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(p) => p,
        Err(response) => return response,
    };

    match SearchManager::search(&app_state.search_manager, &body.keyword, body.notebook_id.as_deref(), notebooks, &root_path) {
        Ok(output) => {
            if output.failed_notebooks > 0 && output.failed_notebooks == output.total_notebooks {
                HttpResponse::Ok().json(SearchNotesResponse {
                    success: false,
                    fail_code: Some("SEARCH_FAILED".to_string()),
                    results: vec![],
                })
            } else {
                HttpResponse::Ok().json(SearchNotesResponse {
                    success: true,
                    fail_code: None,
                    results: output.results,
                })
            }
        }
        Err(e) => internal_error_response("/api/notebook/search", &e),
    }
}

#[derive(Serialize)]
pub struct NotebookListItemResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub path: String,
    pub encrypted: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl From<NotebookInfo> for NotebookListItemResponse {
    fn from(item: NotebookInfo) -> Self {
        NotebookListItemResponse {
            id: item.id,
            name: item.name,
            description: item.description,
            path: item.path,
            encrypted: item.encrypted,
            created_at: item.created_at,
            updated_at: item.updated_at,
        }
    }
}

#[derive(Serialize)]
pub struct NotebookListResponse {
    pub success: bool,
    pub notebooks: Vec<NotebookListItemResponse>,
}

pub async fn list_notebooks(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    match app_state.notebook_model.list_by_user(&user_id) {
        Ok(notebooks) => {
            let responses: Vec<NotebookListItemResponse> = notebooks.into_iter().map(|n| n.into()).collect();
            HttpResponse::Ok().json(NotebookListResponse { success: true, notebooks: responses })
        }
        Err(e) => internal_error_response("/api/notebook/list", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateNotebookRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub path: String,
    pub encrypted: Option<bool>,
    pub signature: Option<String>,
}

#[derive(Serialize)]
pub struct CreateNotebookResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

fn validate_signature_json(sig: &str) -> bool {
    let v: serde_json::Value = match serde_json::from_str(sig) {
        Ok(v) => v,
        Err(_) => return false,
    };
    v.get("salt").and_then(|v| v.as_str()).is_some()
        && v.get("iv").and_then(|v| v.as_str()).is_some()
        && v.get("rounds").and_then(|v| v.as_i64()).is_some()
        && v.get("signature").and_then(|v| v.as_str()).is_some()
}

pub async fn create_notebook(
    req: web::Json<CreateNotebookRequest>,
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

    let name = req.name.trim().to_string();
    if name.is_empty() {
        return HttpResponse::Ok().json(CreateNotebookResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
            id: None,
        });
    }

    let encrypted = req.encrypted.unwrap_or(false);

    if encrypted {
        match &req.signature {
            Some(sig) if !sig.is_empty() && validate_signature_json(sig) => {}
            _ => {
                return HttpResponse::Ok().json(CreateNotebookResponse {
                    success: false,
                    fail_code: Some("INVALID_PARAM".to_string()),
                    id: None,
                });
            }
        }
    }

    let path = if req.path == "/" { "".to_string() } else { req.path.trim_end_matches('/').to_string() };

    if path.is_empty() {
        return HttpResponse::Ok().json(CreateNotebookResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
            id: None,
        });
    }

    if !is_safe_path(&path) {
        return HttpResponse::Ok().json(CreateNotebookResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            id: None,
        });
    }

    if path == "attachment" || path.starts_with("attachment/") {
        return HttpResponse::Ok().json(CreateNotebookResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
            id: None,
        });
    }

    if let Ok(Some(_)) = app_state.notebook_model.get_by_user_and_path(&user_id, &path) {
        return HttpResponse::Ok().json(CreateNotebookResponse {
            success: false,
            fail_code: Some("DUPLICATE_NOTEBOOK_PATH".to_string()),
            id: None,
        });
    }

    let root_path_obj = Path::new(&root_path);
    let target_path = root_path_obj.join(&path);

    if !is_path_under_root(&target_path, root_path_obj) {
        return HttpResponse::Ok().json(CreateNotebookResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            id: None,
        });
    }

    let dir_created_this_time;
    if !target_path.exists() {
        if let Err(_) = std::fs::create_dir_all(&target_path) {
            return HttpResponse::Ok().json(CreateNotebookResponse {
                success: false,
                fail_code: Some("FILE_WRITE_ERROR".to_string()),
                id: None,
            });
        }
        dir_created_this_time = true;
    } else if target_path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&target_path) {
            if entries.count() > 0 {
                return HttpResponse::Ok().json(CreateNotebookResponse {
                    success: false,
                    fail_code: Some("PATH_NOT_EMPTY".to_string()),
                    id: None,
                });
            }
        }
        dir_created_this_time = false;
    } else {
        return HttpResponse::Ok().json(CreateNotebookResponse {
            success: false,
            fail_code: Some("PATH_NOT_FOUND".to_string()),
            id: None,
        });
    }

    let sig_path_written = if encrypted {
        match check_nested_encrypted(&app_state, &user_id, &path) {
            Err(response) => return response,
            Ok(()) => {}
        }

        let sig_path = target_path.join(".notebook.sig");
        if let Err(_) = std::fs::write(&sig_path, req.signature.as_ref().unwrap()) {
            return HttpResponse::Ok().json(CreateNotebookResponse {
                success: false,
                fail_code: Some("FILE_WRITE_ERROR".to_string()),
                id: None,
            });
        }
        Some(sig_path)
    } else {
        None
    };

    match app_state.notebook_model.create(&user_id, &name, &req.description, &path, encrypted) {
        Ok(id) => {
            if !encrypted {
                let search_mgr = Arc::clone(&app_state.search_manager);
                let nb_id = id.clone();
                std::thread::spawn(move || {
                    if let Err(e) = search_mgr.init_search_db(&nb_id) {
                        error_logger::log_error("search/init_search_db", &e);
                    }
                });
            }
            HttpResponse::Ok().json(CreateNotebookResponse {
                success: true,
                fail_code: None,
                id: Some(id),
            })
        }
        Err(e) => {
            if let Some(ref sig_path) = sig_path_written {
                let _ = std::fs::remove_file(sig_path);
            }
            if dir_created_this_time {
                let _ = std::fs::remove_dir(&target_path);
            }
            internal_error_response("/api/notebook/create", &e)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct OpenNotebookRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub path: String,
    pub encrypted: Option<bool>,
}

pub async fn open_notebook(
    req: web::Json<OpenNotebookRequest>,
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

    if req.name.trim().is_empty() {
        return HttpResponse::Ok().json(CreateNotebookResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
            id: None,
        });
    }

    let name = req.name.trim().to_string();

    let path = if req.path == "/" { "".to_string() } else { req.path.trim_end_matches('/').to_string() };

    if path.is_empty() {
        return HttpResponse::Ok().json(CreateNotebookResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
            id: None,
        });
    }

    if !is_safe_path(&path) {
        return HttpResponse::Ok().json(CreateNotebookResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            id: None,
        });
    }

    if path == "attachment" || path.starts_with("attachment/") {
        return HttpResponse::Ok().json(CreateNotebookResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
            id: None,
        });
    }

    let root_path_obj = Path::new(&root_path);
    let target_path = root_path_obj.join(&path);

    if let Ok(Some(_)) = app_state.notebook_model.get_by_user_and_path(&user_id, &path) {
        return HttpResponse::Ok().json(CreateNotebookResponse {
            success: false,
            fail_code: Some("DUPLICATE_NOTEBOOK_PATH".to_string()),
            id: None,
        });
    }

    if !is_path_under_root(&target_path, root_path_obj) {
        return HttpResponse::Ok().json(CreateNotebookResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            id: None,
        });
    }

    if !target_path.exists() || !target_path.is_dir() {
        return HttpResponse::Ok().json(CreateNotebookResponse {
            success: false,
            fail_code: Some("PATH_NOT_FOUND".to_string()),
            id: None,
        });
    }

    let encrypted = req.encrypted.unwrap_or(false);

    if encrypted {
        let sig_path = target_path.join(".notebook.sig");
        if !sig_path.exists() {
            return HttpResponse::Ok().json(CreateNotebookResponse {
                success: false,
                fail_code: Some("INVALID_PARAM".to_string()),
                id: None,
            });
        }
        let sig_content = match std::fs::read_to_string(&sig_path) {
            Ok(c) => c,
            Err(_) => {
                return HttpResponse::Ok().json(CreateNotebookResponse {
                    success: false,
                    fail_code: Some("INVALID_PARAM".to_string()),
                    id: None,
                });
            }
        };
        if !validate_signature_json(&sig_content) {
            return HttpResponse::Ok().json(CreateNotebookResponse {
                success: false,
                fail_code: Some("INVALID_PARAM".to_string()),
                id: None,
            });
        }

        match check_nested_encrypted(&app_state, &user_id, &path) {
            Err(response) => return response,
            Ok(()) => {}
        }
    }

    match app_state.notebook_model.create(&user_id, &name, &req.description, &path, encrypted) {
        Ok(id) => {
            if !encrypted {
                let search_mgr = Arc::clone(&app_state.search_manager);
                let nb_id = id.clone();
                let nb_root = target_path.to_string_lossy().to_string();
                std::thread::spawn(move || {
                    if let Err(e) = search_mgr.rebuild_notebook_index(&nb_id, &nb_root) {
                        error_logger::log_error("search/rebuild_notebook_index", &e);
                    }
                });
            }
            HttpResponse::Ok().json(CreateNotebookResponse {
                success: true,
                fail_code: None,
                id: Some(id),
            })
        }
        Err(e) => {
            internal_error_response("/api/notebook/open", &e)
        }
    }
}

fn check_nested_encrypted(app_state: &web::Data<AppState>, user_id: &str, target_path: &str) -> Result<(), HttpResponse> {
    let encrypted_notebooks = match app_state.notebook_model.list_encrypted_by_user(user_id) {
        Ok(list) => list,
        Err(e) => return Err(internal_error_response("/api/notebook", &e)),
    };

    for nb in &encrypted_notebooks {
        let nb_path = &nb.path;
        let target_is_parent_of_nb = nb_path == target_path || nb_path.starts_with(&format!("{}/", target_path));
        let nb_is_parent_of_target = target_path == nb_path || target_path.starts_with(&format!("{}/", nb_path));

        if target_is_parent_of_nb || nb_is_parent_of_target {
            return Err(HttpResponse::Ok().json(CreateNotebookResponse {
                success: false,
                fail_code: Some("NESTED_ENCRYPTED_NOT_ALLOWED".to_string()),
                id: None,
            }));
        }
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct UpdateNotebookRequest {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

pub async fn update_notebook(
    req: web::Json<UpdateNotebookRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let name = req.name.trim().to_string();
    if name.is_empty() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
        });
    }

    let description = match &req.description {
        Some(d) => d.clone(),
        None => {
            match app_state.notebook_model.get_by_id_and_user(&req.id, &user_id) {
                Ok(Some(nb)) => nb.description.clone(),
                Ok(None) => {
                    return HttpResponse::Ok().json(ApiResponse {
                        success: false,
                        fail_code: Some("NOTEBOOK_NOT_FOUND".to_string()),
                    });
                }
                Err(e) => return internal_error_response("/api/notebook/update", &e),
            }
        }
    };

    match app_state.notebook_model.update(&req.id, &user_id, &name, &description) {
        Ok(true) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        }),
        Ok(false) => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("NOTEBOOK_NOT_FOUND".to_string()),
        }),
        Err(e) => internal_error_response("/api/notebook/update", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct DeleteNotebookRequest {
    pub id: String,
}

pub async fn delete_notebook(
    req: web::Json<DeleteNotebookRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    match app_state.notebook_model.delete(&req.id, &user_id) {
        Ok(true) => {
            let search_mgr = Arc::clone(&app_state.search_manager);
            let nb_id = req.id.clone();
            std::thread::spawn(move || {
                if let Err(e) = search_mgr.delete_search_db(&nb_id) {
                    error_logger::log_error("search/delete_search_db", &e);
                }
            });

            HttpResponse::Ok().json(ApiResponse {
                success: true,
                fail_code: None,
            })
        }
        Ok(false) => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("NOTEBOOK_NOT_FOUND".to_string()),
        }),
        Err(e) => internal_error_response("/api/notebook/delete", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct ReadNoteRequest {
    pub notebook_id: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct ReadNoteResponse {
    pub success: bool,
    pub content: String,
    pub hash: String,
}

#[derive(Serialize)]
pub struct ReadNoteErrorResponse {
    pub success: bool,
    pub fail_code: String,
}

pub async fn read_note(
    req: web::Json<ReadNoteRequest>,
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

    let notebook = match app_state.notebook_model.get_by_id_and_user(&req.notebook_id, &user_id) {
        Ok(Some(nb)) => nb,
        Ok(None) => {
            return HttpResponse::Ok().json(ReadNoteErrorResponse {
                success: false,
                fail_code: "NOTEBOOK_NOT_FOUND".to_string(),
            });
        }
        Err(e) => return internal_error_response("/api/notebook/read_note", &e),
    };

    if !is_safe_path(&req.path) {
        return HttpResponse::Ok().json(ReadNoteErrorResponse {
            success: false,
            fail_code: "INVALID_FILE_PATH".to_string(),
        });
    }

    let path_file_name = Path::new(&req.path).file_name().and_then(|n| n.to_str()).unwrap_or("");
    if path_file_name == ".notebook.sig" && !notebook.encrypted {
        return HttpResponse::Ok().json(ReadNoteErrorResponse {
            success: false,
            fail_code: "INVALID_FILE_PATH".to_string(),
        });
    }

    let root_path_obj = Path::new(&root_path);
    let full_path = root_path_obj.join(&notebook.path).join(&req.path);

    if !is_path_under_root(&full_path, root_path_obj) {
        return HttpResponse::Ok().json(ReadNoteErrorResponse {
            success: false,
            fail_code: "INVALID_FILE_PATH".to_string(),
        });
    }

    let content = match std::fs::read(&full_path) {
        Ok(bytes) => {
            match String::from_utf8(bytes) {
                Ok(s) => s,
                Err(_) => {
                    return HttpResponse::Ok().json(ReadNoteErrorResponse {
                        success: false,
                        fail_code: "FILE_READ_ERROR".to_string(),
                    });
                }
            }
        }
        Err(_) => {
            return HttpResponse::Ok().json(ReadNoteErrorResponse {
                success: false,
                fail_code: "FILE_NOT_FOUND".to_string(),
            });
        }
    };

    let mut hasher = Sha256::new();
    hasher.update(&content);
    let hash = hex::encode(hasher.finalize());

    HttpResponse::Ok().json(ReadNoteResponse {
        success: true,
        content,
        hash,
    })
}

#[derive(Debug, Deserialize)]
pub struct SaveNoteRequest {
    pub notebook_id: String,
    pub path: String,
    pub content: String,
    pub hash: Option<String>,
}

#[derive(Serialize)]
pub struct SaveNoteResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateNotebookFolderRequest {
    pub notebook_id: String,
    pub path: String,
}

pub async fn create_notebook_folder(
    req: web::Json<CreateNotebookFolderRequest>,
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

    let notebook = match app_state.notebook_model.get_by_id_and_user(&req.notebook_id, &user_id) {
        Ok(Some(nb)) => nb,
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("NOTEBOOK_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/notebook/create_folder", &e),
    };

    if !is_safe_path(&req.path) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    if req.path.split('/').any(|s| s == ".notebook.sig") {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    if req.path.split('/').any(|s| s == "attachment") || req.path == "attachment" {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
        });
    }

    let root_path_obj = Path::new(&root_path);
    let full_path = root_path_obj.join(&notebook.path).join(&req.path);

    if !is_path_under_root(&full_path, root_path_obj) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    if full_path.exists() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("FOLDER_ALREADY_EXISTS".to_string()),
        });
    }

    match std::fs::create_dir_all(&full_path) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        }),
        Err(e) => internal_error_response("/api/notebook/create_folder", &e.to_string()),
    }
}

pub async fn save_note(
    req: web::Json<SaveNoteRequest>,
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

    let notebook = match app_state.notebook_model.get_by_id_and_user(&req.notebook_id, &user_id) {
        Ok(Some(nb)) => nb,
        Ok(None) => {
            return HttpResponse::Ok().json(SaveNoteResponse {
                success: false,
                fail_code: Some("NOTEBOOK_NOT_FOUND".to_string()),
                hash: None,
                server_content: None,
                server_hash: None,
            });
        }
        Err(e) => return internal_error_response("/api/notebook/save_note", &e),
    };

    if !is_safe_path(&req.path) {
        return HttpResponse::Ok().json(SaveNoteResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            hash: None,
            server_content: None,
            server_hash: None,
        });
    }

    let path_file_name = Path::new(&req.path).file_name().and_then(|n| n.to_str()).unwrap_or("");
    if path_file_name == ".notebook.sig" {
        return HttpResponse::Ok().json(SaveNoteResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            hash: None,
            server_content: None,
            server_hash: None,
        });
    }

    if req.path.split('/').any(|s| s == "attachment") || req.path == "attachment" {
        return HttpResponse::Ok().json(SaveNoteResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            hash: None,
            server_content: None,
            server_hash: None,
        });
    }

    let root_path_obj = Path::new(&root_path);
    let full_path = root_path_obj.join(&notebook.path).join(&req.path);

    if !is_path_under_root(&full_path, root_path_obj) {
        return HttpResponse::Ok().json(SaveNoteResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            hash: None,
            server_content: None,
            server_hash: None,
        });
    }

    match &req.hash {
        None => {
            if full_path.exists() {
                return HttpResponse::Ok().json(SaveNoteResponse {
                    success: false,
                    fail_code: Some("FILE_ALREADY_EXISTS".to_string()),
                    hash: None,
                    server_content: None,
                    server_hash: None,
                });
            }

            if let Some(parent) = full_path.parent() {
                if !parent.exists() {
                    if let Err(_) = std::fs::create_dir_all(parent) {
                        return HttpResponse::Ok().json(SaveNoteResponse {
                            success: false,
                            fail_code: Some("FILE_WRITE_ERROR".to_string()),
                            hash: None,
                            server_content: None,
                            server_hash: None,
                        });
                    }
                }
            }

            match std::fs::write(&full_path, &req.content) {
                Ok(_) => {
                    let mut hasher = Sha256::new();
                    hasher.update(&req.content);
                    let new_hash = hex::encode(hasher.finalize());

                    if !notebook.encrypted {
                        let search_mgr = Arc::clone(&app_state.search_manager);
                        let nb_id = req.notebook_id.clone();
                        let note_path = req.path.clone();
                        let content = req.content.clone();
                        let notebook_root = root_path_obj.join(&notebook.path).to_string_lossy().to_string();
                        std::thread::spawn(move || {
                            if let Err(e) = search_mgr.index_note(&nb_id, &note_path, &content, &notebook_root) {
                                error_logger::log_error("search/index_note", &e);
                            }
                        });
                    }

                    HttpResponse::Ok().json(SaveNoteResponse {
                        success: true,
                        fail_code: None,
                        hash: Some(new_hash),
                        server_content: None,
                        server_hash: None,
                    })
                }
                Err(_) => HttpResponse::Ok().json(SaveNoteResponse {
                    success: false,
                    fail_code: Some("FILE_WRITE_ERROR".to_string()),
                    hash: None,
                    server_content: None,
                    server_hash: None,
                }),
            }
        }
        Some(expected_hash) => {
            if !full_path.exists() {
                return HttpResponse::Ok().json(SaveNoteResponse {
                    success: false,
                    fail_code: Some("FILE_NOT_FOUND".to_string()),
                    hash: None,
                    server_content: None,
                    server_hash: None,
                });
            }

            let current_content = match std::fs::read_to_string(&full_path) {
                Ok(c) => c,
                Err(_) => {
                    return HttpResponse::Ok().json(SaveNoteResponse {
                        success: false,
                        fail_code: Some("FILE_READ_ERROR".to_string()),
                        hash: None,
                        server_content: None,
                        server_hash: None,
                    });
                }
            };

            let mut hasher = Sha256::new();
            hasher.update(&current_content);
            let current_hash = hex::encode(hasher.finalize());

            if current_hash != *expected_hash {
                return HttpResponse::Ok().json(SaveNoteResponse {
                    success: false,
                    fail_code: Some("CONFLICT_DETECTED".to_string()),
                    hash: None,
                    server_content: Some(current_content),
                    server_hash: Some(current_hash),
                });
            }

            match std::fs::write(&full_path, &req.content) {
                Ok(_) => {
                    let mut hasher2 = Sha256::new();
                    hasher2.update(&req.content);
                    let new_hash = hex::encode(hasher2.finalize());

                    if !notebook.encrypted {
                        let search_mgr = Arc::clone(&app_state.search_manager);
                        let nb_id = req.notebook_id.clone();
                        let note_path = req.path.clone();
                        let content = req.content.clone();
                        let notebook_root = root_path_obj.join(&notebook.path).to_string_lossy().to_string();
                        std::thread::spawn(move || {
                            if let Err(e) = search_mgr.index_note(&nb_id, &note_path, &content, &notebook_root) {
                                error_logger::log_error("search/index_note", &e);
                            }
                        });
                    }

                    HttpResponse::Ok().json(SaveNoteResponse {
                        success: true,
                        fail_code: None,
                        hash: Some(new_hash),
                        server_content: None,
                        server_hash: None,
                    })
                }
                Err(_) => HttpResponse::Ok().json(SaveNoteResponse {
                    success: false,
                    fail_code: Some("FILE_WRITE_ERROR".to_string()),
                    hash: None,
                    server_content: None,
                    server_hash: None,
                }),
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SaveConflictRequest {
    pub notebook_id: String,
    pub path: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct SaveConflictResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

pub async fn save_conflict(
    req: web::Json<SaveConflictRequest>,
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

    let notebook = match app_state.notebook_model.get_by_id_and_user(&req.notebook_id, &user_id) {
        Ok(Some(nb)) => nb,
        Ok(None) => {
            return HttpResponse::Ok().json(SaveConflictResponse {
                success: false,
                fail_code: Some("NOTEBOOK_NOT_FOUND".to_string()),
                conflict_path: None,
                hash: None,
            });
        }
        Err(e) => return internal_error_response("/api/notebook/save_conflict", &e),
    };

    if notebook.encrypted {
        return HttpResponse::Ok().json(SaveConflictResponse {
            success: false,
            fail_code: Some("ENCRYPTED_NOTEBOOK".to_string()),
            conflict_path: None,
            hash: None,
        });
    }

    if !is_safe_path(&req.path) {
        return HttpResponse::Ok().json(SaveConflictResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            conflict_path: None,
            hash: None,
        });
    }

    let root_path_obj = Path::new(&root_path);
    let dir_path = root_path_obj.join(&notebook.path);

    let path_obj = Path::new(&req.path);
    let dir_part = path_obj.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let filename = path_obj.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();

    let base_name = filename.strip_suffix(".md").unwrap_or(&filename);
    let conflict_name = format!("{}_conflict.md", base_name);

    let conflict_full_path = if dir_part.is_empty() {
        dir_path.join(&conflict_name)
    } else {
        dir_path.join(&dir_part).join(&conflict_name)
    };

    if !is_path_under_root(&conflict_full_path, root_path_obj) {
        return HttpResponse::Ok().json(SaveConflictResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            conflict_path: None,
            hash: None,
        });
    }

    let final_path = if conflict_full_path.exists() {
        let mut i = 1;
        loop {
            let indexed_name = format!("{}_conflict_{}.md", base_name, i);
            let candidate = if dir_part.is_empty() {
                dir_path.join(&indexed_name)
            } else {
                dir_path.join(&dir_part).join(&indexed_name)
            };
            if !candidate.exists() {
                break candidate;
            }
            i += 1;
        }
    } else {
        conflict_full_path
    };

    if let Some(parent) = final_path.parent() {
        if !parent.exists() {
            if let Err(_) = std::fs::create_dir_all(parent) {
                return HttpResponse::Ok().json(SaveConflictResponse {
                    success: false,
                    fail_code: Some("FILE_WRITE_ERROR".to_string()),
                    conflict_path: None,
                    hash: None,
                });
            }
        }
    }

    match std::fs::write(&final_path, &req.content) {
        Ok(_) => {
            let new_relative = if dir_part.is_empty() {
                final_path.file_name().unwrap().to_string_lossy().to_string()
            } else {
                format!("{}/{}", dir_part, final_path.file_name().unwrap().to_string_lossy())
            };

            let mut hasher = Sha256::new();
            hasher.update(&req.content);
            let content_hash = hex::encode(hasher.finalize());

            let search_mgr = Arc::clone(&app_state.search_manager);
            let nb_id = req.notebook_id.clone();
            let note_path = new_relative.clone();
            let content = req.content.clone();
            let notebook_root = root_path_obj.join(&notebook.path).to_string_lossy().to_string();
            std::thread::spawn(move || {
                if let Err(e) = search_mgr.index_note(&nb_id, &note_path, &content, &notebook_root) {
                    error_logger::log_error("search/index_note", &e);
                }
            });

            HttpResponse::Ok().json(SaveConflictResponse {
                success: true,
                fail_code: None,
                conflict_path: Some(new_relative),
                hash: Some(content_hash),
            })
        }
        Err(_) => HttpResponse::Ok().json(SaveConflictResponse {
            success: false,
            fail_code: Some("FILE_WRITE_ERROR".to_string()),
            conflict_path: None,
            hash: None,
        }),
    }
}

#[derive(Debug, Deserialize)]
pub struct FileTreeRequest {
    pub notebook_id: String,
}

#[derive(Serialize, Clone)]
pub struct FileTreeNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Option<Vec<FileTreeNode>>,
}

#[derive(Serialize)]
pub struct FileTreeResponse {
    pub success: bool,
    pub tree: Vec<FileTreeNode>,
}

fn build_tree(dir: &Path, base: &Path, inside_attachment: bool) -> Result<Vec<FileTreeNode>, String> {
    let mut entries: Vec<std::fs::DirEntry> = Vec::new();
    let rd = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
    for entry in rd.flatten() {
        let filename = entry.file_name();
        let filename_str = filename.to_string_lossy().to_string();
        if filename_str == ".notebook.sig" {
            continue;
        }
        entries.push(entry);
    }

    entries.sort_by(|a, b| {
        let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        match (a_is_dir, b_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                let a_name = a.file_name().to_string_lossy().to_string();
                let b_name = b.file_name().to_string_lossy().to_string();
                a_name.cmp(&b_name)
            }
        }
    });

    let mut nodes = Vec::new();
    for entry in entries {
        let filename = entry.file_name().to_string_lossy().to_string();
        let entry_path = entry.path();
        let relative = entry_path.strip_prefix(base).unwrap_or(&entry_path).to_string_lossy().to_string();
        let relative = relative.replace('\\', "/");
        let is_dir = entry_path.is_dir();

        if !is_dir && !filename.ends_with(".md") && !inside_attachment {
            continue;
        }

        let is_attachment_dir = is_dir && filename == "attachment";
        let children = if is_dir {
            Some(build_tree(&entry_path, base, inside_attachment || is_attachment_dir)?)
        } else {
            None
        };

        nodes.push(FileTreeNode {
            name: filename,
            path: relative,
            is_dir,
            children,
        });
    }

    Ok(nodes)
}

pub async fn file_tree(
    req: web::Json<FileTreeRequest>,
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

    let notebook = match app_state.notebook_model.get_by_id_and_user(&req.notebook_id, &user_id) {
        Ok(Some(nb)) => nb,
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("NOTEBOOK_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/notebook/file_tree", &e),
    };

    let root_path_obj = Path::new(&root_path);
    let notebook_dir = root_path_obj.join(&notebook.path);

    if !notebook_dir.exists() {
        return HttpResponse::Ok().json(FileTreeResponse { success: true, tree: vec![] });
    }

    if !notebook.encrypted && app_state.search_manager.needs_rebuild(&req.notebook_id) {
        let search_mgr = Arc::clone(&app_state.search_manager);
        let nb_id = req.notebook_id.clone();
        let nb_root = notebook_dir.to_string_lossy().to_string();
        std::thread::spawn(move || {
            if let Err(e) = search_mgr.rebuild_notebook_index(&nb_id, &nb_root) {
                error_logger::log_error("search/rebuild_notebook_index", &e);
            }
        });
    }

    match build_tree(&notebook_dir, &notebook_dir, false) {
        Ok(tree) => HttpResponse::Ok().json(FileTreeResponse { success: true, tree }),
        Err(e) => internal_error_response("/api/notebook/file_tree", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct RenameRequest {
    pub notebook_id: String,
    pub old_path: String,
    pub new_name: String,
}

#[derive(Serialize)]
pub struct RenameResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_path: Option<String>,
}

pub async fn rename_note(
    req: web::Json<RenameRequest>,
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

    let notebook = match app_state.notebook_model.get_by_id_and_user(&req.notebook_id, &user_id) {
        Ok(Some(nb)) => nb,
        Ok(None) => {
            return HttpResponse::Ok().json(RenameResponse {
                success: false,
                fail_code: Some("NOTEBOOK_NOT_FOUND".to_string()),
                new_path: None,
            });
        }
        Err(e) => return internal_error_response("/api/notebook/rename", &e),
    };

    if !is_safe_path(&req.old_path) || !is_safe_name(&req.new_name) {
        return HttpResponse::Ok().json(RenameResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            new_path: None,
        });
    }

    if req.new_name == ".notebook.sig"
        || Path::new(&req.old_path).file_name().map(|n| n == ".notebook.sig").unwrap_or(false)
    {
        return HttpResponse::Ok().json(RenameResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            new_path: None,
        });
    }

    if req.new_name == "attachment" {
        return HttpResponse::Ok().json(RenameResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
            new_path: None,
        });
    }

    let root_path_obj = Path::new(&root_path);
    let notebook_dir = root_path_obj.join(&notebook.path);

    let old_full = notebook_dir.join(&req.old_path);
    let old_path_obj = Path::new(&req.old_path);
    let dir_part = old_path_obj.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();

    let new_full = if dir_part.is_empty() {
        notebook_dir.join(&req.new_name)
    } else {
        notebook_dir.join(&dir_part).join(&req.new_name)
    };

    if !is_path_under_root(&old_full, root_path_obj) || !is_path_under_root(&new_full, root_path_obj) {
        return HttpResponse::Ok().json(RenameResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            new_path: None,
        });
    }

    if !old_full.exists() {
        return HttpResponse::Ok().json(RenameResponse {
            success: false,
            fail_code: Some("FILE_NOT_FOUND".to_string()),
            new_path: None,
        });
    }

    if new_full.exists() {
        return HttpResponse::Ok().json(RenameResponse {
            success: false,
            fail_code: Some("FILE_ALREADY_EXISTS".to_string()),
            new_path: None,
        });
    }

    match std::fs::rename(&old_full, &new_full) {
        Ok(_) => {
            let new_relative = if dir_part.is_empty() {
                req.new_name.clone()
            } else {
                format!("{}/{}", dir_part, req.new_name)
            };

            if !notebook.encrypted {
                let search_mgr = Arc::clone(&app_state.search_manager);
                let nb_id = req.notebook_id.clone();
                let old_p = req.old_path.clone();
                let new_p = new_relative.clone();
                let nb_root = notebook_dir.to_string_lossy().to_string();
                std::thread::spawn(move || {
                    if let Err(e) = search_mgr.rename_note_index(&nb_id, &old_p, &new_p, &nb_root) {
                        error_logger::log_error("search/rename_note_index", &e);
                    }
                });
            }

            HttpResponse::Ok().json(RenameResponse {
                success: true,
                fail_code: None,
                new_path: Some(new_relative),
            })
        }
        Err(_) => HttpResponse::Ok().json(RenameResponse {
            success: false,
            fail_code: Some("FILE_WRITE_ERROR".to_string()),
            new_path: None,
        }),
    }
}

#[derive(Debug, Deserialize)]
pub struct MoveNoteRequest {
    pub notebook_id: String,
    pub source_path: String,
    pub target_folder: String,
}

#[derive(Serialize)]
pub struct MoveNoteResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_path: Option<String>,
}

pub async fn move_note(
    req: web::Json<MoveNoteRequest>,
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

    let notebook = match app_state.notebook_model.get_by_id_and_user(&req.notebook_id, &user_id) {
        Ok(Some(nb)) => nb,
        Ok(None) => {
            return HttpResponse::Ok().json(MoveNoteResponse {
                success: false,
                fail_code: Some("NOTEBOOK_NOT_FOUND".to_string()),
                new_path: None,
            });
        }
        Err(e) => return internal_error_response("/api/notebook/move", &e),
    };

    if !is_safe_path(&req.source_path) || !is_safe_path(&req.target_folder) {
        return HttpResponse::Ok().json(MoveNoteResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            new_path: None,
        });
    }

    if req.source_path.split('/').any(|s| s == "attachment")
        || req.target_folder.split('/').any(|s| s == "attachment")
        || req.target_folder == "attachment"
    {
        return HttpResponse::Ok().json(MoveNoteResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            new_path: None,
        });
    }

    if Path::new(&req.source_path).file_name().map(|n| n == ".notebook.sig").unwrap_or(false) {
        return HttpResponse::Ok().json(MoveNoteResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            new_path: None,
        });
    }

    let root_path_obj = Path::new(&root_path);
    let notebook_dir = root_path_obj.join(&notebook.path);

    let source_full = notebook_dir.join(&req.source_path);
    let target_dir = if req.target_folder.is_empty() {
        notebook_dir.clone()
    } else {
        notebook_dir.join(&req.target_folder)
    };
    let file_name = Path::new(&req.source_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let target_full = target_dir.join(&file_name);

    if !is_path_under_root(&source_full, root_path_obj) || !is_path_under_root(&target_full, root_path_obj) {
        return HttpResponse::Ok().json(MoveNoteResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            new_path: None,
        });
    }

    if !source_full.exists() {
        return HttpResponse::Ok().json(MoveNoteResponse {
            success: false,
            fail_code: Some("FILE_NOT_FOUND".to_string()),
            new_path: None,
        });
    }

    if !target_dir.exists() || !target_dir.is_dir() {
        return HttpResponse::Ok().json(MoveNoteResponse {
            success: false,
            fail_code: Some("FOLDER_NOT_FOUND".to_string()),
            new_path: None,
        });
    }

    let source_relative = req.source_path.clone();
    let new_relative = if req.target_folder.is_empty() {
        file_name.clone()
    } else {
        format!("{}/{}", req.target_folder, file_name)
    };

    if source_relative == new_relative {
        return HttpResponse::Ok().json(MoveNoteResponse {
            success: false,
            fail_code: Some("CANNOT_MOVE_TO_SELF".to_string()),
            new_path: None,
        });
    }

    if target_full.exists() {
        return HttpResponse::Ok().json(MoveNoteResponse {
            success: false,
            fail_code: Some("FILE_ALREADY_EXISTS".to_string()),
            new_path: None,
        });
    }

    if source_full.is_dir() && new_relative.starts_with(&format!("{}/", source_relative)) {
        return HttpResponse::Ok().json(MoveNoteResponse {
            success: false,
            fail_code: Some("CANNOT_MOVE_TO_SUBDIR".to_string()),
            new_path: None,
        });
    }

    match std::fs::rename(&source_full, &target_full) {
        Ok(_) => {
            if !notebook.encrypted {
                let search_mgr = Arc::clone(&app_state.search_manager);
                let nb_id = req.notebook_id.clone();
                let old_p = source_relative;
                let new_p = new_relative.clone();
                let nb_root = notebook_dir.to_string_lossy().to_string();
                std::thread::spawn(move || {
                    if let Err(e) = search_mgr.rename_note_index(&nb_id, &old_p, &new_p, &nb_root) {
                        error_logger::log_error("search/rename_note_index", &e);
                    }
                });
            }

            HttpResponse::Ok().json(MoveNoteResponse {
                success: true,
                fail_code: None,
                new_path: Some(new_relative),
            })
        }
        Err(_) => HttpResponse::Ok().json(MoveNoteResponse {
            success: false,
            fail_code: Some("FILE_WRITE_ERROR".to_string()),
            new_path: None,
        }),
    }
}

#[derive(Debug, Deserialize)]
pub struct AttachmentQuery {
    pub path: String,
    pub notebook_id: String,
    pub token: String,
}

pub async fn get_notebook_attachment(
    query: web::Query<AttachmentQuery>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let token_data = match validate_attachment_token(&query.token, &app_state.attachment_secret) {
        Some(d) => d,
        None => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("TOKEN_EXPIRED".to_string()),
            });
        }
    };

    if token_data.notebook_id != query.notebook_id {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
        });
    }

    let user_id = &token_data.user_id;

    let notebook = match app_state.notebook_model.get_by_id_and_user(&query.notebook_id, user_id) {
        Ok(Some(nb)) => nb,
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("NOTEBOOK_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/notebook/attachment", &e),
    };

    let root_path = match app_state.user_model.get_user_full(user_id) {
        Ok(Some(u)) => match u.root_path {
            Some(p) => p,
            None => {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("INVALID_PARAM".to_string()),
                });
            }
        },
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INVALID_PARAM".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/notebook/attachment", &e),
    };

    if !query.path.starts_with("attachment/") {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    if !is_safe_path(&query.path) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    let root_path_obj = Path::new(&root_path);
    let full_path = root_path_obj.join(&notebook.path).join(&query.path);

    if !is_path_under_root(&full_path, root_path_obj) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    if !full_path.exists() || !full_path.is_file() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("FILE_NOT_FOUND".to_string()),
        });
    }

    if !notebook.encrypted {
        let data = match std::fs::read(&full_path) {
            Ok(d) => d,
            Err(_) => {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("FILE_READ_ERROR".to_string()),
                });
            }
        };
        let mime_type = match full_path.extension().and_then(|e| e.to_str()).unwrap_or("") {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            "pdf" => "application/pdf",
            "mp4" => "video/mp4",
            "mp3" => "audio/mpeg",
            "txt" => "text/plain",
            "html" | "htm" => "text/html",
            "json" => "application/json",
            _ => "application/octet-stream",
        };
        return HttpResponse::Ok().content_type(mime_type).body(data);
    }

    let cache_key = format!("{}:{}", user_id, query.notebook_id);
    let key = match get_cached_key(&app_state, &cache_key) {
        Some(k) => k,
        None => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INVALID_PARAM".to_string()),
            });
        }
    };

    let file_data = match std::fs::read(&full_path) {
        Ok(d) => d,
        Err(_) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("FILE_READ_ERROR".to_string()),
            });
        }
    };

    if file_data.len() < 16 {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("FILE_READ_ERROR".to_string()),
        });
    }

    let iv = &file_data[..16];
    let ciphertext = &file_data[16..];

    let decrypted = match decrypt_aes_256_cbc(&key, iv, ciphertext) {
        Ok(d) => d,
        Err(_) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("FILE_READ_ERROR".to_string()),
            });
        }
    };

    HttpResponse::Ok().content_type("application/octet-stream").body(decrypted)
}

struct AttachmentTokenData {
    user_id: String,
    notebook_id: String,
}

fn validate_attachment_token(token: &str, secret: &str) -> Option<AttachmentTokenData> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 2 {
        return None;
    }

    let payload_bytes = base64::engine::general_purpose::STANDARD.decode(parts[0]).ok()?;
    let payload_str = String::from_utf8(payload_bytes).ok()?;
    let payload: serde_json::Value = serde_json::from_str(&payload_str).ok()?;

    let provided_sig = base64::engine::general_purpose::STANDARD.decode(parts[1]).ok()?;

    use hmac::{Hmac, Mac};
    type HmacSha256 = Hmac<sha2::Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).ok()?;
    mac.update(parts[0].as_bytes());
    let expected_sig = mac.finalize().into_bytes();

    if provided_sig.len() != expected_sig.len() {
        return None;
    }
    if !subtle_constant_time_eq(&provided_sig, &expected_sig) {
        return None;
    }

    let exp = payload.get("exp")?.as_i64()?;
    if chrono::Utc::now().timestamp() > exp {
        return None;
    }

    let user_id = payload.get("user_id")?.as_str()?.to_string();
    let notebook_id = payload.get("notebook_id")?.as_str()?.to_string();

    Some(AttachmentTokenData { user_id, notebook_id })
}

fn subtle_constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

fn get_cached_key(app_state: &web::Data<AppState>, cache_key: &str) -> Option<Vec<u8>> {
    let mut cache = app_state.notebook_key_cache.lock().unwrap_or_else(|e| e.into_inner());
    cache.retain(|_, (_, instant)| instant.elapsed().as_secs() <= 3600);
    let (key, _instant) = cache.get(cache_key)?;
    Some(key.clone())
}

fn decrypt_aes_256_cbc(key: &[u8], iv: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
    type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

    let mut buf = ciphertext.to_vec();
    let pt = Aes256CbcDec::new(key.into(), iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|e| format!("{:?}", e))?;

    Ok(pt.to_vec())
}

fn generate_attachment_token(user_id: &str, notebook_id: &str, secret: &str) -> String {
    let payload = serde_json::json!({
        "user_id": user_id,
        "notebook_id": notebook_id,
        "exp": chrono::Utc::now().timestamp() + 3600
    });
    let payload_str = serde_json::to_string(&payload).unwrap_or_default();
    let payload_b64 = base64::engine::general_purpose::STANDARD.encode(payload_str.as_bytes());

    use hmac::{Hmac, Mac};
    type HmacSha256 = Hmac<sha2::Sha256>;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload_b64.as_bytes());
    let sig = mac.finalize().into_bytes();
    let sig_b64 = base64::engine::general_purpose::STANDARD.encode(sig);

    format!("{}.{}", payload_b64, sig_b64)
}

#[derive(Debug, Deserialize)]
pub struct AttachmentTokenRequest {
    pub notebook_id: String,
    pub key: Option<String>,
}

#[derive(Serialize)]
pub struct AttachmentTokenResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<u64>,
}

pub async fn attachment_token(
    req: web::Json<AttachmentTokenRequest>,
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

    let notebook = match app_state.notebook_model.get_by_id_and_user(&req.notebook_id, &user_id) {
        Ok(Some(nb)) => nb,
        Ok(None) => {
            return HttpResponse::Ok().json(AttachmentTokenResponse {
                success: false,
                fail_code: Some("NOTEBOOK_NOT_FOUND".to_string()),
                token: None,
                expires_in: None,
            });
        }
        Err(e) => return internal_error_response("/api/notebook/attachment_token", &e),
    };

    if notebook.encrypted {
        let key_b64 = match &req.key {
            Some(k) => k,
            None => {
                return HttpResponse::Ok().json(AttachmentTokenResponse {
                    success: false,
                    fail_code: Some("INVALID_PARAM".to_string()),
                    token: None,
                    expires_in: None,
                });
            }
        };

        let root_path_obj = Path::new(&root_path);
        let sig_path = root_path_obj.join(&notebook.path).join(".notebook.sig");

        let sig_content = match std::fs::read_to_string(&sig_path) {
            Ok(c) => c,
            Err(_) => {
                return HttpResponse::Ok().json(AttachmentTokenResponse {
                    success: false,
                    fail_code: Some("INVALID_PARAM".to_string()),
                    token: None,
                    expires_in: None,
                });
            }
        };

        let sig_json: serde_json::Value = match serde_json::from_str(&sig_content) {
            Ok(v) => v,
            Err(_) => {
                return HttpResponse::Ok().json(AttachmentTokenResponse {
                    success: false,
                    fail_code: Some("INVALID_PARAM".to_string()),
                    token: None,
                    expires_in: None,
                });
            }
        };

        let iv_b64 = match sig_json.get("iv").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => {
                return HttpResponse::Ok().json(AttachmentTokenResponse {
                    success: false,
                    fail_code: Some("INVALID_PARAM".to_string()),
                    token: None,
                    expires_in: None,
                });
            }
        };

        let signature_b64 = match sig_json.get("signature").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => {
                return HttpResponse::Ok().json(AttachmentTokenResponse {
                    success: false,
                    fail_code: Some("INVALID_PARAM".to_string()),
                    token: None,
                    expires_in: None,
                });
            }
        };

        let key = match base64::engine::general_purpose::STANDARD.decode(key_b64) {
            Ok(k) => k,
            Err(_) => {
                return HttpResponse::Ok().json(AttachmentTokenResponse {
                    success: false,
                    fail_code: Some("INVALID_PARAM".to_string()),
                    token: None,
                    expires_in: None,
                });
            }
        };

        if key.len() != 32 {
            return HttpResponse::Ok().json(AttachmentTokenResponse {
                success: false,
                fail_code: Some("INVALID_PARAM".to_string()),
                token: None,
                expires_in: None,
            });
        }

        let iv = match base64::engine::general_purpose::STANDARD.decode(iv_b64) {
            Ok(v) => v,
            Err(_) => {
                return HttpResponse::Ok().json(AttachmentTokenResponse {
                    success: false,
                    fail_code: Some("INVALID_PARAM".to_string()),
                    token: None,
                    expires_in: None,
                });
            }
        };

        let signature = match base64::engine::general_purpose::STANDARD.decode(signature_b64) {
            Ok(s) => s,
            Err(_) => {
                return HttpResponse::Ok().json(AttachmentTokenResponse {
                    success: false,
                    fail_code: Some("INVALID_PARAM".to_string()),
                    token: None,
                    expires_in: None,
                });
            }
        };

        if iv.len() != 16 {
            return HttpResponse::Ok().json(AttachmentTokenResponse {
                success: false,
                fail_code: Some("INVALID_PARAM".to_string()),
                token: None,
                expires_in: None,
            });
        }

        let decrypted = match decrypt_aes_256_cbc(&key, &iv, &signature) {
            Ok(d) => d,
            Err(_) => {
                return HttpResponse::Ok().json(AttachmentTokenResponse {
                    success: false,
                    fail_code: Some("INVALID_PARAM".to_string()),
                    token: None,
                    expires_in: None,
                });
            }
        };

        let verify_str = String::from_utf8_lossy(&decrypted);
        if verify_str != "BROOKFILE_NOTEBOOK_VERIFY" {
            return HttpResponse::Ok().json(AttachmentTokenResponse {
                success: false,
                fail_code: Some("INVALID_PARAM".to_string()),
                token: None,
                expires_in: None,
            });
        }

        let cache_key = format!("{}:{}", user_id, req.notebook_id);
        {
            let mut cache = app_state.notebook_key_cache.lock().unwrap_or_else(|e| e.into_inner());
            cache.retain(|_, (_, instant)| instant.elapsed().as_secs() <= 3600);
            cache.insert(cache_key, (key, std::time::Instant::now()));
        }
    }

    let token = generate_attachment_token(&user_id, &req.notebook_id, &app_state.attachment_secret);

    HttpResponse::Ok().json(AttachmentTokenResponse {
        success: true,
        fail_code: None,
        token: Some(token),
        expires_in: Some(3600),
    })
}

pub async fn upload_notebook_attachment(
    mut payload: Multipart,
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

    let mut notebook_id: Option<String> = None;
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
            "notebook_id" => {
                let mut data = Vec::new();
                while let Some(chunk) = field.next().await {
                    if let Ok(c) = chunk {
                        data.extend_from_slice(&c);
                    }
                }
                notebook_id = Some(String::from_utf8_lossy(&data).to_string());
            }
            "path" => {
                let mut data = Vec::new();
                while let Some(chunk) = field.next().await {
                    if let Ok(c) = chunk {
                        data.extend_from_slice(&c);
                    }
                }
                file_path = Some(String::from_utf8_lossy(&data).to_string());
            }
            "file" => {
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
                file_data = Some(data.freeze().to_vec());
            }
            _ => {
                while let Some(chunk) = field.next().await {
                    let _ = chunk;
                }
            }
        }
    }

    let notebook_id = match notebook_id {
        Some(id) => id,
        None => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INVALID_PARAM".to_string()),
            });
        }
    };

    let file_path = match file_path {
        Some(p) => p,
        None => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INVALID_FILE_PATH".to_string()),
            });
        }
    };

    let file_data = match file_data {
        Some(d) => d,
        None => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INVALID_PARAM".to_string()),
            });
        }
    };

    let notebook = match app_state.notebook_model.get_by_id_and_user(&notebook_id, &user_id) {
        Ok(Some(nb)) => nb,
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("NOTEBOOK_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/notebook/upload_attachment", &e),
    };

    if !is_safe_name(&file_path) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    let root_path_obj = Path::new(&root_path);
    let attachment_dir = root_path_obj.join(&notebook.path).join("attachment");
    let target_path = attachment_dir.join(&file_path);

    if !attachment_dir.exists() {
        if let Err(_) = std::fs::create_dir_all(&attachment_dir) {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("FILE_WRITE_ERROR".to_string()),
            });
        }
    }

    if !is_path_under_root(&target_path, root_path_obj) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    if target_path.exists() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("FILE_ALREADY_EXISTS".to_string()),
        });
    }

    match std::fs::write(&target_path, &file_data) {
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
pub struct DeleteNotebookFolderRequest {
    pub notebook_id: String,
    pub path: String,
}

pub async fn delete_notebook_folder(
    req: web::Json<DeleteNotebookFolderRequest>,
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

    let notebook = match app_state.notebook_model.get_by_id_and_user(&req.notebook_id, &user_id) {
        Ok(Some(nb)) => nb,
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("NOTEBOOK_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/notebook/delete_folder", &e),
    };

    if !is_safe_path(&req.path) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    if req.path.split('/').any(|s| s == "attachment") || req.path == "attachment" {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    if req.path.split('/').any(|s| s == ".notebook.sig") || req.path == ".notebook.sig" {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    let root_path_obj = Path::new(&root_path);
    let recycle_bin_path = match get_recycle_bin_path(&user_id, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };
    let target_path = root_path_obj.join(&notebook.path).join(&req.path);

    if !is_path_under_root(&target_path, root_path_obj) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    if !target_path.exists() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PATH_NOT_FOUND".to_string()),
        });
    }

    if !target_path.is_dir() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("NOT_A_DIRECTORY".to_string()),
        });
    }

    if let Ok(entries) = std::fs::read_dir(&target_path) {
        if entries.count() > 0 {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("FOLDER_NOT_EMPTY".to_string()),
            });
        }
    }

    let full_relative = format!("{}/{}", notebook.path, req.path);
    if let Err(code) = delete_file_with_recycle(&target_path, &full_relative, &user_id, recycle_bin_path.as_deref(), &app_state) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some(code),
        });
    }

    HttpResponse::Ok().json(ApiResponse {
        success: true,
        fail_code: None,
    })
}

#[derive(Debug, Deserialize)]
pub struct BatchDeleteNotebookFilesRequest {
    pub notebook_id: String,
    pub paths: Vec<String>,
}

#[derive(Serialize)]
pub struct BatchDeleteNotebookFilesResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_paths: Option<Vec<String>>,
}

pub async fn batch_delete_notebook_files(
    body: web::Json<BatchDeleteNotebookFilesRequest>,
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

    let notebook = match app_state.notebook_model.get_by_id_and_user(&body.notebook_id, &user_id) {
        Ok(Some(nb)) => nb,
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("NOTEBOOK_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/notebook/batch_delete", &e),
    };

    let root_path_obj = Path::new(&root_path);
    let recycle_bin_path = match get_recycle_bin_path(&user_id, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };

    for rel_path in &body.paths {
        if !is_safe_path(rel_path) {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INVALID_FILE_PATH".to_string()),
            });
        }

        if rel_path == "attachment" {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INVALID_FILE_PATH".to_string()),
            });
        }

        if rel_path.split('/').any(|s| s == ".notebook.sig") || rel_path == ".notebook.sig" {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INVALID_FILE_PATH".to_string()),
            });
        }

        let target_path = root_path_obj.join(&notebook.path).join(rel_path);

        if !is_path_under_root(&target_path, root_path_obj) {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INVALID_FILE_PATH".to_string()),
            });
        }

        if target_path.is_dir() {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("IS_DIRECTORY".to_string()),
            });
        }
    }

    let mut failed_paths: Vec<String> = Vec::new();
    for rel_path in &body.paths {
        let target_path = root_path_obj.join(&notebook.path).join(rel_path);

        let full_relative = format!("{}/{}", notebook.path, rel_path);
        if let Err(_) = delete_file_with_recycle(&target_path, &full_relative, &user_id, recycle_bin_path.as_deref(), &app_state) {
            failed_paths.push(rel_path.clone());
        }
    }

    HttpResponse::Ok().json(BatchDeleteNotebookFilesResponse {
        success: true,
        fail_code: None,
        failed_paths: if failed_paths.is_empty() { None } else { Some(failed_paths) },
    })
}
