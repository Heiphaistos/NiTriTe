//! slmgr.rs — Orchestration de l'activation Windows via les outils natifs
//! (`slmgr.vbs` + CIM `SoftwareLicensingProduct`). Aucune valeur utilisateur n'est
//! concaténée dans une ligne de commande : tout passe en arguments séparés.
//!
//! Le STATUT est lu via CIM (`Get-CimInstance SoftwareLicensingProduct`) car ses
//! champs sont numériques et indépendants de la langue de Windows — contrairement
//! au texte de `slmgr /dlv` qui est localisé. Le texte brut de `slmgr /dlv` reste
//! exposé à l'UI pour transparence.

use serde::Serialize;

use crate::error::NiTriTeError;
use crate::maintenance::commands::{execute_system_command, CommandResult};

/// Chemin absolu de `slmgr.vbs` (via %SystemRoot%, repli C:\Windows).
fn slmgr_path() -> String {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
    format!("{root}\\System32\\slmgr.vbs")
}

/// Lance `cscript //Nologo slmgr.vbs <args>` avec un timeout. `cscript` écrit sur
/// stdout (sinon slmgr affiche une boîte de dialogue WScript bloquante).
pub fn run_slmgr(args: &[&str], timeout_secs: u64) -> Result<CommandResult, NiTriTeError> {
    let path = slmgr_path();
    let mut full: Vec<&str> = vec!["//Nologo", &path];
    full.extend_from_slice(args);
    execute_system_command("cscript", &full, timeout_secs)
}

/// Statut de licence structuré (langue-neutre) pour le produit Windows actif.
#[derive(Debug, Clone, Serialize, Default)]
pub struct LicenseStatus {
    pub name: String,
    pub description: String,
    /// Code CIM : 0=Non licencié, 1=Licencié, 2=Grâce OOB, 3=Grâce OOT,
    /// 4=Non authentique, 5=Notification, 6=Grâce étendue.
    pub license_status: u32,
    pub license_status_text: String,
    /// Minutes restantes avant expiration / fin de grâce (0 si permanent ou inconnu).
    pub grace_minutes: u64,
    /// Jours restants dérivés de `grace_minutes`.
    pub grace_days: u64,
    /// Canal : "Volume:GVLK", "Retail", "OEM:DM"...
    pub channel: String,
    /// 5 derniers caractères de la clé posée (vide si aucune).
    pub partial_key: String,
    pub activated: bool,
}

fn status_text(code: u32) -> &'static str {
    match code {
        0 => "Non licencié",
        1 => "Activé (licencié)",
        2 => "Période de grâce initiale (OOB)",
        3 => "Période de grâce (hors tolérance)",
        4 => "Non authentique",
        5 => "Notification (non activé)",
        6 => "Période de grâce étendue",
        _ => "Inconnu",
    }
}

/// GUID de l'application Windows pour SoftwareLicensingProduct.
const WINDOWS_APP_ID: &str = "55c92734-d682-4d71-983e-d6ec3f16059f";

/// Lit le statut de licence Windows via CIM. Retourne le produit Windows qui porte
/// une clé (PartialProductKey non nul) ; à défaut, un statut vide « Non licencié ».
#[cfg(target_os = "windows")]
pub fn query_license_status() -> Result<LicenseStatus, NiTriTeError> {
    // Sépare les champs par un délimiteur improbable ; une ligne par produit Windows licencié.
    let script = format!(
        "$ErrorActionPreference='SilentlyContinue'; \
         Get-CimInstance SoftwareLicensingProduct -Filter \"ApplicationID='{app}' AND PartialProductKey IS NOT NULL\" | \
         ForEach-Object {{ \
           \"$($_.Name)|$($_.Description)|$($_.LicenseStatus)|$($_.GracePeriodRemaining)|$($_.ProductKeyChannel)|$($_.PartialProductKey)\" }}",
        app = WINDOWS_APP_ID
    );
    let r = execute_system_command(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", &script],
        40,
    )?;
    if !r.success {
        return Err(NiTriTeError::System(format!(
            "Lecture du statut de licence impossible : {}",
            r.stderr.trim()
        )));
    }
    let line = r
        .stdout
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    if line.is_empty() {
        return Ok(LicenseStatus {
            license_status_text: status_text(0).to_string(),
            ..Default::default()
        });
    }
    let f: Vec<&str> = line.split('|').collect();
    let get = |i: usize| f.get(i).map(|s| s.trim().to_string()).unwrap_or_default();
    let license_status: u32 = get(2).parse().unwrap_or(0);
    let grace_minutes: u64 = get(3).parse().unwrap_or(0);
    Ok(LicenseStatus {
        name: get(0),
        description: get(1),
        license_status,
        license_status_text: status_text(license_status).to_string(),
        grace_minutes,
        grace_days: grace_minutes / 1440,
        channel: get(4),
        partial_key: get(5),
        activated: license_status == 1,
    })
}

#[cfg(not(target_os = "windows"))]
pub fn query_license_status() -> Result<LicenseStatus, NiTriTeError> {
    Err(NiTriTeError::System("Disponible sous Windows uniquement.".into()))
}

/// Texte brut de `slmgr /dlv` (diagnostic détaillé, localisé) pour affichage.
pub fn detailed_license_info() -> Result<CommandResult, NiTriTeError> {
    run_slmgr(&["/dlv"], 40)
}

/// Texte brut de `slmgr /xpr` (date d'expiration lisible).
pub fn expiration_info() -> Result<CommandResult, NiTriTeError> {
    run_slmgr(&["/xpr"], 30)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slmgr_path_contient_slmgr_vbs() {
        assert!(slmgr_path().to_lowercase().ends_with("system32\\slmgr.vbs"));
    }

    #[test]
    fn status_text_couvre_codes_connus() {
        assert_eq!(status_text(1), "Activé (licencié)");
        assert_eq!(status_text(0), "Non licencié");
        assert_eq!(status_text(42), "Inconnu");
    }
}
