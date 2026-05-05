use actix_web::{dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform}, Error, HttpMessage};
use actix_web::cookie::{Cookie, SameSite};
use std::future::{ready, Ready};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use crate::session_manager::SessionManager;

pub struct SessionMiddleware {
    session_manager: Arc<SessionManager>,
    cookie_name: String,
}

impl SessionMiddleware {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        SessionMiddleware {
            session_manager,
            cookie_name: "session_id".to_string(),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for SessionMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = SessionMiddlewareInner<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(SessionMiddlewareInner {
            service: Rc::new(service),
            session_manager: Arc::clone(&self.session_manager),
            cookie_name: self.cookie_name.clone(),
        }))
    }
}

pub struct SessionMiddlewareInner<S> {
    service: Rc<S>,
    session_manager: Arc<SessionManager>,
    cookie_name: String,
}

impl<S, B> Service<ServiceRequest> for SessionMiddlewareInner<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let session_id = match req.cookie(&self.cookie_name) {
            Some(cookie) => {
                let id = cookie.value().to_string();
                if self.session_manager.validate_session(&id) {
                    id
                } else {
                    self.session_manager.create_session()
                }
            }
            None => {
                self.session_manager.create_session()
            }
        };

        req.extensions_mut().insert(session_id.clone());

        let service = self.service.clone();
        let cookie_name = self.cookie_name.clone();
        let session_manager = Arc::clone(&self.session_manager);
        let existing_cookie = req.cookie(&cookie_name);
        let needs_new_cookie = existing_cookie.is_none() || existing_cookie.map(|c| c.value() != session_id).unwrap_or(true);

        Box::pin(async move {
            let mut res = service.call(req).await?;

            let regenerated = session_manager.take_regenerated(&session_id);
            if needs_new_cookie || regenerated.is_some() {
                let final_session_id = regenerated.unwrap_or(session_id);
                let cookie = Cookie::build(&cookie_name, final_session_id)
                    .path("/")
                    .http_only(true)
                    .same_site(SameSite::Lax)
                    .finish();
                if let Ok(value) = cookie.to_string().parse() {
                    res.headers_mut().append(
                        actix_web::http::header::SET_COOKIE,
                        value,
                    );
                }
            }

            Ok(res)
        })
    }
}

pub fn get_session_id(req: &actix_web::HttpRequest) -> Option<String> {
    req.extensions().get::<String>().cloned()
}
