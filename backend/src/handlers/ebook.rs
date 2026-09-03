use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use sha2::{Sha256, Digest};
use std::io::BufReader;
use std::io::Read;
use crate::app_state::AppState;
use crate::handlers::{ApiResponse, internal_error_response, get_current_user_id, get_user_root_path, is_safe_path, is_path_under_root};
use crate::ebook::EbookDb;

pub async fn test_pdf(http_req: HttpRequest) -> impl Responder {
    use actix_files::NamedFile;
    use std::env;

    const TEST_PDF_NAME: &str = "机器学习_11020203.pdf";
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut dir = cwd.as_path();
    for _ in 0..6 {
        let candidate = dir.join(TEST_PDF_NAME);
        if candidate.is_file() {
            return match NamedFile::open(&candidate) {
                Ok(file) => file.use_last_modified(true).into_response(&http_req),
                Err(_) => HttpResponse::NotFound().json(ApiResponse {
                    success: false,
                    fail_code: Some("FILE_READ_ERROR".to_string()),
                }),
            };
        }
        dir = match dir.parent() {
            Some(p) => p,
            None => break,
        };
    }
    HttpResponse::NotFound().json(ApiResponse {
        success: false,
        fail_code: Some("PATH_NOT_FOUND".to_string()),
    })
}

fn compute_file_hash_and_size(file_path: &Path) -> Result<(String, i64), String> {
    let file = std::fs::File::open(file_path).map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    let file_size = metadata.len() as i64;

    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    let hash = hasher.finalize();
    Ok((hex::encode(hash), file_size))
}

fn get_ebook_path(app_state: &web::Data<AppState>, user_id: &str) -> Result<String, HttpResponse> {
    let user = match app_state.user_model.get_user_full(user_id) {
        Ok(Some(u)) => u,
        Ok(None) => {
            return Err(HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("NOT_LOGGED_IN".to_string()),
            }));
        }
        Err(e) => {
            return Err(internal_error_response("/api/ebook", &e));
        }
    };
    match user.ebook_path {
        Some(ep) if !ep.is_empty() => Ok(ep),
        _ => Err(HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("EBOOK_NOT_CONFIGURED".to_string()),
        })),
    }
}

fn detect_format(filename: &str) -> Option<&'static str> {
    let lower = filename.to_lowercase();
    if lower.ends_with(".txt") {
        Some("txt")
    } else if lower.ends_with(".epub") {
        Some("epub")
    } else if lower.ends_with(".pdf") {
        Some("pdf")
    } else {
        None
    }
}

#[derive(Serialize)]
pub struct EbookConfigResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    pub ebook_path: Option<String>,
    pub ebook_enabled: bool,
    pub ebook_db_status: String,
}

pub async fn ebook_get_config(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };

    let user = match app_state.user_model.get_user_full(&user_id) {
        Ok(Some(u)) => u,
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("NOT_LOGGED_IN".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/ebook/config/get", &e),
    };

    let ebook_path = user.ebook_path.clone();
    let (ebook_enabled, ebook_db_status) = match (&root_path, &ebook_path) {
        (rp, Some(ep)) => {
            let status = crate::ebook::check_ebook_metadata_db(Path::new(rp), ep);
            match status {
                Ok(crate::ebook::EbookDbStatus::Ok) => (true, "ok".to_string()),
                Ok(crate::ebook::EbookDbStatus::Missing) => (false, "missing".to_string()),
                Ok(_) => (false, "corrupt".to_string()),
                Err(_) => (false, "corrupt".to_string()),
            }
        }
        _ => (false, "missing".to_string()),
    };

    HttpResponse::Ok().json(EbookConfigResponse {
        success: true,
        fail_code: None,
        ebook_path,
        ebook_enabled,
        ebook_db_status,
    })
}

#[derive(Deserialize)]
pub struct EbookSetConfigRequest {
    pub ebook_path: String,
}

