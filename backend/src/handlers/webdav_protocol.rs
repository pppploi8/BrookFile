use actix_web::{web, HttpRequest, HttpResponse};
use base64::Engine;
use md5::Md5;
use rand::Rng;
use sha2::Digest;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, Instant};
use urlencoding::{decode, encode};

use crate::app_state::AppState;
use crate::error_logger;
use crate::handlers::move_recursive;

struct DavAuthResult {
    root_path: String,
    config: crate::models::WebDavConfigInfo,
}

fn parse_basic_auth(req: &HttpRequest) -> Option<(String, String)> {
    let auth_header = req.headers().get("Authorization")?.to_str().ok()?;
    if !auth_header.starts_with("Basic ") {
        return None;
    }
    let encoded = &auth_header[6..];
    let decoded = base64::engine::general_purpose::STANDARD.decode(encoded).ok()?;
    let credentials = String::from_utf8(decoded).ok()?;
    let parts: Vec<&str> = credentials.splitn(2, ':').collect();
    if parts.len() != 2 {
        return None;
    }
    Some((parts[0].to_string(), parts[1].to_string()))
}

fn parse_digest_auth(req: &HttpRequest) -> Option<HashMap<String, String>> {
    let auth_header = req.headers().get("Authorization")?.to_str().ok()?;
    if !auth_header.starts_with("Digest ") {
        return None;
    }
    let params_str = &auth_header[7..];
    let mut params = HashMap::new();
    let bytes = params_str.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        while i < len && bytes[i] == b' ' {
            i += 1;
        }
        if i >= len {
            break;
        }

        let key_start = i;
        while i < len && bytes[i] != b'=' {
            i += 1;
        }
        if i >= len {
            break;
        }
        let key = params_str[key_start..i].trim().to_string();
        i += 1;

        while i < len && bytes[i] == b' ' {
            i += 1;
        }

        let value = if i < len && bytes[i] == b'"' {
            i += 1;
            let vs = i;
            while i < len && bytes[i] != b'"' {
                i += 1;
            }
            let v = params_str[vs..i].to_string();
            if i < len {
                i += 1;
            }
            v
        } else {
            let vs = i;
            while i < len && bytes[i] != b',' && bytes[i] != b' ' {
                i += 1;
            }
            params_str[vs..i].to_string()
        };

        params.insert(key, value);

        while i < len && (bytes[i] == b',' || bytes[i] == b' ') {
            i += 1;
        }
    }

    Some(params)
}

const NONCE_EXPIRY: Duration = Duration::from_secs(300);
const REALM: &str = "WebDAV";

fn nonce_store() -> &'static RwLock<HashMap<String, Instant>> {
    static STORE: OnceLock<RwLock<HashMap<String, Instant>>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn generate_nonce() -> String {
    let nonce: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    nonce_store()
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .insert(nonce.clone(), Instant::now());
    nonce
}

fn is_nonce_valid(nonce: &str) -> bool {
    let store = nonce_store().read().unwrap_or_else(|e| e.into_inner());
    match store.get(nonce) {
        Some(created) => created.elapsed() < NONCE_EXPIRY,
        None => false,
    }
}

fn cleanup_old_nonces() {
    let mut store = nonce_store().write().unwrap_or_else(|e| e.into_inner());
    store.retain(|_, created| created.elapsed() < NONCE_EXPIRY);
}

fn md5_hex(input: String) -> String {
    let mut hasher = Md5::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

fn verify_digest_response(
    ha1: &str,
    method: &str,
    auth_params: &HashMap<String, String>,
) -> bool {
    let nonce = match auth_params.get("nonce") {
        Some(n) => n.as_str(),
        None => return false,
    };

    if !is_nonce_valid(nonce) {
        return false;
    }

    let nonce_owned = nonce.to_string();

    let uri = match auth_params.get("uri") {
        Some(u) => u.as_str(),
        None => return false,
    };
    let response = match auth_params.get("response") {
        Some(r) => r.as_str(),
        None => return false,
    };

    let ha2 = md5_hex(format!("{}:{}", method, uri));

    let expected = match auth_params.get("qop").map(|s| s.as_str()) {
        Some("auth") => {
            let nc = auth_params.get("nc").map(|s| s.as_str()).unwrap_or("");
            let cnonce = auth_params.get("cnonce").map(|s| s.as_str()).unwrap_or("");
            md5_hex(format!(
                "{}:{}:{}:{}:auth:{}",
                ha1, nonce, nc, cnonce, ha2
            ))
        }
        None => md5_hex(format!("{}:{}:{}", ha1, nonce, ha2)),
        _ => return false,
    };

    if expected.eq_ignore_ascii_case(response) {
        nonce_store()
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&nonce_owned);
        true
    } else {
        false
    }
}

