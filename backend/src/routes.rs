use crate::app_state::AppState;
use crate::handlers;
use actix_web::web;

pub fn configure(app: &mut web::ServiceConfig, app_state: web::Data<AppState>) {
    app.app_data(app_state.clone())
        .route(
            "/api/system/info",
            web::post().to(handlers::get_system_info),
        )
        .route("/api/system/init", web::post().to(handlers::init_system))
        .route(
            "/api/system/browse",
            web::post().to(handlers::browse_folders),
        )
        .route(
            "/api/system/get_settings",
            web::post().to(handlers::get_settings),
        )
        .route(
            "/api/system/update_settings",
            web::post().to(handlers::update_settings),
        )
        .route(
            "/api/system/rebuild_notebook_index",
            web::post().to(handlers::rebuild_notebook_index),
        )
        .route(
            "/api/system/upload_logo",
            web::post().to(handlers::upload_system_logo),
        )
        .route(
            "/api/system/delete_logo",
            web::post().to(handlers::delete_system_logo),
        )
        .route(
            "/api/system/logo",
            web::get().to(handlers::get_system_logo),
        )
        .route("/api/auth/login", web::post().to(handlers::login))
        .route("/api/auth/logout", web::post().to(handlers::logout))
        .route("/api/file/browse", web::post().to(handlers::browse_files))
        .route(
            "/api/file/download",
            web::post().to(handlers::download_file),
        )
        .route(
            "/api/file/create_folder",
            web::post().to(handlers::create_folder),
        )
        .route("/api/file/delete", web::post().to(handlers::delete_file))
        .route("/api/file/move", web::post().to(handlers::move_files))
        .route("/api/file/rename", web::post().to(handlers::rename_file))
        .route(
            "/api/file/batch_delete",
            web::post().to(handlers::batch_delete),
        )
        .route(
            "/api/file/upload_start",
            web::post().to(handlers::upload_start),
        )
        .route(
            "/api/file/upload_chunk",
            web::post().to(handlers::upload_chunk),
        )
        .route(
            "/api/file/upload_complete",
            web::post().to(handlers::upload_complete),
        )
        .route(
            "/api/file/upload_cancel",
            web::post().to(handlers::upload_cancel),
        )
        .route(
            "/api/file/download_folder",
            web::post().to(handlers::download_folder),
        )
        .route("/api/user/list", web::post().to(handlers::list_users))
        .route("/api/user/get", web::post().to(handlers::get_user))
        .route("/api/user/create", web::post().to(handlers::create_user))
        .route("/api/user/update", web::post().to(handlers::update_user))
        .route("/api/user/delete", web::post().to(handlers::delete_user))
        .route(
            "/api/user/upload_avatar",
            web::post().to(handlers::upload_avatar),
        )
        .route("/api/user/get_avatar", web::post().to(handlers::get_avatar))
        .route(
            "/api/user/delete_avatar",
            web::post().to(handlers::delete_avatar),
        )
        .route(
            "/api/user/change_password",
            web::post().to(handlers::change_password),
        )
        .route(
            "/api/user/update_feature_order",
            web::post().to(handlers::update_feature_order),
        )
        .route(
            "/api/backup/list",
            web::post().to(handlers::list_backup_rules),
        )
        .route("/api/backup/get", web::post().to(handlers::get_backup_rule))
        .route(
            "/api/backup/create",
            web::post().to(handlers::create_backup_rule),
        )
        .route(
            "/api/backup/update",
            web::post().to(handlers::update_backup_rule),
        )
        .route(
            "/api/backup/delete",
            web::post().to(handlers::delete_backup_rule),
        )
        .route("/api/backup/start", web::post().to(handlers::start_backup))
        .route(
            "/api/backup/cancel",
            web::post().to(handlers::cancel_backup),
        )
        .route(
            "/api/backup/progress",
            web::post().to(handlers::get_backup_progress),
        )
        .route(
            "/api/backup/logs",
            web::post().to(handlers::get_backup_logs),
        )
        .route(
            "/api/restore/check",
            web::post().to(handlers::check_restore_target),
        )
        .route(
            "/api/restore/start",
            web::post().to(handlers::start_restore),
        )
        .route(
            "/api/restore/progress",
            web::post().to(handlers::get_restore_progress),
        )
        .route(
            "/api/restore/cancel",
            web::post().to(handlers::cancel_restore),
        )
        .route(
            "/api/restore/retry_file",
            web::post().to(handlers::retry_restore_file),
        )
        .route(
            "/api/recycle/list",
            web::post().to(handlers::list_recycle_bin),
        )
        .route(
            "/api/recycle/restore",
            web::post().to(handlers::restore_recycle_item),
        )
        .route(
            "/api/recycle/batch_restore",
            web::post().to(handlers::batch_restore_recycle_items),
        )
        .route(
            "/api/recycle/delete",
            web::post().to(handlers::delete_recycle_item),
        )
        .route(
            "/api/recycle/batch_delete",
            web::post().to(handlers::batch_delete_recycle_items),
        )
        .route(
            "/api/recycle/empty",
            web::post().to(handlers::empty_recycle_bin),
        )
        .route("/api/vault/list", web::post().to(handlers::list_vaults))
        .route("/api/vault/create", web::post().to(handlers::create_vault))
        .route("/api/vault/update", web::post().to(handlers::update_vault))
        .route("/api/vault/update_meta", web::post().to(handlers::update_vault_meta))
        .route("/api/vault/delete", web::post().to(handlers::delete_vault))
        .route("/api/vault/import", web::post().to(handlers::import_vault))
        .route(
            "/api/vault/upload_single",
            web::post().to(handlers::upload_single),
        )
        .route(
            "/api/notebook/list",
            web::post().to(handlers::list_notebooks),
        )
        .route(
            "/api/notebook/create",
            web::post().to(handlers::create_notebook),
        )
        .route(
            "/api/notebook/open",
            web::post().to(handlers::open_notebook),
        )
        .route(
            "/api/notebook/update",
            web::post().to(handlers::update_notebook),
        )
        .route(
            "/api/notebook/delete",
            web::post().to(handlers::delete_notebook),
        )
        .route(
            "/api/notebook/read_note",
            web::post().to(handlers::read_note),
        )
        .route(
            "/api/notebook/create_folder",
            web::post().to(handlers::create_notebook_folder),
        )
        .route(
            "/api/notebook/save_note",
            web::post().to(handlers::save_note),
        )
        .route(
            "/api/notebook/save_conflict",
            web::post().to(handlers::save_conflict),
        )
        .route(
            "/api/notebook/file_tree",
            web::post().to(handlers::file_tree),
        )
        .route(
            "/api/notebook/rename",
            web::post().to(handlers::rename_note),
        )
        .route("/api/notebook/move", web::post().to(handlers::move_note))
        .route(
            "/api/notebook/attachment",
            web::get().to(handlers::get_notebook_attachment),
        )
        .route(
            "/api/notebook/attachment_token",
            web::post().to(handlers::attachment_token),
        )
        .route(
            "/api/notebook/search",
            web::post().to(handlers::search_notes),
        )
        .route(
            "/api/notebook/upload_attachment",
            web::post().to(handlers::upload_notebook_attachment),
        )
        .route(
            "/api/notebook/delete_folder",
            web::post().to(handlers::delete_notebook_folder),
        )
        .route(
            "/api/notebook/batch_delete",
            web::post().to(handlers::batch_delete_notebook_files),
        )
        .route("/api/share/info", web::post().to(handlers::get_share_info))
        .route(
            "/api/share/get_download_token",
            web::post().to(handlers::get_share_download_token),
        )
        .route(
            "/api/share/file/{code}",
            web::get().to(handlers::download_share_file),
        )
        .route("/api/share/create", web::post().to(handlers::create_share))
        .route(
            "/api/share/get_by_path",
            web::post().to(handlers::get_share_by_path),
        )
        .route("/api/share/list", web::post().to(handlers::list_shares))
        .route("/api/share/delete", web::post().to(handlers::delete_shares))
        .route(
            "/api/webdav/list",
            web::post().to(handlers::list_webdav_configs),
        )
        .route(
            "/api/webdav/create",
            web::post().to(handlers::create_webdav_config),
        )
        .route(
            "/api/webdav/update",
            web::post().to(handlers::update_webdav_config),
        )
        .route(
            "/api/webdav/delete",
            web::post().to(handlers::delete_webdav_config),
        )
        .service(
            web::scope("/dav")
                .route("", web::route().to(handlers::dav_handler))
                .route("/{path:[^{}]*}", web::route().to(handlers::dav_handler)),
        )
        .default_service(
            web::route()
                .guard(actix_web::guard::Get())
                .to(crate::static_files::serve_static),
        );
}