pub async fn ebook_set_config(
    req: web::Json<EbookSetConfigRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };

    let root_path_obj = Path::new(&root_path);
    let trimmed = req.ebook_path.trim().trim_start_matches('/').to_string();
    let ebook_path: Option<String> = if trimmed.is_empty() {
        None
    } else {
        if !is_safe_path(&trimmed) {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("PATH_INVALID".to_string()),
            });
        }
        let target = root_path_obj.join(trimmed.as_str());
        if !is_path_under_root(&target, root_path_obj) || !target.exists() || !target.is_dir() {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("PATH_INVALID".to_string()),
            });
        }
        match crate::ebook::check_ebook_metadata_db(root_path_obj, &trimmed) {
            Ok(crate::ebook::EbookDbStatus::Ok) => {}
            Ok(crate::ebook::EbookDbStatus::Missing) => {
                if let Err(_) = crate::ebook::init_ebook_metadata_db(root_path_obj, &trimmed) {
                    return HttpResponse::Ok().json(ApiResponse {
                        success: false,
                        fail_code: Some("EBOOK_DB_INIT_FAILED".to_string()),
                    });
                }
            }
            Ok(_) => {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("EBOOK_DB_INVALID".to_string()),
                });
            }
            Err(e) => return internal_error_response("/api/ebook/config/set", &e),
        }
        Some(trimmed.clone())
    };

    match app_state.user_model.update_ebook_path(&user_id, ebook_path.as_deref()) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse { success: true, fail_code: None }),
        Err(e) => internal_error_response("/api/ebook/config/set", &e),
    }
}

#[derive(Serialize)]
pub struct ShelfBookItem {
    pub id: String,
    pub title: String,
    pub format: String,
    pub file_size: i64,
    pub sha256: String,
    pub category_id: Option<String>,
    pub added_at: String,
    pub scan_status: String,
    pub has_preview: bool,
}

#[derive(Serialize)]
pub struct ShelfCategoryItem {
    pub id: String,
    pub name: String,
    pub sort_order: i64,
}

#[derive(Serialize)]
pub struct ShelfListResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    pub books: Vec<ShelfBookItem>,
    pub categories: Vec<ShelfCategoryItem>,
}

pub async fn ebook_shelf_list(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };

    let ebook_path = match get_ebook_path(&app_state, &user_id) {
        Ok(ep) => ep,
        Err(resp) => return resp,
    };

    let db = match EbookDb::open(Path::new(&root_path), &ebook_path) {
        Ok(db) => db,
        Err(e) => return internal_error_response("/api/ebook/shelf/list", &e),
    };

    let books = match db.list_books() {
        Ok(b) => b,
        Err(e) => return internal_error_response("/api/ebook/shelf/list", &e),
    };

    let categories = match db.list_categories() {
        Ok(c) => c,
        Err(e) => return internal_error_response("/api/ebook/shelf/list", &e),
    };

    let book_items: Vec<ShelfBookItem> = books.iter().map(|b| ShelfBookItem {
        id: b.id.clone(),
        title: b.title.clone(),
        format: b.format.clone(),
        file_size: b.file_size,
        sha256: b.sha256.clone(),
        category_id: b.category_id.clone(),
        added_at: b.added_at.clone(),
        scan_status: b.scan_status.clone(),
        has_preview: b.preview_filename.is_some(),
    }).collect();

    let cat_items: Vec<ShelfCategoryItem> = categories.iter().map(|c| ShelfCategoryItem {
        id: c.id.clone(),
        name: c.name.clone(),
        sort_order: c.sort_order,
    }).collect();

    HttpResponse::Ok().json(ShelfListResponse {
        success: true,
        fail_code: None,
        books: book_items,
        categories: cat_items,
    })
}

#[derive(Deserialize)]
pub struct ShelfAddRequest {
    pub items: Vec<ShelfAddItem>,
}

#[derive(Deserialize)]
pub struct ShelfAddItem {
    pub path: String,
}

#[derive(Serialize)]
pub struct ShelfAddResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    pub added: Vec<ShelfAddedBook>,
    pub duplicates: Vec<ShelfDuplicateBook>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
}

#[derive(Serialize)]
pub struct ShelfAddedBook {
    pub id: String,
    pub title: String,
    pub format: String,
}

#[derive(Serialize)]
pub struct ShelfDuplicateBook {
    pub path: String,
    pub title: String,
}

