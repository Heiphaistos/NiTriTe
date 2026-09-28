use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub winget_id: Option<String>,
    pub choco_id: Option<String>,
    pub url: Option<String>,
    pub icon: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEntry {
    pub name: String,
    pub description: String,
    pub command: String,
    pub is_url: bool,
    pub section: String,
    pub icon: String,
    /// Lance l'outil eleve (invite UAC) : DISM, SFC, CHKDSK, resets reseau…
    #[serde(default)]
    pub requires_admin: bool,
    /// "cmd" (defaut) ou "powershell".
    #[serde(default)]
    pub shell: String,
}

static APPS: LazyLock<Vec<AppEntry>> = LazyLock::new(|| {
    let json = include_str!("../../data/programs.json");
    serde_json::from_str(json).unwrap_or_else(|e| {
        tracing::error!("Erreur chargement programs.json: {}", e);
        Vec::new()
    })
});

static TOOLS: LazyLock<Vec<ToolEntry>> = LazyLock::new(|| {
    let json = include_str!("../../data/tools.json");
    serde_json::from_str(json).unwrap_or_else(|e| {
        tracing::error!("Erreur chargement tools.json: {}", e);
        Vec::new()
    })
});

pub fn get_default_apps() -> Vec<AppEntry> {
    APPS.clone()
}

pub fn get_tools() -> Vec<ToolEntry> {
    TOOLS.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Les deux catalogues sont charges via `unwrap_or_else(Vec::new)` : un JSON
    // invalide ne casse pas le build, il vide silencieusement la page. Ces
    // tests transforment cette panne silencieuse en echec de CI.
    #[test]
    fn programs_catalog_parses() {
        assert!(get_default_apps().len() > 700);
    }

    #[test]
    fn tools_catalog_parses_with_unique_keys() {
        let tools = get_tools();
        assert!(tools.len() > 500);
        let mut keys = std::collections::HashSet::new();
        for t in &tools {
            assert!(keys.insert((t.section.clone(), t.name.clone())), "outil en double : {} / {}", t.section, t.name);
            assert!(t.shell.is_empty() || t.shell == "powershell", "shell inconnu pour {}", t.name);
        }
    }

    #[test]
    fn cmd_tools_contain_no_powershell_syntax() {
        let cmdlet = regex::Regex::new(r"(^|[\s|;(])(get|set|restart|foreach)-[a-z]").unwrap();
        for t in get_tools().iter().filter(|t| !t.is_url && t.shell.is_empty()) {
            let c = t.command.to_lowercase();
            assert!(!cmdlet.is_match(&c) && !c.starts_with("wmic"),
                "{} : commande PowerShell/wmic executee par cmd : {}", t.name, t.command);
        }
    }
}
