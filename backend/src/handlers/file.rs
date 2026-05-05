use actix_web::{web, HttpRequest, HttpResponse, Responder};
use actix_files::NamedFile;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use crate::app_state::AppState;
use crate::handlers::{ApiResponse, get_current_user_id, get_user_root_path, is_path_under_root, is_safe_name, is_safe_path, internal_error_response, move_recursive};

struct NotebookPathMatch {
    notebook_id: String,
    relative_path: String,
}

fn find_matching_notebook(file_path: &str, user_id: &str, app_state: &web::Data<AppState>) -> Option<NotebookPathMatch> {
    let notebooks = app_state.notebook_model.list_by_user(user_id).ok()?;
    let mut best: Option<(NotebookPathMatch, usize)> = None;
    for nb in notebooks {
        if nb.encrypted {
            continue;
        }
        let nb_path = nb.path.trim_end_matches('/');
        if file_path == nb_path {
            continue;
        }
        let prefix = format!("{}/", nb_path);
        if file_path.starts_with(&prefix) {
            let len = nb_path.len();
            if best.as_ref().map_or(true, |(_, l)| len > *l) {
                let relative = file_path[len + 1..].to_string();
                best = Some((NotebookPathMatch {
                    notebook_id: nb.id,
                    relative_path: relative,
                }, len));
            }
        }
    }
    best.map(|(m, _)| m)
}

fn collect_md_files_recursive(dir: &Path, base_relative: &str) -> Vec<(String, std::path::PathBuf)> {
    let mut results = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let dir_name = p.file_name().unwrap_or_default().to_string_lossy();
                let sub_relative = if base_relative.is_empty() {
                    dir_name.to_string()
                } else {
                    format!("{}/{}", base_relative, dir_name)
                };
                results.extend(collect_md_files_recursive(&p, &sub_relative));
            } else if p.extension().and_then(|e| e.to_str()) == Some("md") {
                let file_name = p.file_name().unwrap_or_default().to_string_lossy();
                let note_rel_path = if base_relative.is_empty() {
                    file_name.to_string()
                } else {
                    format!("{}/{}", base_relative, file_name)
                };
                results.push((note_rel_path, p));
            }
        }
    }
    results
}

pub fn cleanup_search_index_on_delete(file_path: &str, user_id: &str, app_state: &web::Data<AppState>) {
    if let Some(match_info) = find_matching_notebook(file_path, user_id, app_state) {
        if match_info.relative_path.is_empty() {
            return;
        }
        let _ = app_state.search_manager.remove_note_index_with_children(&match_info.notebook_id, &match_info.relative_path);
    }
}

pub fn cleanup_search_index_on_move(
    old_file_path: &str,
    new_file_path: &str,
    user_id: &str,
    app_state: &web::Data<AppState>,
) {
    let root_path = match get_user_root_from_state(app_state, user_id) {
        Some(p) => p,
        None => return,
    };
    let full_new = Path::new(&root_path).join(new_file_path);

    if let Some(old_match) = find_matching_notebook(old_file_path, user_id, app_state) {
        if !old_match.relative_path.is_empty() {
            let _ = app_state.search_manager.remove_note_index_with_children(&old_match.notebook_id, &old_match.relative_path);
        }
    }

    reindex_notebook_path(&full_new, new_file_path, user_id, app_state);
}

pub fn cleanup_search_index_on_restore(
    file_path: &str,
    user_id: &str,
    app_state: &web::Data<AppState>,
) {
    let root_path = match get_user_root_from_state(app_state, user_id) {
        Some(p) => p,
        None => return,
    };
    let full_path = Path::new(&root_path).join(file_path);
    reindex_notebook_path(&full_path, file_path, user_id, app_state);
}

fn reindex_notebook_path(
    full_path: &Path,
    relative_path: &str,
    user_id: &str,
    app_state: &web::Data<AppState>,
) {
    let root_path = match get_user_root_from_state(app_state, user_id) {
        Some(p) => p,
        None => return,
    };

    if let Some(match_info) = find_matching_notebook(relative_path, user_id, app_state) {
        if match_info.relative_path.is_empty() {
            return;
        }
        let notebooks = match app_state.notebook_model.list_by_user(user_id) {
            Ok(nbs) => nbs,
            Err(_) => return,
        };
        let nb_path = match notebooks.iter().find(|nb| nb.id == match_info.notebook_id) {
            Some(nb) => nb.path.clone(),
            None => return,
        };
        let notebook_root = Path::new(&root_path).join(&nb_path);
        let notebook_root_str = notebook_root.to_string_lossy().to_string();

        if full_path.is_dir() {
            for (new_rel, md_path) in collect_md_files_recursive(full_path, &match_info.relative_path) {
                if let Ok(content) = std::fs::read_to_string(&md_path) {
                    let _ = app_state.search_manager.index_note(
                        &match_info.notebook_id, &new_rel, &content, &notebook_root_str,
                    );
                }
            }
        } else if full_path.extension().and_then(|e| e.to_str()) == Some("md") {
            if let Ok(content) = std::fs::read_to_string(full_path) {
                let _ = app_state.search_manager.index_note(
                    &match_info.notebook_id, &match_info.relative_path, &content, &notebook_root_str,
                );
            }
        }
    }
}