pub async fn ebook_shelf_add(
    req: web::Json<ShelfAddRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };

    let ebook_path = match get_ebook_path(&app_state, &user_id) {
        Ok(ep) => ep,
        Err(resp) => return resp,
    };

    let root_path_obj = Path::new(&root_path);
    let db = match EbookDb::open(root_path_obj, &ebook_path) {
        Ok(db) => db,
        Err(e) => return internal_error_response("/api/ebook/shelf/add", &e),
    };

    let mut added = Vec::new();
    let mut duplicates = Vec::new();
    let mut scan_books: Vec<(String, String, String)> = Vec::new();

    for item in &req.items {
        let trimmed = item.path.trim().trim_start_matches('/').to_string();
        if !is_safe_path(&trimmed) {
            continue;
        }
        let abs_path = root_path_obj.join(&trimmed);
        if !is_path_under_root(&abs_path, root_path_obj) || !abs_path.exists() || !abs_path.is_file() {
            continue;
        }

        let filename = abs_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let format = match detect_format(filename) {
            Some(f) => f,
            None => continue,
        };

        let (sha256, file_size) = match compute_file_hash_and_size(&abs_path) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Ok(Some(existing)) = db.find_duplicate(&trimmed, file_size, &sha256) {
            duplicates.push(ShelfDuplicateBook {
                path: trimmed.clone(),
                title: existing.title,
            });
            continue;
        }

        let title = Path::new(&filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();

        match db.add_book(&trimmed, format, &title, file_size, &sha256) {
            Ok(book_id) => {
                added.push(ShelfAddedBook {
                    id: book_id.clone(),
                    title: title.clone(),
                    format: format.to_string(),
                });
                scan_books.push((book_id, abs_path.to_string_lossy().to_string(), format.to_string()));
            }
            Err(_) => continue,
        }
    }

    let batch_id = if !scan_books.is_empty() {
        let batch_id = uuid::Uuid::new_v4().to_string();
        let data_dir = crate::ebook::ebook_meta_dir(root_path_obj, &ebook_path);
        app_state.ebook_scan_manager.start_metadata_scan(
            &batch_id,
            scan_books,
            root_path.clone(),
            ebook_path.clone(),
            data_dir,
        ).await;
        Some(batch_id)
    } else {
        None
    };

    HttpResponse::Ok().json(ShelfAddResponse {
        success: true,
        fail_code: None,
        added,
        duplicates,
        batch_id,
    })
}

#[derive(Deserialize)]
pub struct ShelfRemoveRequest {
    pub book_id: String,
}

pub async fn ebook_shelf_remove(
    req: web::Json<ShelfRemoveRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };

    let ebook_path = match get_ebook_path(&app_state, &user_id) {
        Ok(ep) => ep,
        Err(resp) => return resp,
    };

    let root_path_obj = Path::new(&root_path);
    let db = match EbookDb::open(root_path_obj, &ebook_path) {
        Ok(db) => db,
        Err(e) => return internal_error_response("/api/ebook/shelf/remove", &e),
    };

    if let Ok(Some(book)) = db.get_book(&req.book_id) {
        let data_dir = crate::ebook::ebook_meta_dir(root_path_obj, &ebook_path);
        if let Some(ref preview) = book.preview_filename {
            let _ = std::fs::remove_file(data_dir.join("previews").join(preview));
        }
        if let Some(ref cache) = book.txt_cache_filename {
            let _ = std::fs::remove_file(data_dir.join("cache").join(cache));
        }
    }

    match db.remove_book(&req.book_id) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse { success: true, fail_code: None }),
        Err(e) => internal_error_response("/api/ebook/shelf/remove", &e),
    }
}

#[derive(Deserialize)]
pub struct ShelfRelinkRequest {
    pub book_id: String,
    pub new_path: Option<String>,
}

