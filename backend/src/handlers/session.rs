use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use rusqlite::params;
use crate::app_state::AppState;
use crate::handlers::response::{ApiResponse, get_current_user_id, internal_error_response};
use crate::middleware::get_session_id;

#[derive(Serialize)]
struct SessionInfo {
    id: String,
    device_name: String,
    user_agent: String,
    ip_address: String,
    created_at: u64,
    last_access_time: u64,
    is_current: bool,
}

#[derive(Serialize)]
struct ListSessionsResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    fail_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sessions: Option<Vec<SessionInfo>>,
}

#[derive(Deserialize)]
pub struct UpdateSessionNameRequest {
    session_id: String,
    device_name: String,
}

#[derive(Deserialize)]
pub struct RevokeSessionRequest {
    session_id: String,
}

pub async fn list_sessions(
    http_req: HttpRequest,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let current_session_id = get_session_id(&http_req).unwrap_or_default();

    let conn = match app_state.session_manager.pool().get() {
        Ok(c) => c,
        Err(e) => return internal_error_response("/api/session/list", &e.to_string()),
    };

    let mut stmt = match conn.prepare(
        "SELECT id, device_name, user_agent, ip_address, created_at, last_access_time \
         FROM sessions WHERE user_id = ?1 ORDER BY last_access_time DESC"
    ) {
        Ok(s) => s,
        Err(e) => return internal_error_response("/api/session/list", &e.to_string()),
    };

    let rows = match stmt.query_map(params![user_id], |row| {
        Ok(SessionInfo {
            id: row.get(0)?,
            device_name: row.get(1)?,
            user_agent: row.get(2)?,
            ip_address: row.get(3)?,
            created_at: row.get::<_, i64>(4)? as u64,
            last_access_time: row.get::<_, i64>(5)? as u64,
            is_current: false,
        })
    }) {
        Ok(r) => r,
        Err(e) => return internal_error_response("/api/session/list", &e.to_string()),
    };

    let mut sessions: Vec<SessionInfo> = rows.filter_map(|r| r.ok()).collect();
    for s in &mut sessions {
        s.is_current = s.id == current_session_id;
    }

    HttpResponse::Ok().json(ListSessionsResponse {
        success: true,
        fail_code: None,
        sessions: Some(sessions),
    })
}

pub async fn update_session_name(
    http_req: HttpRequest,
    body: web::Json<UpdateSessionNameRequest>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    if body.device_name.len() > 100 {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("INVALID_PARAMETER".to_string()),
        });
    }

    let conn = match app_state.session_manager.pool().get() {
        Ok(c) => c,
        Err(e) => return internal_error_response("/api/session/update_name", &e.to_string()),
    };

    let affected = match conn.execute(
        "UPDATE sessions SET device_name = ?1 WHERE id = ?2 AND user_id = ?3",
        params![body.device_name, body.session_id, user_id],
    ) {
        Ok(n) => n,
        Err(e) => return internal_error_response("/api/session/update_name", &e.to_string()),
    };

    if affected == 0 {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("SESSION_NOT_FOUND".to_string()),
        });
    }

    HttpResponse::Ok().json(ApiResponse {
        success: true,
        fail_code: None,
    })
}

pub async fn revoke_session(
    http_req: HttpRequest,
    body: web::Json<RevokeSessionRequest>,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    let user_id = match get_current_user_id(&http_req, &app_state) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let current_session_id = get_session_id(&http_req).unwrap_or_default();
    if body.session_id == current_session_id {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("CANNOT_REVOKE_CURRENT".to_string()),
        });
    }

    let conn = match app_state.session_manager.pool().get() {
        Ok(c) => c,
        Err(e) => return internal_error_response("/api/session/revoke", &e.to_string()),
    };

    let belongs_to_user: bool = match conn.query_row(
        "SELECT COUNT(*) FROM sessions WHERE id = ?1 AND user_id = ?2",
        params![body.session_id, user_id],
        |row| row.get::<_, i64>(0),
    ) {
        Ok(n) => n > 0,
        Err(e) => return internal_error_response("/api/session/revoke", &e.to_string()),
    };

    if !belongs_to_user {
        return HttpResponse::Ok().json(ApiResponse {
            success: false,
            fail_code: Some("SESSION_NOT_FOUND".to_string()),
        });
    }

    app_state.session_manager.invalidate(&body.session_id);

    HttpResponse::Ok().json(ApiResponse {
        success: true,
        fail_code: None,
    })
}