fn get_user_root_from_state(app_state: &web::Data<AppState>, user_id: &str) -> Option<String> {
    app_state.user_model.get_user_full(user_id).ok().flatten().and_then(|u| u.root_path)
}

#[derive(Debug, Serialize)]
pub struct FileBrowseResponse {
    pub success: bool,
    pub files: Vec<FileInfo>,
}

#[derive(Debug, Serialize)]
pub struct FileInfo {
    pub name: String,
    pub file_type: String,
    pub size: u64,
    pub modified: String,
}

#[derive(Debug, Deserialize)]
pub struct BrowseRequest {
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DownloadRequest {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateFolderRequest {
    pub parent_path: Option<String>,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteRequest {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct MoveRequest {
    pub files: Vec<String>,
    pub current_path: Option<String>,
    pub target_path: String,
}

#[derive(Debug, Deserialize)]
pub struct BatchDeleteRequest {
    pub files: Vec<String>,
    pub current_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BatchDeleteData {
    pub failed_files: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct BatchDeleteResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<BatchDeleteData>,
}

#[derive(Debug, Serialize)]
pub struct MoveResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict_files: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_files: Option<Vec<String>>,
}

pub async fn browse_files(
    body: web::Json<BrowseRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };
    let root_path_obj = Path::new(&root_path);

    let relative_path = body.path.as_deref().unwrap_or("");

    if !relative_path.is_empty() && !is_safe_path(relative_path) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    let target_path = if relative_path.is_empty() {
        root_path_obj.to_path_buf()
    } else {
        root_path_obj.join(relative_path)
    };

    if !target_path.exists() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PATH_NOT_FOUND".to_string()),
        });
    }

    if !is_path_under_root(&target_path, root_path_obj) {
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

    let mut files = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&target_path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let file_type = if metadata.is_dir() {
                "directory".to_string()
            } else if metadata.is_file() {
                "file".to_string()
            } else {
                "other".to_string()
            };

            let modified = metadata
                .modified()
                .ok()
                .and_then(|t| {
                    let duration = t.duration_since(std::time::UNIX_EPOCH).ok()?;
                    Some(duration.as_secs())
                })
                .unwrap_or(0);

            let name = entry_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let size = if metadata.is_dir() { 0 } else { metadata.len() };

            files.push(FileInfo {
                name,
                file_type,
                size,
                modified: modified.to_string(),
            });
        }
    }

    files.sort_by(|a, b| {
        match (a.file_type.as_str(), b.file_type.as_str()) {
            ("directory", "directory") | ("file", "file") => a.name.cmp(&b.name),
            ("directory", _) => std::cmp::Ordering::Less,
            (_, "directory") => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        }
    });

    HttpResponse::Ok().json(FileBrowseResponse { success: true, files })
}

pub async fn download_file(
    body: web::Json<DownloadRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };
    let root_path_obj = Path::new(&root_path);

    if body.path.is_empty() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    if !is_safe_path(&body.path) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    let target_path = root_path_obj.join(&body.path);

    if !target_path.exists() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PATH_NOT_FOUND".to_string()),
        });
    }

    if !is_path_under_root(&target_path, root_path_obj) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PATH_NOT_FOUND".to_string()),
        });
    }

    if !target_path.is_file() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("NOT_A_FILE".to_string()),
        });
    }

    match NamedFile::open(&target_path) {
        Ok(file) => file.into_response(&http_req),
        Err(_) => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("FILE_READ_ERROR".to_string()),
        }),
    }
}