fn is_https(req: &HttpRequest) -> bool {
    if let Some(v) = req.headers().get("X-Forwarded-Proto") {
        if let Ok(val) = v.to_str() {
            if val.eq_ignore_ascii_case("https") {
                return true;
            }
        }
    }
    req.connection_info().scheme() == "https"
}

fn make_unauthorized(https: bool) -> HttpResponse {
    cleanup_old_nonces();
    let nonce = generate_nonce();
    let digest_challenge = format!(
        "Digest realm=\"{}\", nonce=\"{}\", qop=\"auth\"",
        REALM, nonce
    );
    let mut builder = HttpResponse::Unauthorized();
    if https {
        builder.insert_header(("WWW-Authenticate", format!("Basic realm=\"{}\"", REALM)));
    }
    builder.insert_header(("WWW-Authenticate", digest_challenge));
    builder.finish()
}

fn authenticate_dav(
    req: &HttpRequest,
    app_state: &web::Data<AppState>,
    dav_path: &str,
) -> Result<DavAuthResult, HttpResponse> {
    let https = is_https(req);

    if let Some(params) = parse_digest_auth(req) {
        let username = match params.get("username") {
            Some(u) => u.as_str(),
            None => return Err(make_unauthorized(https)),
        };

        let user = match app_state.user_model.get_user_by_username(username) {
            Ok(Some(u)) => u,
            Ok(None) => return Err(make_unauthorized(https)),
            Err(e) => {
                error_logger::log_error("DAV_AUTH", &e.to_string());
                return Err(HttpResponse::InternalServerError().finish());
            }
        };

        let root_path = match &user.root_path {
            Some(p) if !p.is_empty() => p.clone(),
            _ => return Err(HttpResponse::InternalServerError().finish()),
        };

        let config = match app_state
            .webdav_config_model
            .get_by_dav_path(&user.id, dav_path)
        {
            Ok(Some(c)) => c,
            Ok(None) => return Err(HttpResponse::NotFound().finish()),
            Err(e) => {
                error_logger::log_error("DAV_AUTH", &e.to_string());
                return Err(HttpResponse::InternalServerError().finish());
            }
        };

        match &config.digest_ha1 {
            Some(ha1)
                if verify_digest_response(ha1, req.method().as_str(), &params) =>
            {
                return Ok(DavAuthResult { root_path, config });
            }
            _ => return Err(make_unauthorized(https)),
        }
    }

    if let Some((username, password)) = parse_basic_auth(req) {
        if !https {
            return Err(make_unauthorized(https));
        }

        let user = match app_state.user_model.get_user_by_username(&username) {
            Ok(Some(u)) => u,
            Ok(None) => return Err(make_unauthorized(https)),
            Err(e) => {
                error_logger::log_error("DAV_AUTH", &e.to_string());
                return Err(HttpResponse::InternalServerError().finish());
            }
        };

        let root_path = match &user.root_path {
            Some(p) if !p.is_empty() => p.clone(),
            _ => return Err(HttpResponse::InternalServerError().finish()),
        };

        let config = match app_state
            .webdav_config_model
            .get_by_dav_path(&user.id, dav_path)
        {
            Ok(Some(c)) => c,
            Ok(None) => return Err(HttpResponse::NotFound().finish()),
            Err(e) => {
                error_logger::log_error("DAV_AUTH", &e.to_string());
                return Err(HttpResponse::InternalServerError().finish());
            }
        };

        if app_state.webdav_config_model.verify_password(&config, &password) {
            return Ok(DavAuthResult { root_path, config });
        }

        return Err(make_unauthorized(https));
    }

    Err(make_unauthorized(https))
}

