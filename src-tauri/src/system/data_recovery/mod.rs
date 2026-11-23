/// data_recovery/mod.rs — Re-exports de tous les sous-modules

pub mod shadow_vss;
pub mod disk_recovery;
pub mod backup_folders;

pub use shadow_vss::*;
pub use disk_recovery::*;
pub use backup_folders::*;

// ─── Utilitaire PowerShell partagé ────────────────────────────────────────────

/// Délai maximal des scripts de récupération (VSS, USN, corbeille) : un
/// lecteur USB débranché ou un service VSS figé ne doit plus bloquer le thread
/// indéfiniment (l'UI restait en chargement sans fin).
const PS_TIMEOUT_SECS: u64 = 900;

pub(super) fn run_ps(script: &str) -> Option<String> {
    crate::maintenance::commands::execute_system_command(
        "powershell", &["-NoProfile", "-NonInteractive", "-Command", script], PS_TIMEOUT_SECS,
    ).ok().map(|o| o.stdout)
}

/// Comme `run_ps` mais passe des arguments supplémentaires accessibles via `$args[0]`, `$args[1]`, …
/// Les valeurs sont transmises comme arguments de processus séparés — aucune interpolation PS possible.
/// Le script est enveloppé dans `& { ... }` : passé nu à `-Command`, il tourne dans la
/// portée top-level de powershell.exe où `$args` ne recoit JAMAIS les arguments de ligne
/// de commande suivants (silencieusement $null, jamais d'erreur) — seule une invocation
/// de scriptblock reelle (`& { }`) les reçoit. Confirmé en testant les deux formes cote a
/// cote sur une machine reelle : nu -> $args[0] est $null ; enveloppé -> $args[0] correct.
/// decode_output (via execute_system_command) : noms de fichiers/dossiers
/// récupérés couramment accentués FR.
pub(super) fn run_ps_with_args(script: &str, extra_args: &[&str]) -> Option<String> {
    let wrapped = format!("& {{ {} }}", script);
    let mut args = vec!["-NoProfile", "-NonInteractive", "-Command", wrapped.as_str()];
    args.extend_from_slice(extra_args);
    crate::maintenance::commands::execute_system_command("powershell", &args, PS_TIMEOUT_SECS)
        .ok()
        .map(|o| o.stdout)
}
