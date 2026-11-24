//! activation — Réactivation Windows pour maintenance technicien.
//!
//! Module auto-contenu, deux voies :
//!   1. KMS local hors ligne : détection d'édition → pose de la GVLK officielle →
//!      activation contre un serveur KMS tournant sur 127.0.0.2:1688 (voir kms_server).
//!   2. HWID permanent : délègue à MAS (logiciel/MAS/MAS_AIO.cmd) en mode /HWID.
//!
//! Fonction de maintenance légitime : remettre en règle un Windows après réparation /
//! réinstallation par le propriétaire de l'outil.
//!
//! Voir le rapport pour l'état HONNÊTE du serveur KMS (framing RPC + primitives
//! prouvés ; assemblage cryptographique v6 à valider en VM).

pub mod crypto;
pub mod gvlk;
pub mod kms_server;
pub mod slmgr;

use std::sync::{Mutex, OnceLock};

use serde::Serialize;

use crate::error::NiTriTeError;
use crate::maintenance::commands::{execute_system_command, CommandResult};
use gvlk::DetectedEdition;
use kms_server::{KmsServer, KMS_BIND_ADDR};
use slmgr::LicenseStatus;

/// Serveur KMS local partagé (démarré par `activation_kms`, arrêté à la fermeture).
fn server_slot() -> &'static Mutex<Option<KmsServer>> {
    static S: OnceLock<Mutex<Option<KmsServer>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

/// Arrête le serveur KMS local s'il tourne. À appeler depuis le hook de fermeture
/// de l'app (l'orchestrateur le câble ; `Drop` sur `KmsServer` est le filet de secours).
pub fn shutdown_kms_server() {
    if let Ok(mut slot) = server_slot().lock() {
        if let Some(mut srv) = slot.take() {
            srv.stop();
        }
    }
}

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

impl Step {
    fn from(label: &str, r: Result<CommandResult, NiTriTeError>) -> Self {
        match r {
            Ok(c) => {
                let mut out = c.stdout.trim().to_string();
                if !c.stderr.trim().is_empty() {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(c.stderr.trim());
                }
                Step { label: label.to_string(), success: c.success, output: out }
            }
            Err(e) => Step { label: label.to_string(), success: false, output: e.to_string() },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct KmsResult {
    pub steps: Vec<Step>,
    /// Statut relu après l'opération.
    pub status_after: Option<LicenseStatus>,
    /// Avertissement d'honnêteté affiché à l'UI.
    pub note: String,
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

/// Active en KMS hors ligne : démarre le serveur local, pose la GVLK de l'édition
/// détectée, pointe slmgr sur 127.0.0.2:1688, lance `/ato`, puis planifie (option)
/// le renouvellement. Renvoie le résultat réel de chaque étape — rien n'est inventé.
#[tauri::command]
pub async fn activation_kms(schedule_renew: bool) -> Result<KmsResult, NiTriTeError> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            let mut steps = Vec::new();

            // 1. GVLK correspondant à l'édition installée.
            let edition = gvlk::detect_edition();
            let Some(gvlk) = edition.gvlk.clone() else {
                return Err(NiTriTeError::System(format!(
                    "Aucune GVLK connue pour l'édition « {} » (EditionID: {}). \
                     L'activation KMS ne s'applique qu'aux éditions Volume prises en charge.",
                    edition.product_name, edition.edition_id
                )));
            };

            // 2. Démarrage du serveur KMS local (si pas déjà lancé).
            {
                let mut slot = server_slot().lock().map_err(|_| NiTriTeError::System("verrou serveur".into()))?;
                if slot.is_none() {
                    match KmsServer::start() {
                        Ok(srv) => {
                            *slot = Some(srv);
                            steps.push(Step { label: format!("Serveur KMS local démarré ({KMS_BIND_ADDR})"), success: true, output: "en écoute sur la boucle locale uniquement".into() });
                        }
                        Err(e) => {
                            steps.push(Step { label: "Démarrage serveur KMS local".into(), success: false, output: format!("{e} (port 1688 déjà pris ?)") });
                            return Ok(KmsResult { steps, status_after: None, note: honest_note() });
                        }
                    }
                } else {
                    steps.push(Step { label: "Serveur KMS local".into(), success: true, output: "déjà en écoute".into() });
                }
            }

            // 3. slmgr /ipk <GVLK>
            steps.push(Step::from(&format!("Pose de la GVLK ({})", edition.gvlk_label.clone().unwrap_or_default()), slmgr::install_product_key(&gvlk)));
            // 4. slmgr /skms 127.0.0.2:1688
            steps.push(Step::from("Pointage du serveur KMS (/skms)", slmgr::set_kms_server(KMS_BIND_ADDR)));
            // 5. slmgr /ato
            steps.push(Step::from("Activation (/ato)", slmgr::activate_online()));
            // 6. Renouvellement planifié (optionnel)
            if schedule_renew {
                steps.push(Step::from("Planification du renouvellement (7 j)", slmgr::schedule_renewal()));
            }

            let status_after = slmgr::query_license_status().ok();
            Ok(KmsResult { steps, status_after, note: honest_note() })
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = schedule_renew;
            Err(NiTriTeError::System("Disponible sous Windows uniquement.".into()))
        }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

fn honest_note() -> String {
    "Si /ato échoue, l'activation KMS n'est pas fonctionnelle sur cette machine : le \
     répondeur KMS local est implémenté mais son assemblage cryptographique v6 reste à \
     valider (voir documentation). Utilisez l'activation HWID (via MAS) comme méthode fiable."
        .to_string()
}

/// Arrête explicitement le serveur KMS local (libère le port 1688).
#[tauri::command]
pub async fn activation_stop_kms() -> Result<bool, NiTriTeError> {
    tokio::task::spawn_blocking(|| {
        shutdown_kms_server();
        Ok(true)
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

/// Lance l'activation HWID permanente via MAS, en mode `/HWID`, élevé, fenêtre visible.
/// Aucune valeur utilisateur : le chemin est résolu localement, l'argument est fixe.
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
            // Start-Process élevé (RunAs), fenêtre cmd visible, MAS en mode HWID.
            // La chaîne entre guillemets protège le chemin à espaces ; argument fixe `/HWID`.
            let ps = format!(
                "Start-Process -FilePath 'cmd.exe' -Verb RunAs -ArgumentList '/k \"{}\" /HWID'",
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
                    "MAS lancé en mode HWID dans une fenêtre élevée. Suivez son déroulé ; \
                     l'activation HWID est permanente et liée au matériel."
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