struct DavContext {
    dav_path: String,
    relative: String,
    auth: DavAuthResult,
}

fn decode_path_segment(s: &str) -> Option<String> {
    decode(s).ok().map(|c| c.into_owned())
}

fn normalize_dav_path(raw_path: &str) -> String {
    let path = raw_path.trim_matches('/');
    if path == "dav" || path.is_empty() {
        return String::new();
    }
    path.strip_prefix("dav/")
        .unwrap_or(path)
        .to_string()
}

fn try_authenticate(
    req: &HttpRequest,
    app_state: &web::Data<AppState>,
    url_path: &str,
) -> Result<DavContext, HttpResponse> {
    let path = normalize_dav_path(url_path);

    if path.is_empty() {
        let auth = authenticate_dav(req, app_state, "")?;
        return Ok(DavContext {
            dav_path: String::new(),
            relative: String::new(),
            auth,
        });
    }

    let (first, rest) = if let Some(slash_pos) = path.find('/') {
        (&path[..slash_pos], &path[slash_pos + 1..])
    } else {
        (&*path, "")
    };

    let first_decoded = decode_path_segment(first).unwrap_or_else(|| first.to_string());
    let rest_decoded = decode_path_segment(rest).unwrap_or_else(|| rest.to_string());

    match authenticate_dav(req, app_state, &first_decoded) {
        Ok(auth) => Ok(DavContext {
            dav_path: first_decoded,
            relative: rest_decoded,
            auth,
        }),
        Err(resp) => {
            let status = resp.status();
            if status == 404 {
                let path_decoded = decode_path_segment(&path).unwrap_or_else(|| path.clone());
                match authenticate_dav(req, app_state, "") {
                    Ok(auth) => Ok(DavContext {
                        dav_path: String::new(),
                        relative: path_decoded,
                        auth,
                    }),
                    Err(inner) => {
                        let inner_status = inner.status();
                        if inner_status == 404 {
                            Err(HttpResponse::NotFound().finish())
                        } else {
                            Err(inner)
                        }
                    }
                }
            } else {
                Err(resp)
            }
        }
    }
}

fn check_permission(config: &crate::models::WebDavConfigInfo, action: &str) -> bool {
    match config.permission.as_str() {
        "full_control" => true,
        "edit" => action != "delete",
        "read_only" => action == "read",
        _ => false,
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn url_encode_path(path: &str) -> String {
    path.split('/')
        .map(|seg| encode(seg).to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn format_http_date(path: &Path) -> String {
    path.metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| {
            let dt: chrono::DateTime<chrono::Utc> = t.into();
            dt.format("%a, %d %b %Y %H:%M:%S GMT").to_string()
        })
        .unwrap_or_default()
}

fn build_propfind_xml(
    base: &Path,
    relative: &str,
    depth: &str,
    href_prefix: &str,
) -> String {
    let target = if relative.is_empty() {
        base.to_path_buf()
    } else {
        base.join(relative)
    };

    let mut responses = String::new();

    let href_self = if relative.is_empty() {
        href_prefix.to_string()
    } else if target.is_dir() {
        format!("{}{}/", href_prefix, url_encode_path(relative))
    } else {
        format!("{}{}", href_prefix, url_encode_path(relative))
    };
    responses.push_str(&build_response_xml(&href_self, &target));

    if depth != "0" && target.is_dir() {
        append_child_responses(&target, relative, depth, href_prefix, &mut responses);
    }

    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <D:multistatus xmlns:D=\"DAV:\">\
         {}\
         </D:multistatus>",
        responses
    )
}

fn append_child_responses(
    dir: &Path,
    relative: &str,
    depth: &str,
    href_prefix: &str,
    responses: &mut String,
) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by(|a, b| {
            let a_is_dir = a.path().is_dir();
            let b_is_dir = b.path().is_dir();
            b_is_dir.cmp(&a_is_dir).then(a.file_name().cmp(&b.file_name()))
        });
        for entry in entries {
            let name = entry.file_name().to_string_lossy().to_string();
            let entry_relative = if relative.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", relative, name)
            };
            let entry_href = format!("{}{}", href_prefix, url_encode_path(&entry_relative));
            let is_dir = entry.path().is_dir();
            let display_href = if is_dir {
                format!("{}/", entry_href)
            } else {
                entry_href
            };
            responses.push_str(&build_response_xml(&display_href, &entry.path()));
            if depth == "infinity" && is_dir {
                append_child_responses(&entry.path(), &entry_relative, "infinity", href_prefix, responses);
            }
        }
    }
}

