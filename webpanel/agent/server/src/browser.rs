//! Ouverture du panneau : navigateur par defaut, ou fenetre « application »
//! de Microsoft Edge (`--app`) — une fenetre sans onglets ni barre d'adresse,
//! comme une application, sans embarquer de WebView dans l'agent.

use std::path::PathBuf;

/// Emplacements standard d'Edge (present sur tout Windows 10/11).
fn edge_candidates() -> Vec<PathBuf> {
    ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"]
        .iter()
        .filter_map(|v| std::env::var_os(v))
        .map(|base| PathBuf::from(base).join("Microsoft").join("Edge").join("Application").join("msedge.exe"))
        .collect()
}

pub fn app_window_args(url: &str) -> Vec<String> {
    vec![format!("--app={url}"), "--window-size=1440,900".into(), "--no-first-run".into()]
}

pub fn open(url: &str, app_window: bool) {
    if app_window {
        if let Some(edge) = edge_candidates().into_iter().find(|p| p.exists()) {
            if std::process::Command::new(edge).args(app_window_args(url)).spawn().is_ok() {
                return;
            }
        }
        crate::agentlog::line("Edge introuvable : ouverture dans le navigateur par defaut");
    }
    if let Err(e) = open::that(url) {
        crate::agentlog::line(&format!("Impossible d'ouvrir le navigateur ({e}). Ouvrez manuellement : {url}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_window_args_carry_the_url() {
        let a = app_window_args("http://127.0.0.1:7878/#t=abc");
        assert_eq!(a[0], "--app=http://127.0.0.1:7878/#t=abc");
    }
}
