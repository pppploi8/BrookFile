mod config;
mod database;
mod handlers;
mod models;
mod app_state;
mod routes;
mod middleware;
mod error_logger;
mod session_manager;
mod storage;
mod backup;
mod restore;
mod search;
mod static_files;

use actix_web::{web, App, HttpServer};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use crate::backup::{BackupManager, BackupScheduler};
use crate::restore::RestoreManager;

fn start_cleanup_thread(pool: database::Pool, notebook_key_cache: Arc<Mutex<HashMap<String, (Vec<u8>, std::time::Instant)>>>, share_tokens: Arc<Mutex<HashMap<String, crate::app_state::ShareTokenEntry>>>) {
    std::thread::spawn(move || {
        let upload_cache_model = models::UploadCacheModel::new(&pool);
        let share_model = models::ShareModel::new(&pool);
        let upload_timeout_seconds: i64 = 300;
        
        loop {
            std::thread::sleep(Duration::from_secs(60));
            
            match upload_cache_model.get_expired_uploads(upload_timeout_seconds) {
                Ok(expired_uploads) => {
                    for cache in expired_uploads {
                        let temp_path = std::path::Path::new(&cache.temp_file_path);
                        if temp_path.exists() {
                            let _ = std::fs::remove_file(temp_path);
                        }
                        let _ = upload_cache_model.delete(&cache.id);
                    }
                }
                Err(e) => {
                    eprintln!("Cleanup thread error: {}", e);
                }
            }

            if let Err(e) = share_model.cleanup_expired(7) {
                error_logger::log_error("SHARE_CLEANUP", &format!("cleanup_expired error: {}", e));
            }

            {
                let mut tokens = share_tokens.lock().unwrap_or_else(|e| e.into_inner());
                let now = std::time::Instant::now();
                tokens.retain(|_, entry| entry.expires_at > now);
            }

            let mut cache = notebook_key_cache.lock().unwrap_or_else(|e| e.into_inner());
            let now = std::time::Instant::now();
            cache.retain(|_, (_, created_at)| now.duration_since(*created_at).as_secs() < 3600);
        }
    });
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::panic::set_hook(Box::new(|panic_info| {
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };
        
        let location = panic_info.location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());
        
        error_logger::log_error("PANIC", &format!("{} at {}", message, location));
    }));

    let config = config::Config::init();
    println!("Configuration loaded: port = {}", config.port);

    let database = database::Database::new().expect("Failed to initialize database");
    println!("Database initialized");

    let system_config_model = models::SystemConfigModel::new(&database.pool);

    let session_timeout: u64 = match system_config_model.get_config("session_timeout") {
        Ok(Some(v)) => v.parse().unwrap_or(1800),
        _ => {
            let default: u64 = 1800;
            let _ = system_config_model.set_config("session_timeout", &default.to_string());
            default
        }
    };

    let fulltext_enabled = match system_config_model.get_config("notebook_fulltext_search") {
        Ok(Some(v)) => v == "true",
        _ => {
            let _ = system_config_model.set_config("notebook_fulltext_search", "true");
            true
        }
    };
    let needs_rebuild_flag = match system_config_model.get_config("notebook_needs_rebuild") {
        Ok(Some(v)) => v == "true",
        _ => false,
    };
    println!("Fulltext search: {}", fulltext_enabled);

    let session_manager = session_manager::SessionManager::new(session_timeout);
    println!("SessionManager initialized (timeout: {}s)", session_timeout);

    let backup_manager = Arc::new(BackupManager::new(database.pool.clone()));
    backup_manager.handle_startup_interrupted_tasks().await;
    println!("BackupManager initialized");

    let backup_scheduler = Arc::new(BackupScheduler::new(Arc::clone(&backup_manager), database.pool.clone()));
    backup_scheduler.load_scheduled_rules().await;
    println!("BackupScheduler initialized");

    let restore_manager = Arc::new(RestoreManager::new());
    println!("RestoreManager initialized");

    let app_state = web::Data::new(app_state::AppState::new(
        database.pool.clone(),
        Arc::clone(&session_manager),
        Arc::clone(&backup_manager),
        Arc::clone(&backup_scheduler),
        Arc::clone(&restore_manager),
        fulltext_enabled,
    ));
    println!("AppState initialized");

    let pool_for_search = database.pool.clone();

    start_cleanup_thread(database.pool, app_state.notebook_key_cache.clone(), app_state.share_tokens.clone());

    println!("Cleanup thread started");

    let search_mgr = Arc::clone(&app_state.search_manager);
    let pool_clone = pool_for_search.clone();
    actix_web::rt::spawn(async move {
        if search_mgr.is_enabled() {
            let notebook_model = crate::models::NotebookModel::new(&pool_clone);
            let user_model = crate::models::UserModel::new(&pool_clone);
            match notebook_model.list_all_non_encrypted() {
                Ok(notebooks) => {
                    for (user_id, nb) in &notebooks {
                        if let Ok(Some(user)) = user_model.get_user_full(user_id) {
                            if let Some(rp) = user.root_path {
                                let full_path = format!("{}/{}", rp, nb.path);
                                if needs_rebuild_flag || search_mgr.needs_rebuild(&nb.id) {
                                    let _ = search_mgr.rebuild_notebook_index(&nb.id, &full_path);
                                }
                            }
                        }
                    }
                    if needs_rebuild_flag {
                        let config_model = crate::models::SystemConfigModel::new(&pool_clone);
                        let _ = config_model.set_config("notebook_needs_rebuild", "false");
                    }
                }
                Err(_) => {}
            }
        }
        loop {
            actix_web::rt::time::sleep(std::time::Duration::from_secs(86400)).await;
            if !search_mgr.is_enabled() {
                continue;
            }
            let pool_inner = crate::models::NotebookModel::new(&pool_clone);
            let user_model_inner = crate::models::UserModel::new(&pool_clone);
            match pool_inner.list_all_non_encrypted() {
                Ok(notebooks) => {
                    for (user_id, nb) in &notebooks {
                        if let Ok(Some(user)) = user_model_inner.get_user_full(user_id) {
                            if let Some(rp) = user.root_path {
                                let full_path = format!("{}/{}", rp, nb.path);
                                let _ = search_mgr.incremental_rebuild(&nb.id, &full_path);
                            }
                        }
                    }
                }
                Err(_) => {}
            }
        }
    });

    let scheduler_clone = Arc::clone(&backup_scheduler);
    tokio::spawn(async move {
        scheduler_clone.run().await;
    });
    println!("Backup scheduler started");

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::AuthMiddleware::new(Arc::clone(&session_manager)))
            .wrap(middleware::SessionMiddleware::new(Arc::clone(&session_manager)))
            .configure(|app| routes::configure(app, app_state.clone()))
    })
    .bind(("0.0.0.0", config.port))?
    .run()
    .await
}