fn build_response_xml(href: &str, path: &Path) -> String {
    let is_dir = path.is_dir();
    let content_type = if is_dir {
        "httpd/unix-directory"
    } else {
        "application/octet-stream"
    };
    let resource_type = if is_dir {
        "<D:collection/>"
    } else {
        ""
    };
    let last_modified = format_http_date(path);
    let content_length = path.metadata().map(|m| m.len()).unwrap_or(0);

    format!(
        "<D:response>\
         <D:href>{}</D:href>\
         <D:propstat>\
         <D:prop>\
         <D:getcontenttype>{}</D:getcontenttype>\
         <D:resourcetype>{}</D:resourcetype>\
         <D:getlastmodified>{}</D:getlastmodified>\
         <D:getcontentlength>{}</D:getcontentlength>\
         </D:prop>\
         <D:status>HTTP/1.1 200 OK</D:status>\
         </D:propstat>\
         </D:response>",
        xml_escape(href),
        content_type,
        resource_type,
        xml_escape(&last_modified),
        content_length
    )
}

fn parse_destination(
    req: &HttpRequest,
    dav_path: &str,
    base: &Path,
) -> Result<PathBuf, HttpResponse> {
    let dest_header = match req
        .headers()
        .get("Destination")
        .and_then(|v| v.to_str().ok())
    {
        Some(d) => d,
        None => return Err(HttpResponse::BadRequest().finish()),
    };
    let dest_path = if dest_header.starts_with("http://") || dest_header.starts_with("https://") {
        let without_scheme = if let Some(pos) = dest_header.find("://") {
            &dest_header[pos + 3..]
        } else {
            dest_header
        };
        if let Some(slash_pos) = without_scheme.find('/') {
            without_scheme[slash_pos..].to_string()
        } else {
            "/".to_string()
        }
    } else {
        dest_header.to_string()
    };
    let dest_path = match decode(&dest_path) {
        Ok(p) => p.into_owned(),
        Err(_) => return Err(HttpResponse::BadRequest().finish()),
    };
    let dest_after_dav = normalize_dav_path(&dest_path);
    let dest_relative = if dav_path.is_empty() {
        dest_after_dav.to_string()
    } else {
        match dest_after_dav.strip_prefix(&format!("{}/", dav_path)) {
            Some(rest) => rest.to_string(),
            None => return Err(HttpResponse::BadRequest().finish()),
        }
    };
    let dest_target = base.join(dest_relative.trim_start_matches('/'));
    if !crate::handlers::is_path_under_root(&dest_target, base) {
        return Err(HttpResponse::BadRequest().finish());
    }
    Ok(dest_target)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            copy_dir_recursive(&src_path, &dst_path)?;
        }
    } else {
        std::fs::copy(src, dst)?;
    }
    Ok(())
}

fn safe_canonicalize(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        path.canonicalize().ok()
    } else {
        let parent = path.parent()?;
        let file_name = path.file_name()?;
        Some(parent.canonicalize().ok()?.join(file_name))
    }
}

fn find_backup_path(path: &Path) -> PathBuf {
    let mut i = 0u32;
    loop {
        let suffix = if i == 0 {
            ".davbak".to_string()
        } else {
            format!(".davbak.{}", i)
        };
        let backup = {
            let mut s = path.as_os_str().to_owned();
            s.push(suffix);
            PathBuf::from(s)
        };
        if !backup.exists() {
            return backup;
        }
        i += 1;
    }
}