pub async fn ebook_shelf_relink(
    req: web::Json<ShelfRelinkRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };

    let ebook_path = match get_ebook_path(&app_state, &user_id) {
        Ok(ep) => ep,
        Err(resp) => return resp,
    };

    let root_path_obj = Path::new(&root_path);
    let db = match EbookDb::open(root_path_obj, &ebook_path) {
        Ok(db) => db,
        Err(e) => return internal_error_response("/api/ebook/shelf/relink", &e),
    };

    let book = match db.get_book(&req.book_id) {
        Ok(Some(b)) => b,
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("BOOK_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/ebook/shelf/relink", &e),
    };

    let new_rel_path = match &req.new_path {
        Some(np) => {
            let trimmed = np.trim().trim_start_matches('/').to_string();
            if !is_safe_path(&trimmed) {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("PATH_INVALID".to_string()),
                });
            }
            let abs = root_path_obj.join(&trimmed);
            if !is_path_under_root(&abs, root_path_obj) || !abs.exists() || !abs.is_file() {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("FILE_NOT_FOUND".to_string()),
                });
            }
            trimmed
        }
        None => book.source_path.clone(),
    };

    let abs_path = root_path_obj.join(&new_rel_path);
    let (_sha256, _file_size) = match compute_file_hash_and_size(&abs_path) {
        Ok(v) => v,
        Err(e) => return internal_error_response("/api/ebook/shelf/relink", &e),
    };

    if let Err(e) = db.update_book_path(&req.book_id, &new_rel_path) {
        return internal_error_response("/api/ebook/shelf/relink", &e);
    }

    let format = book.format.clone();
    let batch_id = uuid::Uuid::new_v4().to_string();
    let data_dir = crate::ebook::ebook_meta_dir(root_path_obj, &ebook_path);
    app_state.ebook_scan_manager.start_metadata_scan(
        &batch_id,
        vec![(req.book_id.clone(), abs_path.to_string_lossy().to_string(), format)],
        root_path.clone(),
        ebook_path.clone(),
        data_dir,
    ).await;

    #[derive(Serialize)]
    struct RelinkResponse {
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        fail_code: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        batch_id: Option<String>,
    }

    HttpResponse::Ok().json(RelinkResponse {
        success: true,
        fail_code: None,
        batch_id: Some(batch_id),
    })
}

#[derive(Deserialize)]
pub struct ScanDiscoverRequest {
    pub path: String,
}

#[derive(Serialize)]
pub struct ScanDiscoverItem {
    pub path: String,
    pub format: String,
    pub title: String,
    pub file_size: i64,
}

#[derive(Serialize)]
pub struct ScanDiscoverResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    pub files: Vec<ScanDiscoverItem>,
}

pub async fn ebook_scan_discover(
    req: web::Json<ScanDiscoverRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };

    let _ebook_path = match get_ebook_path(&app_state, &user_id) {
        Ok(ep) => ep,
        Err(resp) => return resp,
    };

    let root_path_obj = Path::new(&root_path);
    let trimmed = req.path.trim().trim_start_matches('/').to_string();
    if !is_safe_path(&trimmed) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PATH_INVALID".to_string()),
        });
    }

    let scan_dir = root_path_obj.join(&trimmed);
    if !is_path_under_root(&scan_dir, root_path_obj) || !scan_dir.exists() || !scan_dir.is_dir() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PATH_INVALID".to_string()),
        });
    }

    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(&scan_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if let Some(format) = detect_format(filename) {
                    let rel_path = path.strip_prefix(root_path_obj)
                        .ok()
                        .and_then(|p| p.to_str())
                        .unwrap_or("");
                    let file_size = path.metadata().map(|m| m.len() as i64).unwrap_or(0);
                    let title = Path::new(filename)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Unknown")
                        .to_string();
                    files.push(ScanDiscoverItem {
                        path: rel_path.to_string(),
                        format: format.to_string(),
                        title,
                        file_size,
                    });
                }
            }
        }
    }

    HttpResponse::Ok().json(ScanDiscoverResponse {
        success: true,
        fail_code: None,
        files,
    })
}

#[derive(Deserialize)]
pub struct ScanProgressRequest {
    pub batch_id: String,
}

#[derive(Serialize)]
pub struct ScanProgressResponseData {
    pub success: bool,
    pub is_running: bool,
    pub total: u64,
    pub completed: u64,
    pub failed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_book: Option<String>,
}

pub async fn ebook_scan_progress(
    req: web::Json<ScanProgressRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let _user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    match app_state.ebook_scan_manager.get_progress(&req.batch_id) {
        Some(progress) => HttpResponse::Ok().json(ScanProgressResponseData {
            success: true,
            is_running: progress.is_running,
            total: progress.total,
            completed: progress.completed,
            failed: progress.failed,
            current_book: progress.current_book,
        }),
        None => HttpResponse::Ok().json(ScanProgressResponseData {
            success: true,
            is_running: false,
            total: 0,
            completed: 0,
            failed: 0,
            current_book: None,
        }),
    }
}

#[derive(Serialize)]
pub struct CategoryListResponse {
    pub success: bool,
    pub categories: Vec<ShelfCategoryItem>,
}

