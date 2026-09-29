//! Lancement des outils du catalogue `tools.json` (page « Outils Système »).
//!
//! `execute_tool` recoit une ligne de commande arbitraire depuis la webview et
//! refuse donc tout metacaractere shell (`&`, `|`, `%`…) : toutes les commandes
//! chainees du catalogue (reset reseau, reset Windows Update, reparation du
//! demarrage, nettoyage du dossier Temp…) etaient refusees, et rien n'etait
//! jamais lance en administrateur (DISM, SFC, CHKDSK echouaient en « acces
//! refuse »).
//!
//! Ici la webview ne transmet qu'un NOM d'outil : la commande vient du
//! catalogue compile dans le binaire (donc de confiance), est ecrite dans un
//! script temporaire, puis executee dans une fenetre visible qui reste ouverte.
//! Si l'outil exige les droits admin, Windows affiche l'invite UAC.

use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::error::NiTriTeError;
use crate::installer::manager::ToolEntry;

/// Schemas ouverts par le shell Windows (pas de ligne de commande).
const URL_SCHEMES: &[&str] = &["http://", "https://", "ms-settings:", "ms-windows-store:", "windowsdefender:"];

pub fn is_openable_url(target: &str) -> bool {
    let t = target.trim().to_lowercase();
    URL_SCHEMES.iter().any(|s| t.starts_with(s))
}

#[derive(Debug, PartialEq, Eq)]
pub enum ScriptKind {
    Cmd,
    PowerShell,
}

/// Contenu du script a executer. Le titre est reduit aux caracteres surs : il
/// finit dans `title` (cmd) ou `$Host.UI.RawUI.WindowTitle` (PowerShell).
pub fn build_script(tool: &ToolEntry) -> (ScriptKind, String) {
    let title: String = tool
        .name
        .chars()
        .filter(|c| c.is_alphanumeric() || " -_./()+".contains(*c))
        .take(60)
        .collect();
    if tool.shell.eq_ignore_ascii_case("powershell") {
        let body = format!(
            "$Host.UI.RawUI.WindowTitle = 'NiTriTe - {title}'\r\n\
             Write-Host '[NiTriTe] {title}' -ForegroundColor Cyan\r\n\
             {cmd}\r\n\
             Write-Host ''\r\n\
             Write-Host '[NiTriTe] Commande terminee.' -ForegroundColor Green\r\n",
            cmd = tool.command
        );
        (ScriptKind::PowerShell, body)
    } else {
        let body = format!(
            "@echo off\r\n\
             chcp 65001 >nul\r\n\
             title NiTriTe - {title}\r\n\
             echo [NiTriTe] {title}\r\n\
             echo.\r\n\
             {cmd}\r\n\
             echo.\r\n\
             echo [NiTriTe] Commande terminee (code %ERRORLEVEL%).\r\n",
            cmd = tool.command
        );
        (ScriptKind::Cmd, body)
    }
}

fn script_path(tool: &ToolEntry, kind: &ScriptKind) -> PathBuf {
    let slug: String = tool
        .name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let ext = if *kind == ScriptKind::PowerShell { "ps1" } else { "cmd" };
    std::env::temp_dir()
        .join("nitrite-tools")
        .join(format!("{}-{}.{}", slug.trim_matches('-'), std::process::id(), ext))
}

/// PowerShell 5.1 lit un .ps1 sans BOM en ANSI : BOM UTF-8 pour les accents.
fn write_script(path: &Path, kind: &ScriptKind, content: &str) -> Result<(), NiTriTeError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut bytes = Vec::with_capacity(content.len() + 3);
    if *kind == ScriptKind::PowerShell {
        bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    }
    bytes.extend_from_slice(content.as_bytes());
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Chaine PowerShell entre apostrophes (les apostrophes sont doublees).
fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Ligne `Start-Process` qui ouvre le script dans une nouvelle fenetre,
/// elevee (`-Verb RunAs`) si demande.
pub fn start_process_line(path: &Path, kind: &ScriptKind, elevated: bool) -> String {
    let p = path.to_string_lossy();
    let (exe, args) = match kind {
        ScriptKind::Cmd => ("cmd.exe", vec!["/K".to_string(), format!("\"{p}\"")]),
        ScriptKind::PowerShell => (
            "powershell.exe",
            vec![
                "-NoExit".into(), "-NoProfile".into(), "-ExecutionPolicy".into(), "Bypass".into(),
                "-File".into(), format!("\"{p}\""),
            ],
        ),
    };
    let arg_list = args.iter().map(|a| ps_quote(a)).collect::<Vec<_>>().join(",");
    format!(
        "Start-Process -FilePath {} -ArgumentList {}{} -ErrorAction Stop",
        ps_quote(exe),
        arg_list,
        if elevated { " -Verb RunAs" } else { "" }
    )
}

