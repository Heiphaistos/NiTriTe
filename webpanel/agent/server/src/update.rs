//! Verification de mise a jour de l'agent : derniere release GitHub du depot
//! du panneau web. Informative uniquement (l'interface affiche un lien) :
//! l'agent ne telecharge ni n'execute rien tout seul.

use serde::{Deserialize, Serialize};

/// Depot des releases (surchargeable : NITRITE_AGENT_REPO=proprietaire/depot).
pub const DEFAULT_REPO: &str = "Heiphaistos/Nitrite-We-Panel-";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UpdateInfo {
    pub version: String,
    pub url: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

/// `Some` si `tag` (v1.2.3 ou 1.2.3) est strictement plus recent que `current`.
pub fn newer(current: &str, tag: &str) -> bool {
    let parse = |s: &str| semver::Version::parse(s.trim().trim_start_matches(['v', 'V'])).ok();
    matches!((parse(current), parse(tag)), (Some(c), Some(t)) if t > c)
}

pub async fn check(current: &str) -> Option<UpdateInfo> {
    let repo = std::env::var("NITRITE_AGENT_REPO").unwrap_or_else(|_| DEFAULT_REPO.to_string());
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let client = reqwest::Client::builder()
        .user_agent(concat!("NiTriTe-Agent/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;
    let rel: Release = client.get(url).send().await.ok()?.error_for_status().ok()?.json().await.ok()?;
    if rel.draft || rel.prerelease || !newer(current, &rel.tag_name) {
        return None;
    }
    Some(UpdateInfo { version: rel.tag_name.trim_start_matches(['v', 'V']).to_string(), url: rel.html_url })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(newer("1.0.0", "v1.0.1"));
        assert!(newer("1.0.0", "1.1.0"));
        assert!(!newer("1.1.0", "v1.0.9"));
        assert!(!newer("1.0.0", "v1.0.0"));
        assert!(!newer("1.0.0", "nightly"));
    }
}
