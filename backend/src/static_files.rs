use actix_web::{HttpRequest, HttpResponse};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../frontend/dist/"]
struct Asset;

fn mime_type(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "webp" => "image/webp",
        "map" => "application/json",
        _ => "application/octet-stream",
    }
}

pub async fn serve_static(req: HttpRequest) -> HttpResponse {
    let path = req.path().trim_start_matches('/');

    if !path.is_empty() {
        if let Some(file) = Asset::get(path) {
            return HttpResponse::Ok()
                .content_type(mime_type(path))
                .body(file.data.into_owned());
        }
    }

    if let Some(file) = Asset::get("index.html") {
        return HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(file.data.into_owned());
    }

    HttpResponse::NotFound().finish()
}
