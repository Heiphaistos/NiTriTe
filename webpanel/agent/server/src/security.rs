//! Controles d'acces de l'agent.
//!
//! L'agent execute des commandes d'administration : une page web quelconque
//! ouverte dans le navigateur ne doit pas pouvoir l'appeler.
//! - Jeton : 256 bits aleatoires par session, transmis a l'onglet via
//!   l'URL ouverte par l'agent (fragment `#t=`, jamais envoye au serveur par
//!   le navigateur). L'onglet l'echange aussitot contre un cookie de session
//!   `HttpOnly; SameSite=Strict` : les autres onglets (liens ouverts dans un
//!   nouvel onglet, favori, F5) fonctionnent alors sans jeton dans l'URL, et
//!   un script de la page ne peut pas lire le cookie.
//! - Host : refuse tout nom d'hote autre que 127.0.0.1/localhost (un site
//!   malveillant qui ferait pointer son domaine sur 127.0.0.1 — « DNS
//!   rebinding » — enverrait `Host: son-domaine`).
//! - Origin : une requete venant d'une autre origine est refusee. Avec
//!   SameSite=Strict, c'est la double protection contre le CSRF.
//! - En-tetes : CSP stricte (aucun script en ligne hors celui d'amorce,
//!   autorise par son empreinte), pas d'integration en iframe (clickjacking
//!   sur un panneau d'administration), pas de Referer.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use rand::RngCore;
use serde::Deserialize;

use crate::AppCtx;

pub const COOKIE_NAME: &str = "nitrite_session";

pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Comparaison en temps constant (pas de fuite de longueur de prefixe commun).
pub fn token_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Hotes acceptes en mode local.
pub fn host_allowed(host: &str, port: u16) -> bool {
    let host = host.trim().to_ascii_lowercase();
    ["127.0.0.1", "localhost", "[::1]"]
        .iter()
        .any(|h| host == format!("{h}:{port}") || (port == 80 && host == *h))
}

/// Valeur du cookie de session dans l'en-tete `Cookie`.
pub fn session_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|kv| kv.trim().strip_prefix(&format!("{COOKIE_NAME}=")).map(str::to_string))
}

fn forbidden(msg: &'static str) -> Response {
    (StatusCode::FORBIDDEN, msg).into_response()
}

pub async fn check_host_and_origin(State(ctx): State<Arc<AppCtx>>, req: Request, next: Next) -> Response {
    let host = req.headers().get(header::HOST).and_then(|h| h.to_str().ok()).unwrap_or("").to_string();
    if !ctx.lan && !host_allowed(&host, ctx.port) {
        return forbidden("Hote refuse");
    }
    if let Some(origin) = req.headers().get(header::ORIGIN).and_then(|o| o.to_str().ok()) {
        let expected_http = format!("http://{host}");
        if !origin.eq_ignore_ascii_case(&expected_http) {
            return forbidden("Origine refusee");
        }
    }
    next.run(req).await
}

pub async fn require_token(State(ctx): State<Arc<AppCtx>>, req: Request, next: Next) -> Response {
    let from_header = req.headers().get("x-nitrite-token").and_then(|v| v.to_str().ok()).map(str::to_string);
    // Le WebSocket du navigateur ne peut pas poser d'en-tete : jeton en requete
    // (ou cookie, envoye automatiquement pour la meme origine).
    let from_query = req.uri().query().and_then(|q| {
        q.split('&').find_map(|kv| kv.strip_prefix("token=").map(str::to_string))
    });
    let from_cookie = session_cookie(req.headers());
    let ok = [from_header, from_query, from_cookie]
        .into_iter()
        .flatten()
        .any(|t| token_eq(&t, &ctx.token));
    if !ok {
        return (StatusCode::UNAUTHORIZED, "Jeton manquant ou invalide — relancez NiTriTe Agent pour rouvrir le panneau").into_response();
    }
    next.run(req).await
}

#[derive(Deserialize)]
pub struct SessionRequest {
    token: String,
}