pub async fn ebook_category_list(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };

    let ebook_path = match get_ebook_path(&app_state, &user_id) {
        Ok(ep) => ep,
        Err(resp) => return resp,
    };

    let db = match EbookDb::open(Path::new(&root_path), &ebook_path) {
        Ok(db) => db,
        Err(e) => return internal_error_response("/api/ebook/category/list", &e),
    };

    let categories = match db.list_categories() {
        Ok(c) => c,
        Err(e) => return internal_error_response("/api/ebook/category/list", &e),
    };

    let cat_items: Vec<ShelfCategoryItem> = categories.iter().map(|c| ShelfCategoryItem {
        id: c.id.clone(),
        name: c.name.clone(),
        sort_order: c.sort_order,
    }).collect();

    HttpResponse::Ok().json(CategoryListResponse {
        success: true,
        categories: cat_items,
    })
}

#[derive(Deserialize)]
pub struct CategoryAddRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct CategoryAddResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

pub async fn ebook_category_add(
    req: web::Json<CategoryAddRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };

    let ebook_path = match get_ebook_path(&app_state, &user_id) {
        Ok(ep) => ep,
        Err(resp) => return resp,
    };

    if req.name.trim().is_empty() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
        });
    }

    let db = match EbookDb::open(Path::new(&root_path), &ebook_path) {
        Ok(db) => db,
        Err(e) => return internal_error_response("/api/ebook/category/add", &e),
    };

    match db.add_category(req.name.trim()) {
        Ok(id) => HttpResponse::Ok().json(CategoryAddResponse {
            success: true,
            fail_code: None,
            id: Some(id),
        }),
        Err(e) => internal_error_response("/api/ebook/category/add", &e),
    }
}

#[derive(Deserialize)]
pub struct CategoryRenameRequest {
    pub id: String,
    pub name: String,
}

pub async fn ebook_category_rename(
    req: web::Json<CategoryRenameRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };

    let ebook_path = match get_ebook_path(&app_state, &user_id) {
        Ok(ep) => ep,
        Err(resp) => return resp,
    };

    if req.name.trim().is_empty() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
        });
    }

    let db = match EbookDb::open(Path::new(&root_path), &ebook_path) {
        Ok(db) => db,
        Err(e) => return internal_error_response("/api/ebook/category/rename", &e),
    };

    match db.rename_category(&req.id, req.name.trim()) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse { success: true, fail_code: None }),
        Err(e) => internal_error_response("/api/ebook/category/rename", &e),
    }
}

#[derive(Deserialize)]
pub struct CategoryRemoveRequest {
    pub id: String,
}

pub async fn ebook_category_remove(
    req: web::Json<CategoryRemoveRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };

    let ebook_path = match get_ebook_path(&app_state, &user_id) {
        Ok(ep) => ep,
        Err(resp) => return resp,
    };

    let db = match EbookDb::open(Path::new(&root_path), &ebook_path) {
        Ok(db) => db,
        Err(e) => return internal_error_response("/api/ebook/category/remove", &e),
    };

    match db.remove_category(&req.id) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse { success: true, fail_code: None }),
        Err(e) => internal_error_response("/api/ebook/category/remove", &e),
    }
}

#[derive(Deserialize)]
pub struct CategoryMoveBookRequest {
    pub book_id: String,
    pub category_id: Option<String>,
}

pub async fn ebook_category_move_book(
    req: web::Json<CategoryMoveBookRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };

    let ebook_path = match get_ebook_path(&app_state, &user_id) {
        Ok(ep) => ep,
        Err(resp) => return resp,
    };

    let db = match EbookDb::open(Path::new(&root_path), &ebook_path) {
        Ok(db) => db,
        Err(e) => return internal_error_response("/api/ebook/category/move-book", &e),
    };

    match db.move_book(&req.book_id, req.category_id.as_deref()) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse { success: true, fail_code: None }),
        Err(e) => internal_error_response("/api/ebook/category/move-book", &e),
    }
}

#[derive(Deserialize)]
pub struct BookMetaRequest {
    pub book_id: String,
}

#[derive(Serialize)]
pub struct BookMetaChapter {
    pub chapter_no: i64,
    pub title: Option<String>,
    pub location: String,
}

#[derive(Serialize)]
pub struct BookMetaResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book: Option<ShelfBookItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chapters: Option<Vec<BookMetaChapter>>,
    pub source_status: String,
}