pub async fn dav_handler(
    req: HttpRequest,
    body: web::Bytes,
    app_state: web::Data<AppState>,
) -> HttpResponse {
    if rand::random::<f32>() < 0.01 {
        cleanup_old_nonces();
    }
    let path = req.path();

    let ctx = match try_authenticate(&req, &app_state, path) {
        Ok(ctx) => ctx,
        Err(resp) => return resp,
    };

    let root = Path::new(&ctx.auth.root_path);
    let access_path_stripped = ctx.auth.config.access_path.trim_start_matches('/');
    let access_path = Path::new(access_path_stripped);
    let base = if access_path_stripped.is_empty() {
        root.to_path_buf()
    } else {
        root.join(access_path)
    };
    let relative_stripped = ctx.relative.trim_start_matches('/');
    let target = if relative_stripped.is_empty() {
        base.clone()
    } else {
        base.join(relative_stripped)
    };

    if !crate::handlers::is_path_under_root(&target, &base) {
        return HttpResponse::NotFound().finish();
    }

    let method = req.method().as_str();
    match method {
        "OPTIONS" => HttpResponse::Ok()
            .insert_header(("Allow", "OPTIONS, HEAD, PROPFIND, GET, PUT, MKCOL, COPY, MOVE, DELETE"))
            .insert_header(("DAV", "1"))
            .finish(),
        "PROPFIND" => {
            if !check_permission(&ctx.auth.config, "read") {
                return HttpResponse::Forbidden().finish();
            }
            if !target.exists() {
                if ctx.relative.is_empty() || ctx.relative == "/" {
                    let _ = std::fs::create_dir_all(&target);
                } else {
                    return HttpResponse::NotFound().finish();
                }
            }
            let depth = req
                .headers()
                .get("Depth")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("1");
            if depth == "infinity" {
                return HttpResponse::Forbidden().finish();
            }
            let href_prefix = if ctx.dav_path.is_empty() {
                "/dav/".to_string()
            } else {
                format!("/dav/{}/", ctx.dav_path)
            };
            let xml_body = build_propfind_xml(&base, &ctx.relative, depth, &href_prefix);
            HttpResponse::MultiStatus()
                .content_type("application/xml; charset=utf-8")
                .body(xml_body)
        }
        "GET" | "HEAD" => {
            if !check_permission(&ctx.auth.config, "read") {
                return HttpResponse::Forbidden().finish();
            }
            if !target.exists() {
                return HttpResponse::NotFound().finish();
            }
            if target.is_dir() {
                return HttpResponse::NotFound().finish();
            }
            match actix_files::NamedFile::open(&target) {
                Ok(file) => file.into_response(&req),
                Err(_) => HttpResponse::NotFound().finish(),
            }
        }
        "PUT" => {
            if !check_permission(&ctx.auth.config, "write") {
                return HttpResponse::Forbidden().finish();
            }
            if !target.file_name().and_then(|n| n.to_str()).map(|n| crate::handlers::is_safe_name(n)).unwrap_or(false) {
                return HttpResponse::BadRequest().finish();
            }
            if target.file_name().and_then(|n| n.to_str()) == Some(".notebook.sig") {
                return HttpResponse::BadRequest().finish();
            }
            if target.is_dir() {
                return HttpResponse::Conflict().finish();
            }
            if let Some(parent) = target.parent() {
                if !parent.exists() {
                    return HttpResponse::Conflict().finish();
                }
            }
            let is_new = !target.exists();
            match std::fs::write(&target, &body) {
                Ok(_) => {
                    if target.extension().and_then(|e| e.to_str()) == Some("md") {
                        let relative = target.strip_prefix(root)
                            .ok()
                            .and_then(|p| p.to_str())
                            .unwrap_or("")
                            .to_string();
                        crate::handlers::file::cleanup_search_index_on_restore(
                            &relative, &ctx.auth.config.user_id, &app_state,
                        );
                    }
                    if is_new {
                        HttpResponse::Created().finish()
                    } else {
                        HttpResponse::NoContent().finish()
                    }
                }
                Err(e) => {
                    error_logger::log_error("DAV_PUT", &e.to_string());
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
        "MKCOL" => {
            if !check_permission(&ctx.auth.config, "write") {
                return HttpResponse::Forbidden().finish();
            }
            if !target.file_name().and_then(|n| n.to_str()).map(|n| crate::handlers::is_safe_name(n)).unwrap_or(false) {
                return HttpResponse::BadRequest().finish();
            }
            if target.exists() {
                return HttpResponse::MethodNotAllowed().finish();
            }
            if let Some(parent) = target.parent() {
                if !parent.exists() {
                    return HttpResponse::Conflict().finish();
                }
            }
            match std::fs::create_dir(&target) {
                Ok(_) => HttpResponse::Created().finish(),
                Err(e) => {
                    error_logger::log_error("DAV_MKCOL", &e.to_string());
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
        "DELETE" => {
            if !check_permission(&ctx.auth.config, "delete") {
                return HttpResponse::Forbidden().finish();
            }
            if !target.exists() {
                return HttpResponse::NotFound().finish();
            }

            let relative_path = target.strip_prefix(root)
                .ok()
                .and_then(|p| p.to_str())
                .unwrap_or("")
                .to_string();

            let recycled = {
                if let Ok(Some(user)) = app_state.user_model.get_user_full(&ctx.auth.config.user_id) {
                    let recycle_bin_path = user.recycle_bin_path.as_deref().filter(|p| !p.is_empty());
                    match crate::handlers::file::delete_file_with_recycle(
                        &target,
                        &relative_path,
                        &ctx.auth.config.user_id,
                        recycle_bin_path,
                        &app_state,
                    ) {
                        Ok(()) => true,
                        Err(_) => false,
                    }
                } else {
                    false
                }
            };

            if !recycled {
                let result = if target.is_dir() {
                    std::fs::remove_dir_all(&target)
                } else {
                    std::fs::remove_file(&target)
                };
                match result {
                    Ok(_) => {
                        crate::handlers::file::cleanup_search_index_on_delete(
                            &relative_path, &ctx.auth.config.user_id, &app_state,
                        );
                        HttpResponse::NoContent().finish()
                    }
                    Err(e) => {
                        error_logger::log_error("DAV_DELETE", &e.to_string());
                        HttpResponse::InternalServerError().finish()
                    }
                }
            } else {
                HttpResponse::NoContent().finish()
            }
        }
        "MOVE" => {
            if !check_permission(&ctx.auth.config, "write") {
                return HttpResponse::Forbidden().finish();
            }
            let dest_target = match parse_destination(&req, &ctx.dav_path, &base) {
                Ok(p) => p,
                Err(resp) => return resp,
            };
            if !target.exists() {
                return HttpResponse::NotFound().finish();
            }
            if let (Some(sc), Some(dc)) = (safe_canonicalize(&target), safe_canonicalize(&dest_target)) {
                if sc == dc {
                    return HttpResponse::Forbidden().finish();
                }
            }
            if let (Some(bc), Some(dc)) = (safe_canonicalize(&base), safe_canonicalize(&dest_target)) {
                if dc == bc {
                    return HttpResponse::BadRequest().finish();
                }
            }
            if target.is_dir() {
                if let (Some(sc), Some(dc)) = (safe_canonicalize(&target), safe_canonicalize(&dest_target)) {
                    if dc.starts_with(&sc) {
                        return HttpResponse::Forbidden().finish();
                    }
                }
            }
            if target.is_dir() && dest_target.exists() && !dest_target.is_dir() {
                return HttpResponse::Conflict().finish();
            }
            if !dest_target.file_name().and_then(|n| n.to_str()).map(|n| crate::handlers::is_safe_name(n)).unwrap_or(false) {
                return HttpResponse::BadRequest().finish();
            }
            if let Some(parent) = dest_target.parent() {
                if !parent.exists() {
                    return HttpResponse::Conflict().finish();
                }
            }
            let overwrite = req
                .headers()
                .get("Overwrite")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("T");
            let dest_existed = dest_target.exists();
            if dest_existed && overwrite == "F" {
                return HttpResponse::PreconditionFailed().finish();
            }
            let backup_path = if dest_existed {
                let backup = find_backup_path(&dest_target);
                if let Err(e) = std::fs::rename(&dest_target, &backup) {
                    error_logger::log_error("DAV_MOVE", &e.to_string());
                    return HttpResponse::InternalServerError().finish();
                }
                Some(backup)
            } else {
                None
            };
            let old_relative = target.strip_prefix(root)
                .ok()
                .and_then(|p| p.to_str())
                .unwrap_or("")
                .to_string();
            let new_relative = dest_target.strip_prefix(root)
                .ok()
                .and_then(|p| p.to_str())
                .unwrap_or("")
                .to_string();
            match move_recursive(&target, &dest_target) {
                Ok(_) => {
                    if let Some(backup) = &backup_path {
                        let _ = if backup.is_dir() {
                            std::fs::remove_dir_all(backup)
                        } else {
                            std::fs::remove_file(backup)
                        };
                    }
                    crate::handlers::file::cleanup_search_index_on_move(
                        &old_relative, &new_relative, &ctx.auth.config.user_id, &app_state,
                    );
                    if dest_existed {
                        HttpResponse::NoContent().finish()
                    } else {
                        HttpResponse::Created().finish()
                    }
                }
                Err(e) => {
                    if let Some(backup) = &backup_path {
                        if dest_target.is_dir() {
                            let _ = std::fs::remove_dir_all(&dest_target);
                        } else if dest_target.exists() {
                            let _ = std::fs::remove_file(&dest_target);
                        }
                        let _ = std::fs::rename(backup, &dest_target);
                    }
                    error_logger::log_error("DAV_MOVE", &e.to_string());
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
        "COPY" => {
            if !check_permission(&ctx.auth.config, "write") {
                return HttpResponse::Forbidden().finish();
            }
            if !target.exists() {
                return HttpResponse::NotFound().finish();
            }
            let dest_target = match parse_destination(&req, &ctx.dav_path, &base) {
                Ok(p) => p,
                Err(resp) => return resp,
            };
            if let (Some(sc), Some(dc)) = (safe_canonicalize(&target), safe_canonicalize(&dest_target)) {
                if sc == dc {
                    return HttpResponse::Forbidden().finish();
                }
            }
            if target.is_dir() {
                if let (Some(sc), Some(dc)) = (safe_canonicalize(&target), safe_canonicalize(&dest_target)) {
                    if dc.starts_with(&sc) {
                        return HttpResponse::Forbidden().finish();
                    }
                }
            }
            if !dest_target.file_name().and_then(|n| n.to_str()).map(|n| crate::handlers::is_safe_name(n)).unwrap_or(false) {
                return HttpResponse::BadRequest().finish();
            }
            if let Some(parent) = dest_target.parent() {
                if !parent.exists() {
                    return HttpResponse::Conflict().finish();
                }
            }
            if target.is_dir() && dest_target.exists() && !dest_target.is_dir() {
                return HttpResponse::Conflict().finish();
            }
            let overwrite = req
                .headers()
                .get("Overwrite")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("T");
            let dest_existed = dest_target.exists();
            if dest_existed && overwrite == "F" {
                return HttpResponse::PreconditionFailed().finish();
            }
            let backup_path = if dest_existed {
                let backup = find_backup_path(&dest_target);
                if let Err(e) = std::fs::rename(&dest_target, &backup) {
                    error_logger::log_error("DAV_COPY", &e.to_string());
                    return HttpResponse::InternalServerError().finish();
                }
                Some(backup)
            } else {
                None
            };
            let result = copy_dir_recursive(&target, &dest_target);
            if result.is_ok() {
                if let Some(backup) = &backup_path {
                    let _ = if backup.is_dir() {
                        std::fs::remove_dir_all(backup)
                    } else {
                        std::fs::remove_file(backup)
                    };
                }
                if let Some(new_relative) = dest_target.strip_prefix(root).ok().and_then(|p| p.to_str()) {
                    if !new_relative.is_empty() {
                        crate::handlers::file::cleanup_search_index_on_restore(
                            new_relative, &ctx.auth.config.user_id, &app_state,
                        );
                    }
                }
            }
            match result {
                Ok(_) => {
                    if dest_existed {
                        HttpResponse::NoContent().finish()
                    } else {
                        HttpResponse::Created().finish()
                    }
                }
                Err(e) => {
                    if let Some(backup) = &backup_path {
                        if dest_target.is_dir() {
                            let _ = std::fs::remove_dir_all(&dest_target);
                        } else if dest_target.exists() {
                            let _ = std::fs::remove_file(&dest_target);
                        }
                        let _ = std::fs::rename(backup, &dest_target);
                    }
                    error_logger::log_error("DAV_COPY", &e.to_string());
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
        _ => HttpResponse::MethodNotAllowed().finish(),
    }
}
