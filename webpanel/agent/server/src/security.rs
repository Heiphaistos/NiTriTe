//! Controles d'acces de l'agent.
//!
//! L'agent execute des commandes d'administration : une page web quelconque
//! ouverte dans le navigateur ne doit pas pouvoir l'appeler.
//! - Jeton : 256 bits aleatoires par session, transmis a l'onglet via
//!   l'URL ouverte par l'agent (fragment `#t=`, jamais envoye au serveur par
//!   le navigateur), puis en en-tete `X-Nitrite-Token`.
//! - Host : refuse tout nom d'hote autre que 127.0.0.1/localhost (un site
//!   malveillant qui ferait pointer son domaine sur 127.0.0.1 — « DNS
//!   rebinding » — enverrait `Host: son-domaine`).
//! - Origin : une requete venant d'une autre origine est refusee.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use rand::RngCore;

use crate::AppCtx;

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
    // Le WebSocket du navigateur ne peut pas poser d'en-tete : jeton en requete.
    let from_query = req.uri().query().and_then(|q| {
        q.split('&').find_map(|kv| kv.strip_prefix("token=").map(str::to_string))
    });
    let ok = from_header.or(from_query).is_some_and(|t| token_eq(&t, &ctx.token));
    if !ok {
        return (StatusCode::UNAUTHORIZED, "Jeton manquant ou invalide — relancez NiTriTe Agent pour rouvrir le panneau").into_response();
    }
    next.run(req).await
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
}