/// Echange le jeton de l'URL contre le cookie de session (hors middleware de
/// jeton : c'est justement ici qu'on le presente).
pub async fn open_session(State(ctx): State<Arc<AppCtx>>, Json(req): Json<SessionRequest>) -> Response {
    ctx.touch();
    if !token_eq(&req.token, &ctx.token) {
        return (StatusCode::UNAUTHORIZED, "Jeton invalide").into_response();
    }
    // Pas de `Secure` : http://127.0.0.1 n'est pas en HTTPS. Pas d'expiration :
    // cookie de session du navigateur, et le jeton change a chaque lancement.
    let cookie = format!("{COOKIE_NAME}={}; HttpOnly; SameSite=Strict; Path=/", ctx.token);
    ([(header::SET_COOKIE, cookie)], StatusCode::NO_CONTENT).into_response()
}

/// Politique de securite du contenu. `script_hashes` : empreintes des scripts
/// en ligne d'index.html (le script d'amorce qui capture le jeton).
pub fn content_security_policy(script_hashes: &[String]) -> String {
    let scripts = script_hashes.iter().map(|h| format!(" '{h}'")).collect::<String>();
    [
        "default-src 'self'".to_string(),
        format!("script-src 'self'{scripts}"),
        // Vue pose des styles en ligne (:style) ; polices Google de l'interface.
        "style-src 'self' 'unsafe-inline' https://fonts.googleapis.com".to_string(),
        "font-src 'self' data: https://fonts.gstatic.com".to_string(),
        "img-src 'self' data: blob: https:".to_string(),
        "media-src 'self' blob:".to_string(),
        // Meme origine (API + WebSocket de l'agent, couverts par 'self' en CSP 3)
        // + services appeles par l'interface
        // (verification VirusTotal du Hash Checker, test de debit Cloudflare).
        "connect-src 'self' https://www.virustotal.com https://speed.cloudflare.com".to_string(),
        "object-src 'none'".to_string(),
        "base-uri 'none'".to_string(),
        "form-action 'self'".to_string(),
        "frame-ancestors 'none'".to_string(),
    ]
    .join("; ")
}

pub async fn security_headers(State(ctx): State<Arc<AppCtx>>, req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&ctx.csp) {
        h.insert(header::CONTENT_SECURITY_POLICY, v);
    }
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    h.insert("cross-origin-opener-policy", HeaderValue::from_static("same-origin"));
    h.insert("cross-origin-resource-policy", HeaderValue::from_static("same-origin"));
    h.insert("permissions-policy", HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=()"));
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_random_and_long() {
        let a = new_token();
        let b = new_token();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }

    #[test]
    fn token_comparison() {
        assert!(token_eq("abc", "abc"));
        assert!(!token_eq("abc", "abd"));
        assert!(!token_eq("abc", "abcd"));
    }

    #[test]
    fn only_loopback_hosts() {
        assert!(host_allowed("127.0.0.1:7878", 7878));
        assert!(host_allowed("LOCALHOST:7878", 7878));
        assert!(!host_allowed("127.0.0.1:7879", 7878));
        assert!(!host_allowed("evil.example:7878", 7878));
        assert!(!host_allowed("", 7878));
    }

    #[test]
    fn reads_session_cookie_among_others() {
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, HeaderValue::from_static("a=1; nitrite_session=abc; b=2"));
        assert_eq!(session_cookie(&h).as_deref(), Some("abc"));
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, HeaderValue::from_static("xnitrite_session=abc"));
        assert_eq!(session_cookie(&h), None);
    }

    #[test]
    fn csp_forbids_inline_scripts_except_hashed_and_framing() {
        let csp = content_security_policy(&["sha256-AAA=".into()]);
        assert!(csp.contains("script-src 'self' 'sha256-AAA='"));
        assert!(!csp.contains("script-src 'self' 'unsafe-inline'"));
        assert!(csp.contains("frame-ancestors 'none'"));
        assert!(csp.contains("object-src 'none'"));
    }
}