pub async fn ebook_book_meta(
    req: web::Json<BookMetaRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };

    let ebook_path = match get_ebook_path(&app_state, &user_id) {
        Ok(ep) => ep,
        Err(resp) => return resp,
    };

    let root_path_obj = Path::new(&root_path);
    let db = match EbookDb::open(root_path_obj, &ebook_path) {
        Ok(db) => db,
        Err(e) => return internal_error_response("/api/ebook/book/meta", &e),
    };

    let book = match db.get_book(&req.book_id) {
        Ok(Some(b)) => b,
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("BOOK_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/ebook/book/meta", &e),
    };

    let abs_path = root_path_obj.join(&book.source_path);
    let source_status = if !abs_path.exists() || !abs_path.is_file() {
        "file_not_found".to_string()
    } else {
        match compute_file_hash_and_size(&abs_path) {
            Ok((current_sha256, current_size)) => {
                if current_sha256 != book.sha256 || current_size != book.file_size {
                    "content_changed".to_string()
                } else {
                    "ok".to_string()
                }
            }
            Err(_) => "file_not_found".to_string(),
        }
    };

    let chapters = match db.list_chapters(&req.book_id) {
        Ok(chs) => chs.iter().map(|c| BookMetaChapter {
            chapter_no: c.chapter_no,
            title: c.title.clone(),
            location: c.location.clone(),
        }).collect(),
        Err(_) => Vec::new(),
    };

    let book_item = ShelfBookItem {
        id: book.id.clone(),
        title: book.title.clone(),
        format: book.format.clone(),
        file_size: book.file_size,
        sha256: book.sha256.clone(),
        category_id: book.category_id.clone(),
        added_at: book.added_at.clone(),
        scan_status: book.scan_status.clone(),
        has_preview: book.preview_filename.is_some(),
    };

    HttpResponse::Ok().json(BookMetaResponse {
        success: true,
        fail_code: None,
        book: Some(book_item),
        chapters: Some(chapters),
        source_status,
    })
}

#[derive(Deserialize)]
pub struct BookDownloadRequest {
    pub book_id: String,
}