pub async fn create_folder(
    body: web::Json<CreateFolderRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };
    let root_path_obj = Path::new(&root_path);

    if !is_safe_name(&body.name) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FOLDER_NAME".to_string()),
        });
    }

    let parent_path = body.parent_path.as_deref().unwrap_or("");
    if !parent_path.is_empty() && !is_safe_path(parent_path) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }
    let parent_full_path = if parent_path.is_empty() {
        root_path_obj.to_path_buf()
    } else {
        root_path_obj.join(parent_path)
    };

    if !parent_full_path.exists() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PATH_NOT_FOUND".to_string()),
        });
    }

    if !parent_full_path.is_dir() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("NOT_A_DIRECTORY".to_string()),
        });
    }

    if !is_path_under_root(&parent_full_path, root_path_obj) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PATH_NOT_FOUND".to_string()),
        });
    }

    let new_folder_path = parent_full_path.join(&body.name);

    if new_folder_path.exists() {
        if new_folder_path.is_dir() {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("FOLDER_ALREADY_EXISTS".to_string()),
            });
        } else {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("FILE_ALREADY_EXISTS".to_string()),
            });
        }
    }

    match std::fs::create_dir(&new_folder_path) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        }),
        Err(_) => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("CREATE_FOLDER_FAILED".to_string()),
        }),
    }
}

pub async fn delete_file(
    body: web::Json<DeleteRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };
    let root_path_obj = Path::new(&root_path);

    if body.path.is_empty() || body.path == "." {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    if !is_safe_path(&body.path) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    let target_path = root_path_obj.join(&body.path);

    if !target_path.exists() {
        return HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        });
    }

    if !is_path_under_root(&target_path, root_path_obj) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }

    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let recycle_bin_path = match get_recycle_bin_path(&user_id, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };

    match delete_file_with_recycle(&target_path, &body.path, &user_id, recycle_bin_path.as_deref(), &app_state) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        }),
        Err(code) => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some(code),
        }),
    }
}

