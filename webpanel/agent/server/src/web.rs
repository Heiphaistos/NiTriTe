//! Fichiers de l'interface (web/dist), embarques dans l'executable : l'agent
//! est un seul .exe, rien a installer a cote.

use axum::http::{header, StatusCode, Uri};
use axum::response::{Html, IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../web/dist"]
#[allow_missing = true]
struct Assets;

const MISSING_UI: &str = "<!doctype html><meta charset=utf-8><title>NiTriTe Agent</title>\
<body style=\"font-family:system-ui;background:#09090b;color:#fafafa;padding:40px\">\
<h1>NiTriTe Agent</h1><p>L'interface web n'a pas ete compilee dans cet executable.</p>\
<p>Depuis le dossier <code>webpanel</code> : <code>npm ci &amp;&amp; npm run build</code>, puis recompilez l'agent.</p>";

pub async fn static_file(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if let Some(file) = Assets::get(path).filter(|_| !path.is_empty()) {
        let mime = file.metadata.mimetype().to_string();
        // Les ressources de Vite portent un hash dans leur nom : cache long.
        let cache = if path.starts_with("assets/") { "public, max-age=31536000, immutable" } else { "no-cache" };
        return ([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, cache.to_string())], file.data.into_owned()).into_response();
    }
    // Application monopage : toute autre route renvoie index.html (le routeur
    // Vue prend le relais : /master-install, /settings…).
    if path.contains('.') && !path.ends_with(".html") {
        return StatusCode::NOT_FOUND.into_response();
    }
    match Assets::get("index.html") {
        Some(index) => ([(header::CACHE_CONTROL, "no-cache")], Html(index.data.into_owned())).into_response(),
        None => Html(MISSING_UI).into_response(),
    }
}