pub async fn ebook_book_download(
    req: web::Json<BookDownloadRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };

    let ebook_path = match get_ebook_path(&app_state, &user_id) {
        Ok(ep) => ep,
        Err(resp) => return resp,
    };

    let root_path_obj = Path::new(&root_path);
    let db = match EbookDb::open(root_path_obj, &ebook_path) {
        Ok(db) => db,
        Err(e) => return internal_error_response("/api/ebook/book/download", &e),
    };

    let book = match db.get_book(&req.book_id) {
        Ok(Some(b)) => b,
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("BOOK_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/ebook/book/download", &e),
    };

    let data_dir = crate::ebook::ebook_meta_dir(root_path_obj, &ebook_path);
    let file_path = if book.format == "txt" {
        if let Some(ref cache) = book.txt_cache_filename {
            let cache_path = data_dir.join("cache").join(cache);
            if cache_path.exists() {
                cache_path
            } else {
                root_path_obj.join(&book.source_path)
            }
        } else {
            root_path_obj.join(&book.source_path)
        }
    } else {
        root_path_obj.join(&book.source_path)
    };

    if !file_path.exists() || !file_path.is_file() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("FILE_NOT_FOUND".to_string()),
        });
    }

    match std::fs::read(&file_path) {
        Ok(data) => {
            let content_type = match book.format.as_str() {
                "txt" => "text/plain; charset=utf-8",
                "epub" => "application/epub+zip",
                "pdf" => "application/pdf",
                _ => "application/octet-stream",
            };
            HttpResponse::Ok()
                .content_type(content_type)
                .body(data)
        }
        Err(e) => internal_error_response("/api/ebook/book/download", &e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct PreviewGetRequest {
    pub book_id: String,
}

pub async fn ebook_preview_get(
    req: web::Json<PreviewGetRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };

    let ebook_path = match get_ebook_path(&app_state, &user_id) {
        Ok(ep) => ep,
        Err(resp) => return resp,
    };

    let root_path_obj = Path::new(&root_path);
    let db = match EbookDb::open(root_path_obj, &ebook_path) {
        Ok(db) => db,
        Err(e) => return internal_error_response("/api/ebook/preview/get", &e),
    };

    let book = match db.get_book(&req.book_id) {
        Ok(Some(b)) => b,
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("BOOK_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/ebook/preview/get", &e),
    };

    let preview_filename = match book.preview_filename {
        Some(ref f) => f.clone(),
        None => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("PREVIEW_NOT_FOUND".to_string()),
            });
        }
    };

    let data_dir = crate::ebook::ebook_meta_dir(root_path_obj, &ebook_path);
    let preview_path = data_dir.join("previews").join(&preview_filename);

    if !preview_path.exists() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PREVIEW_NOT_FOUND".to_string()),
        });
    }

    match std::fs::read(&preview_path) {
        Ok(data) => {
            HttpResponse::Ok()
                .content_type("image/png")
                .body(data)
        }
        Err(e) => internal_error_response("/api/ebook/preview/get", &e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct ShelfCoverRequest {
    pub book_id: String,
    /// Frontend-rendered cover image: a raw base64 string or a data URL.
    pub image: String,
}

/// Accept a client-rendered cover image for a book and persist it as the
/// preview. PDF covers are rendered in the browser (pdf.js) and uploaded here;
/// EPUB/TXT covers remain server-generated and are not expected through this route.
pub async fn ebook_shelf_cover(
    req: web::Json<ShelfCoverRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    use base64::Engine;
    use image::codecs::png::PngEncoder;
    use image::ImageEncoder;
    use std::io::Cursor;

    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };
    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };
    let ebook_path = match get_ebook_path(&app_state, &user_id) {
        Ok(ep) => ep,
        Err(resp) => return resp,
    };

    let root_path_obj = Path::new(&root_path);
    let db = match EbookDb::open(root_path_obj, &ebook_path) {
        Ok(db) => db,
        Err(e) => return internal_error_response("/api/ebook/shelf/cover", &e),
    };

    let book = match db.get_book(&req.book_id) {
        Ok(Some(b)) => b,
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("BOOK_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/ebook/shelf/cover", &e),
    };

    // Decode base64, tolerating an optional `data:` prefix.
    let trimmed = req.image.trim();
    let b64 = if let Some(rest) = trimmed.strip_prefix("data:") {
        match rest.split_once(',') {
            Some((_, b)) => b.trim(),
            None => trimmed,
        }
    } else {
        trimmed
    };

    let raw = match base64::engine::general_purpose::STANDARD.decode(b64) {
        Ok(r) => r,
        Err(_) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INVALID_IMAGE".to_string()),
            });
        }
    };

    // Validate and normalize to PNG via the `image` crate.
    let img = match image::load_from_memory(&raw) {
        Ok(img) => img,
        Err(_) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("INVALID_IMAGE".to_string()),
            });
        }
    };

    let preview_filename = format!("{}.png", book.id);
    let data_dir = crate::ebook::ebook_meta_dir(root_path_obj, &ebook_path);
    let previews_dir = data_dir.join("previews");
    if let Err(e) = std::fs::create_dir_all(&previews_dir) {
        return internal_error_response("/api/ebook/shelf/cover", &e.to_string());
    }
    let preview_path = previews_dir.join(&preview_filename);

    let mut out: Vec<u8> = Vec::new();
    {
        let mut cursor = Cursor::new(&mut out);
        let encoder = PngEncoder::new(&mut cursor);
        if let Err(e) = encoder.write_image(img.as_bytes(), img.width(), img.height(), img.color().into()) {
            return internal_error_response("/api/ebook/shelf/cover", &e.to_string());
        }
    }

    if let Err(e) = std::fs::write(&preview_path, &out) {
        return internal_error_response("/api/ebook/shelf/cover", &e.to_string());
    }
    if let Err(e) = db.update_book_cover(&book.id, &preview_filename) {
        return internal_error_response("/api/ebook/shelf/cover", &e);
    }

    HttpResponse::Ok().json(ApiResponse {
        success: true,
        fail_code: None,
    })
}

#[derive(Deserialize)]
pub struct ProgressSaveRequest {
    pub book_id: String,
    pub slot: i32,
    pub content_coord: String,
    pub summary: Option<String>,
}

pub async fn ebook_progress_save(
    req: web::Json<ProgressSaveRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };

    let ebook_path = match get_ebook_path(&app_state, &user_id) {
        Ok(ep) => ep,
        Err(resp) => return resp,
    };

    if req.slot < 1 || req.slot > 3 {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_PARAM".to_string()),
        });
    }

    let db = match EbookDb::open(Path::new(&root_path), &ebook_path) {
        Ok(db) => db,
        Err(e) => return internal_error_response("/api/ebook/progress/save", &e),
    };

    match db.save_progress(&req.book_id, req.slot, &req.content_coord, req.summary.as_deref()) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse { success: true, fail_code: None }),
        Err(e) => internal_error_response("/api/ebook/progress/save", &e),
    }
}