pub async fn move_files(
    body: web::Json<MoveRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };
    let root_path_obj = Path::new(&root_path);

    if body.files.is_empty() {
        return HttpResponse::Ok().json(MoveResponse {
            success: false,
            fail_code: Some("NO_FILES_SPECIFIED".to_string()),
            conflict_files: None,
            failed_files: None,
        });
    }

    {
        let mut seen = std::collections::HashSet::new();
        for file_name in &body.files {
            if !seen.insert(file_name.clone()) {
                return HttpResponse::Ok().json(MoveResponse {
                    success: false,
                    fail_code: Some("INVALID_PARAM".to_string()),
                    conflict_files: None,
                    failed_files: None,
                });
            }
        }
    }

    let current_dir = body.current_path.as_deref().unwrap_or("");
    if !current_dir.is_empty() && !is_safe_path(current_dir) {
        return HttpResponse::Ok().json(MoveResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            conflict_files: None,
            failed_files: None,
        });
    }
    let current_full_path = if current_dir.is_empty() {
        root_path_obj.to_path_buf()
    } else {
        root_path_obj.join(current_dir)
    };

    if !current_full_path.exists() || !current_full_path.is_dir() {
        return HttpResponse::Ok().json(MoveResponse {
            success: false,
            fail_code: Some("PATH_NOT_FOUND".to_string()),
            conflict_files: None,
            failed_files: None,
        });
    }

    if !is_path_under_root(&current_full_path, root_path_obj) {
        return HttpResponse::Ok().json(MoveResponse {
            success: false,
            fail_code: Some("PATH_NOT_FOUND".to_string()),
            conflict_files: None,
            failed_files: None,
        });
    }

    if !body.target_path.is_empty() && !is_safe_path(&body.target_path) {
        return HttpResponse::Ok().json(MoveResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
            conflict_files: None,
            failed_files: None,
        });
    }

    let target_full_path = if body.target_path.is_empty() {
        root_path_obj.to_path_buf()
    } else {
        root_path_obj.join(&body.target_path)
    };

    if !target_full_path.exists() || !target_full_path.is_dir() {
        return HttpResponse::Ok().json(MoveResponse {
            success: false,
            fail_code: Some("TARGET_PATH_NOT_FOUND".to_string()),
            conflict_files: None,
            failed_files: None,
        });
    }

    if !is_path_under_root(&target_full_path, root_path_obj) {
        return HttpResponse::Ok().json(MoveResponse {
            success: false,
            fail_code: Some("TARGET_PATH_NOT_FOUND".to_string()),
            conflict_files: None,
            failed_files: None,
        });
    }

    let target_relative = body.target_path.trim_end_matches('/');
    for file_name in &body.files {
        let source_relative = if current_dir.is_empty() {
            file_name.clone()
        } else {
            format!("{}/{}", current_dir, file_name)
        };
        let source_full = root_path_obj.join(&source_relative);
        if source_full.is_dir() {
            let source_relative_prefix = format!("{}/", source_relative);
            if target_relative == source_relative || target_relative.starts_with(&source_relative_prefix) {
                return HttpResponse::Ok().json(MoveResponse {
                    success: false,
                    fail_code: Some("CANNOT_MOVE_INTO_SUBDIR".to_string()),
                    conflict_files: None,
                    failed_files: None,
                });
            }
        }
    }

    let mut conflict_files = Vec::new();
    let mut move_items: Vec<(PathBuf, PathBuf, String)> = Vec::new();

    for file_name in &body.files {
        if !is_safe_name(file_name) {
            return HttpResponse::Ok().json(MoveResponse {
                success: false,
                fail_code: Some("INVALID_FILE_NAME".to_string()),
                conflict_files: None,
                failed_files: None,
            });
        }

        let source_path = current_full_path.join(file_name);

        if !source_path.exists() {
            continue;
        }

        if !is_path_under_root(&source_path, root_path_obj) {
            continue;
        }

        let dest_path = target_full_path.join(file_name);
        if !is_path_under_root(&dest_path, root_path_obj) {
            continue;
        }

        if dest_path.exists() {
            conflict_files.push(file_name.clone());
        } else {
            move_items.push((source_path, dest_path, file_name.clone()));
        }
    }

    if !conflict_files.is_empty() {
        return HttpResponse::Ok().json(MoveResponse {
            success: false,
            fail_code: Some("FILES_ALREADY_EXIST".to_string()),
            conflict_files: Some(conflict_files),
            failed_files: None,
        });
    }

    let mut failed_files: Vec<String> = Vec::new();
    let mut moved_items: Vec<(String, String)> = Vec::new();
    for (source, dest, name) in &move_items {
        if source.is_dir() {
            if std::fs::rename(source, dest).is_err() {
                if let Err(_) = std::fs::create_dir_all(dest) {
                    failed_files.push(name.clone());
                } else {
                    let mut ok = true;
                    if let Err(_) = copy_dir_recursive(source, dest) {
                        ok = false;
                    }
                    if !ok {
                        let _ = std::fs::remove_dir_all(dest);
                        failed_files.push(name.clone());
                    } else if std::fs::remove_dir_all(source).is_err() {
                        let _ = std::fs::remove_dir_all(dest);
                        failed_files.push(name.clone());
                    } else {
                        let current_dir = body.current_path.as_deref().unwrap_or("");
                        let old_relative = if current_dir.is_empty() {
                            name.clone()
                        } else {
                            format!("{}/{}", current_dir, name)
                        };
                        let new_relative = if body.target_path.is_empty() {
                            name.clone()
                        } else {
                            format!("{}/{}", body.target_path, name)
                        };
                        moved_items.push((old_relative, new_relative));
                    }
                }
            } else {
                let current_dir = body.current_path.as_deref().unwrap_or("");
                let old_relative = if current_dir.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", current_dir, name)
                };
                let new_relative = if body.target_path.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", body.target_path, name)
                };
                moved_items.push((old_relative, new_relative));
            }
        } else {
            if std::fs::rename(source, dest).is_err() {
                if std::fs::copy(source, dest).is_err() {
                    failed_files.push(name.clone());
                } else {
                    if std::fs::remove_file(source).is_err() {
                        let _ = std::fs::remove_file(dest);
                        failed_files.push(name.clone());
                    } else {
                        let current_dir = body.current_path.as_deref().unwrap_or("");
                        let old_relative = if current_dir.is_empty() {
                            name.clone()
                        } else {
                            format!("{}/{}", current_dir, name)
                        };
                        let new_relative = if body.target_path.is_empty() {
                            name.clone()
                        } else {
                            format!("{}/{}", body.target_path, name)
                        };
                        moved_items.push((old_relative, new_relative));
                    }
                }
            } else {
                let current_dir = body.current_path.as_deref().unwrap_or("");
                let old_relative = if current_dir.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", current_dir, name)
                };
                let new_relative = if body.target_path.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", body.target_path, name)
                };
                moved_items.push((old_relative, new_relative));
            }
        }
    }

    if !moved_items.is_empty() {
        if let Ok(user_id) = get_current_user_id(&http_req, &app_state) {
            for (old_relative, new_relative) in &moved_items {
                cleanup_search_index_on_move(old_relative, new_relative, &user_id, &app_state);
            }
        }
    }

    if !failed_files.is_empty() {
        return HttpResponse::Ok().json(MoveResponse {
            success: false,
            fail_code: Some("MOVE_FAILED".to_string()),
            conflict_files: None,
            failed_files: Some(failed_files),
        });
    }

    HttpResponse::Ok().json(MoveResponse {
        success: true,
        fail_code: None,
        conflict_files: None,
        failed_files: None,
    })
}

