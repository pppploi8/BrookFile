use actix_web::{body::EitherBody, dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform}, Error, HttpResponse};
use serde::Serialize;
use std::future::{ready, Ready};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use crate::app_state::AppState;
use crate::session_manager::SessionManager;
use super::get_session_id;

#[derive(Serialize)]
struct AuthResponse {
    success: bool,
    fail_code: String,
}

pub struct AuthMiddleware {
    session_manager: Arc<SessionManager>,
}

impl AuthMiddleware {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        AuthMiddleware { session_manager }
    }
}

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthMiddlewareInner<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddlewareInner {
            service: Rc::new(service),
            session_manager: Arc::clone(&self.session_manager),
        }))
    }
}

pub struct AuthMiddlewareInner<S> {
    service: Rc<S>,
    session_manager: Arc<SessionManager>,
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareInner<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let path = req.path();
        let app_state = req.app_data::<actix_web::web::Data<AppState>>();
        let is_initialized = app_state
            .and_then(|state| state.is_initialized().ok())
            .unwrap_or(false);

        match path {
            "/api/system/info" => {
                let service = self.service.clone();
                return Box::pin(async move {
                    service.call(req).await.map(|res| res.map_into_left_body())
                });
            }
            "/api/system/init" => {
                if is_initialized {
                    let response = HttpResponse::Ok().json(AuthResponse {
                        success: false,
                        fail_code: "SYSTEM_ALREADY_INITIALIZED".to_string(),
                    });
                    return Box::pin(async move {
                        Ok(req.into_response(response).map_into_right_body())
                    });
                }
                let service = self.service.clone();
                return Box::pin(async move {
                    service.call(req).await.map(|res| res.map_into_left_body())
                });
            }
            "/api/system/browse" => {
                if is_initialized {
                    let response = HttpResponse::Ok().json(AuthResponse {
                        success: false,
                        fail_code: "SYSTEM_ALREADY_INITIALIZED".to_string(),
                    });
                    return Box::pin(async move {
                        Ok(req.into_response(response).map_into_right_body())
                    });
                }
                let service = self.service.clone();
                return Box::pin(async move {
                    service.call(req).await.map(|res| res.map_into_left_body())
                });
            }
            "/api/auth/login" => {
                if !is_initialized {
                    let response = HttpResponse::Ok().json(AuthResponse {
                        success: false,
                        fail_code: "SYSTEM_NOT_INITIALIZED".to_string(),
                    });
                    return Box::pin(async move {
                        Ok(req.into_response(response).map_into_right_body())
                    });
                }
                let service = self.service.clone();
                return Box::pin(async move {
                    service.call(req).await.map(|res| res.map_into_left_body())
                });
            }
            _ => {
                if !path.starts_with("/api/") && !path.starts_with("/dav") {
                    let service = self.service.clone();
                    return Box::pin(async move {
                        service.call(req).await.map(|res| res.map_into_left_body())
                    });
                }

                if path == "/api/share/info" || path == "/api/share/get_download_token" || path.starts_with("/api/share/file/") || path == "/api/notebook/attachment" || path.starts_with("/dav") {
                    if !is_initialized {
                        let response = HttpResponse::Ok().json(AuthResponse {
                            success: false,
                            fail_code: "SYSTEM_NOT_INITIALIZED".to_string(),
                        });
                        return Box::pin(async move {
                            Ok(req.into_response(response).map_into_right_body())
                        });
                    }
                    let service = self.service.clone();
                    return Box::pin(async move {
                        service.call(req).await.map(|res| res.map_into_left_body())
                    });
                }

                if !is_initialized {
                    let response = HttpResponse::Ok().json(AuthResponse {
                        success: false,
                        fail_code: "SYSTEM_NOT_INITIALIZED".to_string(),
                    });
                    return Box::pin(async move {
                        Ok(req.into_response(response).map_into_right_body())
                    });
                }

                let session_id = get_session_id(req.request());
                let is_logged_in = session_id
                    .as_ref()
                    .and_then(|id| self.session_manager.get(id, "username"))
                    .is_some();

                if !is_logged_in {
                    let response = HttpResponse::Ok().json(AuthResponse {
                        success: false,
                        fail_code: "NOT_LOGGED_IN".to_string(),
                    });
                    return Box::pin(async move {
                        Ok(req.into_response(response).map_into_right_body())
                    });
                }

                let user_id = session_id.as_ref().and_then(|id| self.session_manager.get(id, "user_id"));
                if let Some(uid) = user_id {
                    let last_verified = session_id.as_ref().and_then(|id| self.session_manager.get(id, "user_verified_at"));
                    let needs_verify = match last_verified {
                        None => true,
                        Some(ts) => ts.parse::<u64>().unwrap_or(0) < std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs()
                            .saturating_sub(300),
                    };
                    if needs_verify {
                        if let Some(state) = req.app_data::<actix_web::web::Data<AppState>>() {
                            let user_info = state.user_model.get_user_full(&uid);
                            let valid = match &user_info {
                                Ok(Some(user)) => {
                                    if let Some(ref expire_at) = user.expire_at {
                                        match chrono::DateTime::parse_from_rfc3339(expire_at) {
                                            Ok(expire_time) => expire_time.with_timezone(&chrono::Utc) >= chrono::Utc::now(),
                                Err(_) => false,
                                        }
                                    } else {
                                        true
                                    }
                                }
                                Ok(None) => false,
                                Err(_) => false,
                            };
                            if !valid {
                                if let Some(ref sid) = session_id {
                                    self.session_manager.invalidate(sid);
                                }
                                let response = HttpResponse::Ok().json(AuthResponse {
                                    success: false,
                                    fail_code: "NOT_LOGGED_IN".to_string(),
                                });
                                return Box::pin(async move {
                                    Ok(req.into_response(response).map_into_right_body())
                                });
                            }
                            if let Some(ref sid) = session_id {
                                let now = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs()
                                    .to_string();
                                self.session_manager.set(sid, "user_verified_at", &now);
                            }
                        }
                    }
                }

                let service = self.service.clone();
                Box::pin(async move {
                    service.call(req).await.map(|res| res.map_into_left_body())
                })
            }
        }
    }
}
