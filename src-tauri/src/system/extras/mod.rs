/// extras/mod.rs — Re-exports de tous les sous-modules extras

pub mod hash_dns_ports;
pub mod disk_files;
pub mod temps_wifi_turbo;
pub mod docker;
pub mod security_tools;

pub use hash_dns_ports::*;
pub use disk_files::*;
pub use temps_wifi_turbo::*;
pub use docker::*;
pub use security_tools::*;

// ─── Utilitaires partagés ──────────────────────────────────────────────────────

pub(super) fn parse_json_arr(s: &str) -> Vec<serde_json::Value> {
    let json = if s.starts_with('[') { s.to_string() } else { format!("[{}]", s) };
    serde_json::from_str(&json).unwrap_or_default()
}

/// Décode via `decode_output` (UTF-8 d'abord, repli codepage OEM) comme
/// `utils/ps.rs::ps()` : les sorties texte FR accentuées (noms de capteurs,
/// libellés netsh/docker) arrivaient en mojibake avec `from_utf8_lossy`.
/// Le fast path UTF-8 laisse les sorties JSON/ASCII inchangées.
pub(super) fn ps(script: &str) -> Result<String, String> {
    ps_timeout(script, 180)
}

/// `ps()` avec un délai maximal : un WMI ou un pilote figé ne bloque plus le
/// thread indéfiniment (le sondage des températures restait sinon suspendu et
/// la bannière d'alerte gardait une valeur périmée).
pub(super) fn ps_timeout(script: &str, timeout_secs: u64) -> Result<String, String> {
    let out = crate::maintenance::commands::execute_system_command(
        "powershell", &["-NoProfile", "-NonInteractive", "-Command", script], timeout_secs,
    ).map_err(|e| e.to_string())?;
    Ok(out.stdout.trim().to_string())
}

/// Comme `ps()` mais tient compte du code de sortie : `Err` si le script sort
/// non nul. `ps()` renvoie Ok dès que powershell.exe démarre — inutilisable
/// pour juger le succès réel d'une action (turbo, optimisations) : les scripts
/// doivent sortir `exit 1` en cas d'échec (try/catch ou test $LASTEXITCODE).
/// Décode stdout/stderr via `decode_output` (messages d'erreur PS en OEM FR).
pub(super) fn ps_ok(script: &str) -> Result<String, String> {
    // Délai maximal : un script figé (WMI, service) bloquait sinon la commande sans fin.
    let out = crate::maintenance::commands::execute_system_command(
        "powershell", &["-NoProfile", "-NonInteractive", "-Command", script], 600,
    ).map_err(|e| e.to_string())?;
    let stdout = out.stdout.trim().to_string();
    if out.success {
        Ok(stdout)
    } else {
        let stderr = out.stderr.trim().to_string();
        Err(match (stdout.is_empty(), stderr.is_empty()) {
            (false, _) => stdout,
            (true, false) => stderr,
            (true, true) => "échec (code de sortie non nul)".to_string(),
        })
    }
}

/// Comme `ps()` mais passe des arguments supplémentaires accessibles via `$args[0]`, `$args[1]`, …
/// Les valeurs sont transmises comme arguments de processus séparés — aucune interpolation PS possible.
/// Le script est enveloppé dans `& { ... }` : passé nu à `-Command`, il tourne dans la
/// portée top-level de powershell.exe où `$args` ne recoit JAMAIS les arguments de ligne
/// de commande suivants (silencieusement $null, jamais d'erreur) — seule une invocation
/// de scriptblock reelle (`& { }`) les reçoit. Confirmé en testant les deux formes cote a
/// cote sur une machine reelle : nu -> $args[0] est $null ; enveloppé -> $args[0] correct.
/// Délai maximal `timeout_secs` (processus tué au-delà) ; renvoie stdout ET
/// stderr, pour qu'un appelant puisse remonter l'erreur réelle au lieu d'un
/// résultat vide présenté comme un succès.
pub(super) fn ps_with_args(script: &str, extra_args: &[&str], timeout_secs: u64) -> Result<crate::maintenance::commands::CommandResult, String> {
    let wrapped = format!("& {{ {} }}", script);
    let mut args = vec!["-NoProfile", "-NonInteractive", "-Command", wrapped.as_str()];
    args.extend_from_slice(extra_args);
    crate::maintenance::commands::execute_system_command("powershell", &args, timeout_secs)
        .map_err(|e| e.to_string())
}
