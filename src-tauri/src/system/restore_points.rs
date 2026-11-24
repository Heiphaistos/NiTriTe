use serde::Serialize;
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::error::NiTriTeError;

#[derive(Debug, Clone, Serialize)]
pub struct RestorePoint {
    pub sequence_number: u32,
    pub description: String,
    pub creation_time: String,
    pub restore_type: String,
}

/// PowerShell avec timeout : `.output()` brut n'avait aucune limite, un service
/// VSS / System Restore bloqué figeait la page indéfiniment.
fn run_ps(script: &str, timeout_secs: u64) -> Result<crate::maintenance::commands::CommandResult, NiTriTeError> {
    crate::maintenance::commands::execute_system_command(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", script],
        timeout_secs,
    )
}

pub fn list_restore_points() -> Result<Vec<RestorePoint>, NiTriTeError> {
    let ps = r#"
try {
    $rps = Get-ComputerRestorePoint -ErrorAction Stop | Select-Object SequenceNumber, Description, CreationTime, @{Name='RestorePointType';Expression={
        # RestorePointType est un entier (0/1/10/12/13) ; .ToString() renverrait
        # "0"/"12"… au lieu du nom → le frontend afficherait un numéro brut.
        $t = [int]$_.RestorePointType
        switch ($t) {
            0  {'APPLICATION_INSTALL'}
            1  {'APPLICATION_UNINSTALL'}
            10 {'DEVICE_DRIVER_INSTALL'}
            12 {'MODIFY_SETTINGS'}
            13 {'CANCELLED_OPERATION'}
            default {"TYPE_$t"}
        }
    }}
    $rps | ConvertTo-Json -Compress
} catch {
    "[]"
}
"#;
    let output = run_ps(ps, 60)?;
    let trimmed = output.stdout.trim();

    if trimmed.is_empty() || trimmed == "[]" {
        return Ok(vec![]);
    }

    // PowerShell peut retourner un objet unique au lieu d'un tableau
    let json_str = if trimmed.starts_with('{') {
        format!("[{}]", trimmed)
    } else {
        trimmed.to_string()
    };

    let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str)
        .unwrap_or_default();

    let points: Vec<RestorePoint> = parsed
        .iter()
        .filter_map(|v| {
            let seq = v["SequenceNumber"].as_u64()? as u32;
            let desc = v["Description"].as_str().unwrap_or("").to_string();
            let time = v["CreationTime"].as_str().unwrap_or("").to_string();
            let rtype = v["RestorePointType"].as_str().unwrap_or("APPLICATION_INSTALL").to_string();
            Some(RestorePoint {
                sequence_number: seq,
                description: desc,
                creation_time: time,
                restore_type: rtype,
            })
        })
        .collect();

    Ok(points)
}

pub fn create_restore_point(description: &str) -> Result<(), NiTriTeError> {
    // Nécessite les droits admin
    let ps = format!(
        r#"
$desc = '{}'
$key  = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\SystemRestore'
$name = 'SystemRestorePointCreationFrequency'
$prev = (Get-ItemProperty -Path $key -Name $name -EA SilentlyContinue).$name
$before = @(Get-ComputerRestorePoint -EA SilentlyContinue | ForEach-Object {{ $_.SequenceNumber }})
try {{
    # Contourne la limite de fréquence Windows (1 point / 24 h par défaut) : sans
    # ça, Checkpoint-Computer ne fait RIEN et sort quand même en 0 → faux succès
    # (« point créé » sans point réel). Mettre la fréquence à 0 force la création.
    New-ItemProperty -Path $key -Name $name -Value 0 -PropertyType DWord -Force -EA SilentlyContinue | Out-Null
    Checkpoint-Computer -Description $desc -RestorePointType "APPLICATION_INSTALL" -ErrorAction Stop
    # Preuve : un NOUVEAU point portant cette description doit exister.
    $new = Get-ComputerRestorePoint -EA SilentlyContinue | Where-Object {{ $before -notcontains $_.SequenceNumber -and $_.Description -eq $desc }}
    if (-not $new) {{ throw "Windows n'a créé aucun point (protection du système désactivée sur C: ?)" }}
    Write-Output "OK"
}} catch {{
    Write-Error $_.Exception.Message
    exit 1
}} finally {{
    # Remet la valeur d'origine : la fréquence 0 restait sinon en place pour
    # toujours (modification permanente d'un réglage système). `finally`
    # s'exécute aussi après `exit` dans le catch.
    if ($null -eq $prev) {{ Remove-ItemProperty -Path $key -Name $name -EA SilentlyContinue }}
    else {{ Set-ItemProperty -Path $key -Name $name -Value $prev -EA SilentlyContinue }}
}}
"#,
        description.replace('\'', "''")
    );

    // Checkpoint-Computer peut durer plusieurs minutes (VSS sur gros disque).
    let output = run_ps(&ps, 600)?;
    if !output.success {
        return Err(NiTriTeError::System(format!(
            "Échec création point de restauration : {}",
            output.stderr.trim()
        )));
    }

    Ok(())
}