/// Lance l'outil. `force_admin` permet d'elever un outil qui ne l'exige pas.
pub fn launch(tool: &ToolEntry, force_admin: bool) -> Result<(), NiTriTeError> {
    if tool.is_url || is_openable_url(&tool.command) {
        if !is_openable_url(&tool.command) {
            return Err(NiTriTeError::CommandDenied(format!("Schéma non autorisé: {}", tool.command)));
        }
        return open::that(tool.command.trim()).map_err(|e| NiTriTeError::System(e.to_string()));
    }
    let (kind, content) = build_script(tool);
    let path = script_path(tool, &kind);
    write_script(&path, &kind, &content)?;
    let elevated = tool.requires_admin || force_admin;
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &start_process_line(&path, &kind, elevated)])
        .creation_flags(0x08000000)
        .output()?;
    if out.status.success() {
        Ok(())
    } else if elevated {
        Err(NiTriTeError::System("Élévation administrateur refusée ou annulée (UAC)".into()))
    } else {
        Err(NiTriTeError::System(String::from_utf8_lossy(&out.stderr).trim().chars().take(200).collect()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(cmd: &str, shell: &str) -> ToolEntry {
        ToolEntry {
            name: "Reset Network".into(),
            description: String::new(),
            command: cmd.into(),
            is_url: false,
            section: "Réseau & Internet".into(),
            icon: String::new(),
            requires_admin: true,
            shell: shell.into(),
        }
    }

    #[test]
    fn cmd_script_keeps_chained_commands_verbatim() {
        let (kind, body) = build_script(&tool("netsh int ip reset && netsh winsock reset", ""));
        assert_eq!(kind, ScriptKind::Cmd);
        assert!(body.contains("\r\nnetsh int ip reset && netsh winsock reset\r\n"));
        assert!(body.starts_with("@echo off"));
    }

    #[test]
    fn powershell_script_selected_by_shell_field() {
        let (kind, body) = build_script(&tool("Get-PnpDevice | Format-Table", "powershell"));
        assert_eq!(kind, ScriptKind::PowerShell);
        assert!(body.contains("Get-PnpDevice | Format-Table"));
    }

    #[test]
    fn title_is_sanitized() {
        let mut t = tool("ver", "");
        t.name = "Evil & calc | x".into();
        let (_, body) = build_script(&t);
        assert!(body.contains("title NiTriTe - Evil  calc  x\r\n"));
    }

    #[test]
    fn start_process_line_quotes_path_and_elevates() {
        let line = start_process_line(Path::new(r"C:\Users\O'Brien\Temp\x.cmd"), &ScriptKind::Cmd, true);
        assert!(line.contains(r#"'"C:\Users\O''Brien\Temp\x.cmd"'"#), "{line}");
        assert!(line.ends_with("-Verb RunAs -ErrorAction Stop"));
        let line = start_process_line(Path::new(r"C:\t\x.ps1"), &ScriptKind::PowerShell, false);
        assert!(line.starts_with("Start-Process -FilePath 'powershell.exe'"));
        assert!(!line.contains("RunAs"));
    }

    #[test]
    fn url_schemes() {
        assert!(is_openable_url("ms-settings:windowsupdate"));
        assert!(is_openable_url("windowsdefender:"));
        assert!(is_openable_url("https://example.com"));
        assert!(!is_openable_url("file:///C:/Windows"));
        assert!(!is_openable_url("javascript:alert(1)"));
    }
}