pub async fn batch_delete(
    body: web::Json<BatchDeleteRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };
    let root_path_obj = Path::new(&root_path);

    if body.files.is_empty() {
        return HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        });
    }

    let current_dir = body.current_path.as_deref().unwrap_or("");
    if !current_dir.is_empty() && !is_safe_path(current_dir) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_FILE_PATH".to_string()),
        });
    }
    let current_full_path = if current_dir.is_empty() {
        root_path_obj.to_path_buf()
    } else {
        root_path_obj.join(current_dir)
    };

    if !current_full_path.exists() || !current_full_path.is_dir() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PATH_NOT_FOUND".to_string()),
        });
    }

    if !is_path_under_root(&current_full_path, root_path_obj) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PATH_NOT_FOUND".to_string()),
        });
    }

    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let recycle_bin_path = match get_recycle_bin_path(&user_id, &app_state) {
        Ok(path) => path,
        Err(resp) => return resp,
    };

    let mut failed_files: Vec<String> = Vec::new();
    for file_name in &body.files {
        if !is_safe_name(file_name) {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INVALID_FILE_PATH".to_string()),
            });
        }

        let target_path = current_full_path.join(file_name);

        if !target_path.exists() {
            continue;
        }

        if !is_path_under_root(&target_path, root_path_obj) {
            continue;
        }

        let relative_path = if current_dir.is_empty() {
            file_name.clone()
        } else {
            format!("{}/{}", current_dir, file_name)
        };

        if let Err(_) = delete_file_with_recycle(&target_path, &relative_path, &user_id, recycle_bin_path.as_deref(), &app_state) {
            failed_files.push(file_name.clone());
        }
    }

    if !failed_files.is_empty() {
        return HttpResponse::Ok().json(BatchDeleteResponse {
            success: false,
            fail_code: Some("DELETE_FAILED".to_string()),
            data: Some(BatchDeleteData { failed_files }),
        });
    }

    HttpResponse::Ok().json(BatchDeleteResponse {
        success: true,
        fail_code: None,
        data: None,
    })
}

pub fn get_recycle_bin_path(user_id: &str, app_state: &web::Data<AppState>) -> Result<Option<String>, HttpResponse> {
    app_state.user_model.get_recycle_bin_path(user_id).map_err(|e| internal_error_response("get_recycle_bin_path", &e))
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

pub fn calc_dir_size(dir_path: &Path) -> i64 {
    let mut total: i64 = 0;
    if let Ok(entries) = std::fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                total += calc_dir_size(&path);
            } else if let Ok(metadata) = path.metadata() {
                total += metadata.len() as i64;
            }
        }
    }
    total
}

pub fn delete_file_with_recycle(
    target_path: &Path,
    relative_path: &str,
    user_id: &str,
    recycle_bin_path: Option<&str>,
    app_state: &web::Data<AppState>,
) -> Result<(), String> {
    if !target_path.exists() {
        return Ok(());
    }

    if let Some(rb_path) = recycle_bin_path {
        let is_directory = target_path.is_dir();
        let original_name = target_path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let file_size = if is_directory {
            calc_dir_size(target_path)
        } else {
            target_path.metadata().map(|m| m.len() as i64).unwrap_or(0)
        };

        let record_id = Uuid::new_v4().to_string();
        let dest_dir = Path::new(rb_path).join(&record_id);

        std::fs::create_dir_all(&dest_dir).map_err(|_| "RECYCLE_MOVE_FAILED".to_string())?;

        if let Err(_) = app_state.recycle_bin_model.insert(&record_id, user_id, relative_path, &original_name, is_directory, file_size) {
            let _ = std::fs::remove_dir_all(&dest_dir);
            return Err("RECYCLE_MOVE_FAILED".to_string());
        }

        let dest_path = dest_dir.join(&original_name);
        if let Err(_) = move_recursive(target_path, &dest_path) {
            let _ = std::fs::remove_dir_all(&dest_dir);
            let _ = app_state.recycle_bin_model.delete_by_id(&record_id, user_id);
            return Err("RECYCLE_MOVE_FAILED".to_string());
        }

        cleanup_search_index_on_delete(relative_path, user_id, app_state);
        return Ok(());
    }

    let delete_result = if target_path.is_dir() {
        std::fs::remove_dir_all(target_path)
    } else {
        std::fs::remove_file(target_path)
    };

    delete_result.map_err(|_| "DELETE_FAILED".to_string())?;
    cleanup_search_index_on_delete(relative_path, user_id, app_state);
    Ok(())
}
