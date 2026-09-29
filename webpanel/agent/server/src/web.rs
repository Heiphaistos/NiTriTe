//! Fichiers de l'interface (web/dist), embarques dans l'executable : l'agent
//! est un seul .exe, rien a installer a cote.

use axum::http::{header, StatusCode, Uri};
use axum::response::{Html, IntoResponse, Response};
use base64::Engine;
use rust_embed::RustEmbed;
use sha2::{Digest, Sha256};

#[derive(RustEmbed)]
#[folder = "../../web/dist"]
#[allow_missing = true]
struct Assets;

const MISSING_UI: &str = "<!doctype html><meta charset=utf-8><title>NiTriTe Agent</title>\
<body style=\"font-family:system-ui;background:#09090b;color:#fafafa;padding:40px\">\
<h1>NiTriTe Agent</h1><p>L'interface web n'a pas ete compilee dans cet executable.</p>\
<p>Depuis la racine du panneau : <code>npm ci &amp;&amp; npm run build</code>, puis recompilez l'agent.</p>";

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

/// Empreintes CSP (`sha256-...`) des scripts en ligne d'un document HTML.
pub fn inline_script_hashes(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find("<script") {
        let after = &rest[start..];
        let Some(tag_end) = after.find('>') else { break };
        let tag = &after[..tag_end];
        let body_start = &after[tag_end + 1..];
        let Some(close) = body_start.find("</script>") else { break };
        if !tag.contains(" src=") && !tag.contains(" src ") {
            let digest = Sha256::digest(body_start[..close].as_bytes());
            out.push(format!("sha256-{}", base64::engine::general_purpose::STANDARD.encode(digest)));
        }
        rest = &body_start[close + "</script>".len()..];
    }
    out
}

/// Empreintes des scripts en ligne de l'index embarque.
pub fn index_script_hashes() -> Vec<String> {
    Assets::get("index.html")
        .map(|f| inline_script_hashes(&String::from_utf8_lossy(&f.data)))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_only_inline_scripts() {
        let html = "<head><script>alert(1)</script><script type=\"module\" src=\"/a.js\"></script>\n<script>\nx()\n</script></head>";
        let h = inline_script_hashes(html);
        assert_eq!(h.len(), 2);
        // echo -n 'alert(1)' | openssl dgst -sha256 -binary | base64
        assert_eq!(h[0], "sha256-bhHHL3z2vDgxUt0W3dWQOrprscmda2Y5pLsLg4GF+pI=");
    }

    #[test]
    fn no_scripts_no_hashes() {
        assert!(inline_script_hashes("<p>rien</p>").is_empty());
    }
}
