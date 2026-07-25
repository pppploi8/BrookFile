use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::handlers::{get_current_user_id, internal_error_response, ApiResponse};

fn is_valid_origin(origin: &str) -> bool {
    let origin = origin.trim();
    if origin.is_empty() || origin.len() > 255 {
        return false;
    }
    let rest = if let Some(r) = origin.strip_prefix("https://") {
        r
    } else if let Some(r) = origin.strip_prefix("http://") {
        r
    } else {
        return false;
    };
    if rest.is_empty() || rest.contains('/') || rest.contains('\\') || rest.contains(' ') {
        return false;
    }
    let (host, port) = match rest.rsplit_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (rest, None),
    };
    if host.is_empty() {
        return false;
    }
    if let Some(p) = port {
        if p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
    }
    true
}

#[derive(Debug, Serialize)]
pub struct ListWebDavCorsResponse {
    pub success: bool,
    pub origins: Vec<String>,
}

pub async fn list_webdav_cors(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    match app_state.webdav_cors_model.list_by_user(&user_id) {
        Ok(origins) => HttpResponse::Ok().json(ListWebDavCorsResponse {
            success: true,
            origins,
        }),
        Err(e) => internal_error_response("/api/webdav/cors/list", &e),
    }
}

#[derive(Debug, Deserialize)]
pub struct SaveWebDavCorsRequest {
    pub origins: Vec<String>,
}

pub async fn save_webdav_cors(
    body: web::Json<SaveWebDavCorsRequest>,
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let mut origins: Vec<String> = Vec::new();
    for raw in &body.origins {
        let origin = raw.trim().trim_end_matches('/').to_string();
        if !is_valid_origin(&origin) {
            return HttpResponse::Ok().json(ApiResponse {
                success: false,
                fail_code: Some("CORS_ORIGIN_INVALID".to_string()),
            });
        }
        if !origins.contains(&origin) {
            origins.push(origin);
        }
    }

    if let Err(e) = app_state.webdav_cors_model.save(&user_id, &origins) {
        return internal_error_response("/api/webdav/cors/save", &e);
    }

    match app_state.webdav_cors_model.all_origins() {
        Ok(all) => {
            if let Ok(mut cache) = app_state.cors_origins.write() {
                *cache = all;
            }
        }
        Err(e) => return internal_error_response("/api/webdav/cors/save", &e),
    }

    HttpResponse::Ok().json(ApiResponse {
        success: true,
        fail_code: None,
    })
}
