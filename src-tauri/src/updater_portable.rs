// Mise a jour de la version PORTABLE.
//
// Le plugin officiel de Tauri ne sait pas mettre a jour une application
// portable sous Windows : `tauri-plugin-updater` ne connait que deux formes,
// `WindowsUpdaterType::Nsis` et `::Msi`. Il lance un installeur puis quitte. Une
// copie portable n'a pas d'installeur -- elle EST le fichier.
//
// Ce module fait donc le travail a la main, comme le lanceur de ForgeMT2 :
//
//   1. lire le manifeste sur le serveur ;
//   2. telecharger le binaire et sa signature ;
//   3. VERIFIER la signature minisign avec la meme cle publique que l'updater
//      officiel -- un binaire qui n'est pas signe par la cle du projet n'est
//      jamais ecrit sur le disque de l'utilisateur ;
//   4. Windows interdit de SUPPRIMER un .exe en cours d'execution, mais
//      autorise a le RENOMMER. On ecarte l'ancien, on met le neuf a sa place ;
//   5. l'ancien est balaye au demarrage suivant.
//
// Ce chemin ne connait qu'UN SEUL fichier : l'executable. Les dossiers
// `logiciel\`, `Drivers\` et `Script Windows\` ne sont jamais ouverts, jamais
// listes, jamais touches -- ils appartiennent a Momo, pas a la mise a jour.

use std::path::PathBuf;
use std::time::Duration;

use minisign_verify::{PublicKey, Signature};
use serde::{Deserialize, Serialize};

/// Manifeste de la version portable. Sert aussi de temoin d'existence du canal.
const MANIFESTE: &str = "https://nitrite.heiphaistos.org/maj/latest-portable.json";

/// Meme cle que `plugins.updater.pubkey` dans tauri.conf.json : les deux
/// chemins de mise a jour partagent la confiance, pas le mecanisme.
const CLE_PUBLIQUE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDU1QTZDRUFBQUI4OTA3OTEKUldTUkI0bXJxczZtVmJWWHpKQXJpTmVQRENXMDlScDlMYm1BL1d1VDZoM2VheFF4cURTakJURkUK";

/// Un binaire Nitrite pese ~21 Mo. Au-dela de 200 Mo, ce n'est pas notre
/// executable : on arrete avant de remplir le disque de l'utilisateur.
const TAILLE_MAX: u64 = 200 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct Plateforme {
    url: String,
    signature: String,
}

#[derive(Debug, Deserialize)]
struct Manifeste {
    version: String,
    #[serde(default)]
    notes: String,
    platforms: std::collections::HashMap<String, Plateforme>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MajPortable {
    pub version: String,
    pub version_actuelle: String,
    pub notes: String,
    url: String,
    signature: String,
}

fn exe_courant() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("executable introuvable : {e}"))
}

/// Balaye la depouille laissee par la mise a jour precedente. Elle ne peut pas
/// etre supprimee pendant qu'elle tourne, seulement ecartee : c'est au
/// lancement SUIVANT qu'elle disparait.
pub fn balayer_ancien() {
    if let Ok(exe) = exe_courant() {
        let ancien = exe.with_extension("ancien");
        if ancien.exists() {
            let _ = std::fs::remove_file(&ancien);
        }
    }
}

/// Vrai quand cette copie a ete posee par l'installeur NSIS.
///
/// Temoin : `uninstall.exe`, ecrit par NSIS a cote de l'executable et par lui
/// seul. Les dossiers `Drivers\` / `logiciel\` ne peuvent PAS servir de temoin :
/// depuis que les deux versions embarquent le contenu complet, ils sont
/// presents des deux cotes.
pub fn est_installee() -> bool {
    exe_courant()
        .ok()
        .and_then(|e| e.parent().map(|p| p.join("uninstall.exe")))
        .map(|p| p.is_file())
        .unwrap_or(false)
}

fn plus_recente(distante: &str, locale: &str) -> bool {
    match (semver::Version::parse(distante), semver::Version::parse(locale)) {
        (Ok(d), Ok(l)) => d > l,
        // Un manifeste illisible ne declenche jamais de mise a jour.
        _ => false,
    }
}

async fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(concat!("Nitrite/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())
}

/// Interroge le manifeste. `Ok(None)` = rien de neuf, et c'est le cas normal.
#[tauri::command]
pub async fn portable_maj_verifier() -> Result<Option<MajPortable>, String> {
    let actuelle = env!("CARGO_PKG_VERSION");

    let texte = client()
        .await?
        .get(MANIFESTE)
        .send()
        .await
        .map_err(|e| format!("canal de mise a jour injoignable : {e}"))?
        .error_for_status()
        .map_err(|e| format!("canal de mise a jour en erreur : {e}"))?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    let manifeste: Manifeste =
        serde_json::from_str(&texte).map_err(|e| format!("manifeste illisible : {e}"))?;

    if !plus_recente(&manifeste.version, actuelle) {
        return Ok(None);
    }

    let plateforme = manifeste
        .platforms
        .get("windows-x86_64")
        .ok_or_else(|| "manifeste sans entree windows-x86_64".to_string())?;

    Ok(Some(MajPortable {
        version: manifeste.version,
        version_actuelle: actuelle.to_string(),
        notes: manifeste.notes,
        url: plateforme.url.clone(),
        signature: plateforme.signature.clone(),
    }))
}

fn verifier_signature(octets: &[u8], signature_b64: &str) -> Result<(), String> {
    use base64::Engine;
    let moteur = base64::engine::general_purpose::STANDARD;

    let cle_texte = moteur
        .decode(CLE_PUBLIQUE)
        .map_err(|e| format!("cle publique illisible : {e}"))?;
    let cle = PublicKey::decode(
        std::str::from_utf8(&cle_texte).map_err(|e| format!("cle publique illisible : {e}"))?,
    )
    .map_err(|e| format!("cle publique illisible : {e}"))?;

    // La signature du manifeste est le fichier .sig entier, encode en base64 --
    // meme convention que l'updater officiel.
    let sig_texte = moteur
        .decode(signature_b64)
        .map_err(|e| format!("signature illisible : {e}"))?;
    let signature = Signature::decode(
        std::str::from_utf8(&sig_texte).map_err(|e| format!("signature illisible : {e}"))?,
    )
    .map_err(|e| format!("signature illisible : {e}"))?;

    cle.verify(octets, &signature, true)
        .map_err(|_| "signature invalide : ce binaire n'a pas ete publie par Nitrite".to_string())
}

/// Telecharge, verifie, met en place, et rend la main. Le redemarrage est
/// declenche par l'appelant.
#[tauri::command]
pub async fn portable_maj_appliquer(maj: MajPortable) -> Result<(), String> {
    let reponse = client()
        .await?
        .get(&maj.url)
        .send()
        .await
        .map_err(|e| format!("telechargement impossible : {e}"))?
        .error_for_status()
        .map_err(|e| format!("telechargement refuse : {e}"))?;

    if let Some(taille) = reponse.content_length() {
        if taille > TAILLE_MAX {
            return Err(format!("binaire annonce a {taille} octets : refuse"));
        }
    }

    let octets = reponse.bytes().await.map_err(|e| e.to_string())?;
    if octets.len() as u64 > TAILLE_MAX {
        return Err("binaire trop gros : refuse".to_string());
    }

    // AVANT d'ecrire quoi que ce soit sur le disque de l'utilisateur.
    verifier_signature(&octets, &maj.signature)?;

    let exe = exe_courant()?;
    let nouveau = exe.with_extension("nouveau");
    let ancien = exe.with_extension("ancien");

    std::fs::write(&nouveau, &octets).map_err(|e| format!("ecriture impossible : {e}"))?;

    if ancien.exists() {
        let _ = std::fs::remove_file(&ancien);
    }
    // Windows refuse de supprimer un binaire en cours d'execution, mais accepte
    // de le renommer.
    if let Err(e) = std::fs::rename(&exe, &ancien) {
        let _ = std::fs::remove_file(&nouveau);
        return Err(format!("impossible d'ecarter l'ancienne version : {e}"));
    }
    if let Err(e) = std::fs::rename(&nouveau, &exe) {
        // Remettre l'ancienne en place : mieux vaut une version perimee qu'une
        // installation sans executable.
        let _ = std::fs::rename(&ancien, &exe);
        return Err(format!("impossible de mettre en place la nouvelle version : {e}"));
    }

    tracing::info!("mise a jour portable appliquee : {}", maj.version);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_les_versions() {
        assert!(plus_recente("8.218.0", "8.217.0"));
        assert!(plus_recente("8.217.1", "8.217.0"));
        assert!(!plus_recente("8.217.0", "8.217.0"));
        assert!(!plus_recente("8.216.0", "8.217.0"));
    }

    #[test]
    fn un_manifeste_illisible_ne_met_rien_a_jour() {
        assert!(!plus_recente("pas-une-version", "8.217.0"));
        assert!(!plus_recente("", "8.217.0"));
    }

    // La garde qui compte : un binaire non signe par la cle du projet ne doit
    // jamais atteindre le disque.
    #[test]
    fn refuse_une_signature_qui_ne_correspond_pas() {
        use base64::Engine;
        let moteur = base64::engine::general_purpose::STANDARD;
        let fausse = moteur.encode(
            "untrusted comment: signature from tauri secret key\n\
             RUSRB4mrqs6mVT5H/1lv3wtYJrXeE7h48Lml49bXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX==\n",
        );
        assert!(verifier_signature(b"contenu quelconque", &fausse).is_err());
    }

    #[test]
    fn refuse_une_signature_qui_nest_pas_du_base64() {
        assert!(verifier_signature(b"contenu", "ceci n'est pas du base64 !!").is_err());
    }
}