#[tauri::command]
pub async fn list_restore_points_cmd() -> Result<Vec<RestorePoint>, NiTriTeError> {
    tokio::task::spawn_blocking(list_restore_points)
        .await
        .map_err(|e| NiTriTeError::System(e.to_string()))?
}

#[tauri::command]
pub async fn create_restore_point_cmd(description: String) -> Result<(), NiTriTeError> {
    tokio::task::spawn_blocking(move || create_restore_point(&description))
        .await
        .map_err(|e| NiTriTeError::System(e.to_string()))?
}

#[tauri::command]
pub async fn delete_restore_point_cmd(sequence_number: u32) -> Result<(), NiTriTeError> {
    tokio::task::spawn_blocking(move || {
        // API officielle SRRemoveRestorePoint (srclient.dll) par numéro de
        // séquence. L'ancienne méthode supprimait « le premier cliché VSS créé
        // à ±60 s » : elle pouvait viser le cliché d'un AUTRE volume ou d'un
        // logiciel de sauvegarde, et laisser le point en place.
        let ps = format!(
            r#"
$seq = {seq}
$rp = Get-ComputerRestorePoint | Where-Object {{ $_.SequenceNumber -eq $seq }} | Select-Object -First 1
if (-not $rp) {{ throw "Point de restauration #{seq} introuvable." }}
Add-Type -Namespace NiTriTe -Name SrClient -MemberDefinition '[DllImport("srclient.dll")] public static extern int SRRemoveRestorePoint(int index);'
$rc = [NiTriTe.SrClient]::SRRemoveRestorePoint($seq)
if ($rc -ne 0) {{ throw "SRRemoveRestorePoint a renvoyé le code $rc." }}
if (Get-ComputerRestorePoint | Where-Object {{ $_.SequenceNumber -eq $seq }}) {{ throw "Le point #{seq} est toujours présent après la suppression." }}
Write-Output "OK"
"#,
            seq = sequence_number
        );
        let output = run_ps(&ps, 120)?;
        if output.success {
            Ok(())
        } else {
            Err(NiTriTeError::System(format!(
                "Suppression échouée : {}",
                output.stderr.trim()
            )))
        }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

pub fn rollback_last_windows_update() -> Result<String, NiTriTeError> {
    let ps = r#"
try {
    $kb = Get-HotFix | Sort-Object InstalledOn -Descending | Select-Object -First 1 -ErrorAction Stop
    if (-not $kb) { throw "Aucune mise a jour trouvee" }
    $kbId = $kb.HotFixID -replace 'KB', ''
    $proc = Start-Process -FilePath "wusa.exe" -ArgumentList "/uninstall", "/kb:$kbId", "/quiet", "/norestart" -Wait -PassThru -ErrorAction Stop
    if ($proc.ExitCode -eq 0 -or $proc.ExitCode -eq 3010) {
        Write-Output "OK:$($kb.HotFixID)"
    } else {
        throw "wusa.exe code de sortie: $($proc.ExitCode)"
    }
} catch {
    Write-Error $_.Exception.Message
    exit 1
}
"#;

    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", ps])
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| NiTriTeError::System(e.to_string()))?;

    if !output.status.success() {
        let err = crate::maintenance::commands::decode_output(&output.stderr);
        return Err(NiTriTeError::System(format!(
            "Rollback échoué : {}",
            err.trim()
        )));
    }

    let stdout = crate::maintenance::commands::decode_output(&output.stdout).trim().to_string();
    let kb_id = stdout.strip_prefix("OK:").unwrap_or(&stdout).to_string();
    Ok(kb_id)
}

#[tauri::command]
pub async fn rollback_last_windows_update_cmd() -> Result<String, NiTriTeError> {
    tokio::task::spawn_blocking(rollback_last_windows_update)
        .await
        .map_err(|e| NiTriTeError::System(e.to_string()))?
}
