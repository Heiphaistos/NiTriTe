//! activation — Réactivation Windows pour maintenance technicien.
//!
//! Voie unique retenue : HWID permanent, délégué à MAS (logiciel/MAS/MAS_AIO.cmd,
//! mode `/HWID`), en un clic (mode non-interactif de MAS, fermeture auto).
//!
//! Fonction de maintenance légitime : remettre en règle un Windows après réparation /
//! réinstallation par le propriétaire de l'outil.
//!
//! Le serveur KMS natif (`kms_server`, `crypto`) reste dans le dépôt mais n'est plus
//! câblé à l'UI : l'activation finale contre un vrai Windows 11 était encore refusée
//! (0x8007000D) malgré un framing RPC et des primitives crypto prouvés corrects —
//! HWID via MAS est la méthode fiable et activée.

pub mod crypto;
pub mod gvlk;
pub mod kms_server;
pub mod slmgr;

use serde::Serialize;

use crate::error::NiTriTeError;
use crate::maintenance::commands::execute_system_command;
use gvlk::DetectedEdition;
use slmgr::LicenseStatus;

// ── Résultats sérialisables ─────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ActivationStatus {
    pub edition: DetectedEdition,
    pub license: LicenseStatus,
    /// Texte brut de `slmgr /dlv` (diagnostic détaillé, localisé).
    pub dlv_raw: String,
    /// Texte brut de `slmgr /xpr` (expiration lisible).
    pub xpr_raw: String,
}

/// Une étape d'orchestration et son résultat réel.
#[derive(Debug, Serialize)]
pub struct Step {
    pub label: String,
    pub success: bool,
    pub output: String,
}

// ── Commandes Tauri ─────────────────────────────────────────────────────────

/// État réel de l'activation : édition détectée, statut de licence (CIM, langue-neutre),
/// et sortie brute de `slmgr /dlv` + `/xpr`.
#[tauri::command]
pub async fn activation_status() -> Result<ActivationStatus, NiTriTeError> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            let edition = gvlk::detect_edition();
            let license = slmgr::query_license_status().unwrap_or_default();
            let dlv_raw = slmgr::detailed_license_info().map(|c| {
                let mut s = c.stdout;
                if s.trim().is_empty() {
                    s = c.stderr;
                }
                s.trim().to_string()
            }).unwrap_or_default();
            let xpr_raw = slmgr::expiration_info().map(|c| c.stdout.trim().to_string()).unwrap_or_default();
            Ok(ActivationStatus { edition, license, dlv_raw, xpr_raw })
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(NiTriTeError::System("Disponible sous Windows uniquement.".into()))
        }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

/// Résout le chemin de `logiciel/MAS/MAS_AIO.cmd` (à côté de l'exe, avec replis dev).
#[cfg(target_os = "windows")]
fn find_mas_script() -> Option<String> {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("logiciel").join("MAS").join("MAS_AIO.cmd"));
            // Replis dev : target\debug\ -> remonter vers la racine projet.
            candidates.push(dir.join("..").join("..").join("logiciel").join("MAS").join("MAS_AIO.cmd"));
            candidates.push(dir.join("..").join("..").join("..").join("logiciel").join("MAS").join("MAS_AIO.cmd"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("logiciel").join("MAS").join("MAS_AIO.cmd"));
    }
    candidates.into_iter().find(|p| p.exists()).map(|p| p.to_string_lossy().into_owned())
}

/// Lance l'activation HWID permanente via MAS, en mode `/HWID`, élevé, en un clic
/// (fenêtre non-interactive qui se referme seule). Aucune valeur utilisateur :
/// le chemin est résolu localement, l'argument est fixe.
#[tauri::command]
pub async fn activation_hwid() -> Result<Step, NiTriTeError> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            let Some(script) = find_mas_script() else {
                return Err(NiTriTeError::System(
                    "MAS introuvable (logiciel/MAS/MAS_AIO.cmd). Vérifiez l'installation.".into(),
                ));
            };
            // Start-Process élevé (RunAs), MAS en mode HWID. `/HWID` seul met déjà MAS
            // en mode non-interactif (aucune invite, fermeture auto après ~2 s) ; `/c`
            // (et non `/k`) referme la fenêtre cmd avec lui : vraie activation en un clic.
            // La chaîne entre guillemets protège le chemin à espaces ; argument fixe `/HWID`.
            let ps = format!(
                "Start-Process -FilePath 'cmd.exe' -Verb RunAs -ArgumentList '/c \"{}\" /HWID'",
                script
            );
            let r = execute_system_command(
                "powershell",
                &["-NoProfile", "-NonInteractive", "-Command", &ps],
                30,
            )?;
            Ok(Step {
                label: "Lancement de l'activation HWID (MAS)".into(),
                success: r.success,
                output: if r.success {
                    "Activation HWID lancée (élevée, un clic). La fenêtre se referme seule \
                     une fois terminée ; l'activation HWID est permanente et liée au matériel."
                        .into()
                } else {
                    let e = r.stderr.trim();
                    if e.is_empty() { "Élévation refusée ou lancement annulé.".into() } else { e.to_string() }
                },
            })
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(NiTriTeError::System("Disponible sous Windows uniquement.".into()))
        }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}
