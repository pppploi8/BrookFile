use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::handlers::{get_current_user_id, internal_error_response, is_safe_path, ApiResponse};

fn is_valid_dav_path(dav_path: &str) -> bool {
    !dav_path.is_empty() && dav_path.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn is_valid_permission(permission: &str) -> bool {
    matches!(permission, "full_control" | "edit" | "read_only")
}

#[derive(Debug, Serialize)]
pub struct WebDavConfigItem {
    pub id: String,
    pub dav_path: String,
    pub access_path: String,
    pub permission: String,
    pub url: String,
    pub global_access: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct ListWebDavConfigResponse {
    pub success: bool,
    pub configs: Vec<WebDavConfigItem>,
}

pub async fn list_webdav_configs(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    match app_state.webdav_config_model.list_by_user(&user_id) {
        Ok(configs) => {
            let items: Vec<WebDavConfigItem> = configs
                .into_iter()
                .map(|c| WebDavConfigItem {
                    url: if c.dav_path.is_empty() {
                        "/dav/".to_string()
                    } else {
                        format!("/dav/{}/", c.dav_path)
                    },
                    id: c.id,
                    dav_path: c.dav_path,
                    access_path: c.access_path,
                    permission: c.permission,
                    global_access: c.global_access,
                    created_at: c.created_at,
                    updated_at: c.updated_at,
                })
                .collect();
            HttpResponse::Ok().json(ListWebDavConfigResponse {
                success: true,
                configs: items,
            })
        }
        Err(e) => internal_error_response("/api/webdav/list", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateWebDavConfigRequest {
    pub dav_path: String,
    pub access_path: String,
    pub password: String,
    pub permission: String,
    pub global_access: Option<bool>,
}

pub async fn create_webdav_config(
    body: web::Json<CreateWebDavConfigRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let global_access = body.global_access.unwrap_or(false);

    if !body.dav_path.is_empty() && !is_valid_dav_path(&body.dav_path) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("DAV_PATH_INVALID".to_string()),
        });
    }

    if global_access && !body.dav_path.is_empty() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("DAV_PATH_INVALID".to_string()),
        });
    }

    if !global_access && body.dav_path.is_empty() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("DAV_PATH_INVALID".to_string()),
        });
    }

    if !is_valid_permission(&body.permission) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PARAM_INVALID".to_string()),
        });
    }

    if body.password.is_empty() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PARAM_INVALID".to_string()),
        });
    }

    if !global_access {
        if !body.access_path.is_empty() && !is_safe_path(&body.access_path) {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("PARAM_INVALID".to_string()),
            });
        }
    } else if !body.access_path.is_empty() && !is_safe_path(&body.access_path) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PARAM_INVALID".to_string()),
        });
    }

    if global_access {
        match app_state.webdav_config_model.count_by_user(&user_id, None) {
            Ok(count) if count > 0 => {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("DAV_CONFIGS_CONFLICT".to_string()),
                });
            }
            Err(e) => return internal_error_response("/api/webdav/create", &e),
            _ => {}
        }
    } else {
        match app_state
            .webdav_config_model
            .has_global_config(&user_id, None)
        {
            Ok(true) => {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("DAV_GLOBAL_EXISTS".to_string()),
                });
            }
            Err(e) => return internal_error_response("/api/webdav/create", &e),
            _ => {}
        }
    }

    match app_state.webdav_config_model.create(
        &user_id,
        &body.dav_path,
        &body.access_path,
        &body.password,
        &body.permission,
        global_access,
    ) {
        Ok(_) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        }),
        Err(e) if e == "DAV_PATH_DUPLICATE" => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("DAV_PATH_DUPLICATE".to_string()),
        }),
        Err(e) => internal_error_response("/api/webdav/create", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateWebDavConfigRequest {
    pub id: String,
    pub dav_path: String,
    pub access_path: String,
    pub password: Option<String>,
    pub permission: String,
    pub global_access: Option<bool>,
}

pub async fn update_webdav_config(
    body: web::Json<UpdateWebDavConfigRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let global_access = body.global_access.unwrap_or(false);

    if !body.dav_path.is_empty() && !is_valid_dav_path(&body.dav_path) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("DAV_PATH_INVALID".to_string()),
        });
    }

    if global_access && !body.dav_path.is_empty() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("DAV_PATH_INVALID".to_string()),
        });
    }

    if !global_access && body.dav_path.is_empty() {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("DAV_PATH_INVALID".to_string()),
        });
    }

    if !is_valid_permission(&body.permission) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PARAM_INVALID".to_string()),
        });
    }

    if !global_access {
        if !body.access_path.is_empty() && !is_safe_path(&body.access_path) {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("PARAM_INVALID".to_string()),
            });
        }
    } else if !body.access_path.is_empty() && !is_safe_path(&body.access_path) {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("PARAM_INVALID".to_string()),
        });
    }

    match app_state
        .webdav_config_model
        .get_by_id(&user_id, &body.id)
    {
        Ok(Some(_)) => {}
        Ok(None) => {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("DAV_CONFIG_NOT_FOUND".to_string()),
            });
        }
        Err(e) => return internal_error_response("/api/webdav/update", &e),
    }

    if global_access {
        match app_state
            .webdav_config_model
            .count_by_user(&user_id, Some(&body.id))
        {
            Ok(count) if count > 0 => {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("DAV_CONFIGS_CONFLICT".to_string()),
                });
            }
            Err(e) => return internal_error_response("/api/webdav/update", &e),
            _ => {}
        }
    } else {
        match app_state
            .webdav_config_model
            .has_global_config(&user_id, Some(&body.id))
        {
            Ok(true) => {
                return HttpResponse::Ok().json(ApiResponse {
                    success: false,
                    fail_code: Some("DAV_GLOBAL_EXISTS".to_string()),
                });
            }
            Err(e) => return internal_error_response("/api/webdav/update", &e),
            _ => {}
        }
    }

    match app_state.webdav_config_model.update(
        &user_id,
        &body.id,
        &body.dav_path,
        &body.access_path,
        body.password.as_deref(),
        &body.permission,
        global_access,
    ) {
        Ok(()) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        }),
        Err(e) if e == "DAV_PATH_DUPLICATE" => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("DAV_PATH_DUPLICATE".to_string()),
        }),
        Err(e) => internal_error_response("/api/webdav/update", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct DeleteWebDavConfigRequest {
    pub id: String,
}

pub async fn delete_webdav_config(
    body: web::Json<DeleteWebDavConfigRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    match app_state.webdav_config_model.delete(&user_id, &body.id) {
        Ok(true) => HttpResponse::Ok().json(ApiResponse {
            success: true,
            fail_code: None,
        }),
        Ok(false) => HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("DAV_CONFIG_NOT_FOUND".to_string()),
        }),
        Err(e) => internal_error_response("/api/webdav/delete", &e),
    }
}