#[derive(Deserialize)]
pub struct ProgressGetRequest {
    pub book_id: String,
}

#[derive(Serialize)]
pub struct ProgressSlotItem {
    pub slot: i64,
    pub content_coord: String,
    pub summary: Option<String>,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct ProgressGetResponse {
    pub success: bool,
    pub slots: Vec<ProgressSlotItem>,
}

pub async fn ebook_progress_get(
    req: web::Json<ProgressGetRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };

    let ebook_path = match get_ebook_path(&app_state, &user_id) {
        Ok(ep) => ep,
        Err(resp) => return resp,
    };

    let db = match EbookDb::open(Path::new(&root_path), &ebook_path) {
        Ok(db) => db,
        Err(e) => return internal_error_response("/api/ebook/progress/get", &e),
    };

    let slots = match db.get_progress(&req.book_id) {
        Ok(s) => s.iter().map(|p| ProgressSlotItem {
            slot: p.slot,
            content_coord: p.content_coord.clone(),
            summary: p.summary.clone(),
            updated_at: p.updated_at.clone(),
        }).collect(),
        Err(_) => Vec::new(),
    };

    HttpResponse::Ok().json(ProgressGetResponse {
        success: true,
        slots,
    })
}

#[derive(Deserialize)]
pub struct BookmarkAddRequest {
    pub book_id: String,
    pub content_coord: String,
    pub summary: Option<String>,
}

#[derive(Serialize)]
pub struct BookmarkAddResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fail_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

pub async fn ebook_bookmark_add(
    req: web::Json<BookmarkAddRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };

    let ebook_path = match get_ebook_path(&app_state, &user_id) {
        Ok(ep) => ep,
        Err(resp) => return resp,
    };

    let db = match EbookDb::open(Path::new(&root_path), &ebook_path) {
        Ok(db) => db,
        Err(e) => return internal_error_response("/api/ebook/bookmark/add", &e),
    };

    match db.add_bookmark(&req.book_id, &req.content_coord, req.summary.as_deref()) {
        Ok(id) => HttpResponse::Ok().json(BookmarkAddResponse {
            success: true,
            fail_code: None,
            id: Some(id),
        }),
        Err(e) => internal_error_response("/api/ebook/bookmark/add", &e),
    }
}

#[derive(Deserialize)]
pub struct BookmarkListRequest {
    pub book_id: String,
}

#[derive(Serialize)]
pub struct BookmarkItem {
    pub id: String,
    pub content_coord: String,
    pub summary: Option<String>,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct BookmarkListResponse {
    pub success: bool,
    pub bookmarks: Vec<BookmarkItem>,
}

pub async fn ebook_bookmark_list(
    req: web::Json<BookmarkListRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };

    let ebook_path = match get_ebook_path(&app_state, &user_id) {
        Ok(ep) => ep,
        Err(resp) => return resp,
    };

    let db = match EbookDb::open(Path::new(&root_path), &ebook_path) {
        Ok(db) => db,
        Err(e) => return internal_error_response("/api/ebook/bookmark/list", &e),
    };

    let bookmarks = match db.list_bookmarks(&req.book_id) {
        Ok(b) => b.iter().map(|bm| BookmarkItem {
            id: bm.id.clone(),
            content_coord: bm.content_coord.clone(),
            summary: bm.summary.clone(),
            created_at: bm.created_at.clone(),
        }).collect(),
        Err(_) => Vec::new(),
    };

    HttpResponse::Ok().json(BookmarkListResponse {
        success: true,
        bookmarks,
    })
}

#[derive(Deserialize)]
pub struct BookmarkRemoveRequest {
    pub id: String,
}

pub async fn ebook_bookmark_remove(
    req: web::Json<BookmarkRemoveRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(response) => return response,
    };

    let root_path = match get_user_root_path(&http_req, &app_state) {
        Ok(path) => path,
        Err(response) => return response,
    };

    let ebook_path = match get_ebook_path(&app_state, &user_id) {
        Ok(ep) => ep,
        Err(resp) => return resp,
    };

    let db = match EbookDb::open(Path::new(&root_path), &ebook_path) {
        Ok(db) => db,
        Err(e) => return internal_error_response("/api/ebook/bookmark/remove", &e),
    };

    match db.remove_bookmark(&req.id) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse { success: true, fail_code: None }),
        Err(e) => internal_error_response("/api/ebook/bookmark/remove", &e),
    }
}
