use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct BsodEntry {
    pub timestamp: String,
    pub bug_check_code: String,
    pub bug_check_hex: String,
    pub description: String,
    pub parameters: Vec<String>,
    pub module: String,
    pub dump_file: String,
    pub dump_size_kb: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct BsodReport {
    pub entries: Vec<BsodEntry>,
    pub total_count: u32,
    pub last_bsod: String,
    pub dump_folder: String,
    pub dump_count: u32,
}

#[tauri::command]
pub async fn get_bsod_history() -> BsodReport {
    tokio::task::spawn_blocking(get_bsod_report).await.unwrap_or_default()
}

pub fn get_bsod_report() -> BsodReport {
    let ps = r#"
$report = @{}
try {
    # Get BSOD events from System log (ID 1001 = BugCheck)
    $events = @(Get-WinEvent -FilterHashtable @{LogName='System';Id=1001;ProviderName='Microsoft-Windows-WER-SystemErrorReporting'} -MaxEvents 20 -EA SilentlyContinue |
        ForEach-Object {
            $msg = $_.Message
            # « ... était : 0x0000009f (0x..., 0x..., 0x..., 0x...) » (FR/EN) :
            # 1er hexa = code, les 4 suivants = paramètres (jamais lus avant).
            $hexes = @([regex]::Matches($msg, '0x[0-9A-Fa-f]+') | ForEach-Object { $_.Value })
            $dump = if($msg -match '([A-Za-z]:\\\S+?\.dmp)'){$Matches[1]}else{''}
            $dumpItem = if($dump){Get-Item -LiteralPath $dump -EA SilentlyContinue}else{$null}
            @{
                ts   = $_.TimeCreated.ToString('yyyy-MM-dd HH:mm:ss')
                code = if($hexes.Count){$hexes[0]}else{''}
                desc = ($msg -split '\n')[0].Trim()
                mod  = if($msg -match 'caused by.*?(\S+\.sys)'){$Matches[1]}else{''}
                dump = $dump
                kb   = if($dumpItem){[int]($dumpItem.Length/1024)}else{0}
                params = @($hexes | Select-Object -Skip 1 -First 4)
            }
        })

    # Supplement with WER reports
    $werDir = "$env:ProgramData\Microsoft\Windows\WER\ReportArchive"
    if (Test-Path $werDir) {
        $werEntries = @(Get-ChildItem $werDir -Directory -EA SilentlyContinue |
            Where-Object { $_.Name -like 'Kernel*' } |
            # Les dossiers Kernel_* contiennent aussi les LiveKernelEvent
            # (réinitialisation du pilote graphique, etc.) qui ne sont PAS des
            # écrans bleus : ne garder que EventType=BlueScreen.
            Where-Object {
                $et = Select-String -LiteralPath (Join-Path $_.FullName 'Report.wer') -Pattern '^EventType=(.+)$' -EA SilentlyContinue | Select-Object -First 1
                $et -and $et.Matches[0].Groups[1].Value.Trim() -eq 'BlueScreen'
            } |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 10 |
            ForEach-Object {
                $bcFile = Get-Item (Join-Path $_.FullName 'memory.dmp') -EA SilentlyContinue
                @{
                    ts   = $_.LastWriteTime.ToString('yyyy-MM-dd HH:mm:ss')
                    code = if($_.Name -match '^Kernel_([0-9A-Fa-f]+)_'){'0x'+$Matches[1]}else{$_.Name}
                    desc = "Rapport WER (écran bleu)"
                    mod  = ''
                    dump = if($bcFile){$bcFile.FullName}else{''}
                    kb   = if($bcFile){[int]($bcFile.Length/1024)}else{0}
                    params = @()
                }
            })
        $events += $werEntries
    }

    # Minidumps
    $dumpDir = 'C:\Windows\Minidump'
    $dumpFiles = @(Get-ChildItem $dumpDir -Filter '*.dmp' -EA SilentlyContinue | Sort-Object LastWriteTime -Descending)

    $report.entries = $events
    # Un même écran bleu laisse souvent un événement 1001 ET un rapport WER :
    # les additionner doublait le compteur. Le plus grand des deux = nombre
    # minimal d'écrans bleus distincts.
    $evCount  = @($events | Where-Object { $_.desc -ne 'Rapport WER (écran bleu)' }).Count
    $werCount = $events.Count - $evCount
    $report.total   = [int][math]::Max($evCount, $werCount)
    $report.last    = if($events.Count -gt 0){$events[0].ts}else{'Aucun'}
    $report.dumpDir = $dumpDir
    $report.dumpCt  = [int]$dumpFiles.Count
} catch {
    $report.entries = @()
    $report.total   = 0
    $report.last    = 'Erreur: '+$_.Exception.Message
    $report.dumpDir = 'C:\Windows\Minidump'
    $report.dumpCt  = 0
}
$report | ConvertTo-Json -Depth 4 -Compress
"#;

    #[cfg(target_os = "windows")]
    {
        // Timeout (Get-WinEvent peut se figer sur un journal corrompu) ;
        // ps_with_timeout décode la sortie (messages d'événements localisés FR).
        if let Ok(t) = crate::utils::ps::ps_with_timeout(ps, 60) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(t.trim()) {
                let entries = v["entries"].as_array().map(|arr| arr.iter().map(|e| BsodEntry {
                    timestamp: e["ts"].as_str().unwrap_or("").to_string(),
                    bug_check_code: e["code"].as_str().unwrap_or("").to_string(),
                    bug_check_hex: e["code"].as_str().unwrap_or("").to_string(),
                    description: e["desc"].as_str().unwrap_or("").to_string(),
                    parameters: e["params"].as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(|x| x.to_string())).collect()).unwrap_or_default(),
                    module: e["mod"].as_str().unwrap_or("").to_string(),
                    dump_file: e["dump"].as_str().unwrap_or("").to_string(),
                    dump_size_kb: e["kb"].as_u64().unwrap_or(0),
                }).collect()).unwrap_or_default();
                return BsodReport {
                    entries,
                    total_count: v["total"].as_u64().unwrap_or(0) as u32,
                    last_bsod: v["last"].as_str().unwrap_or("Aucun").to_string(),
                    dump_folder: v["dumpDir"].as_str().unwrap_or("C:\\Windows\\Minidump").to_string(),
                    dump_count: v["dumpCt"].as_u64().unwrap_or(0) as u32,
                };
            }
        }
    }
    BsodReport { last_bsod: "Aucun".to_string(), dump_folder: "C:\\Windows\\Minidump".to_string(), ..Default::default() }
}

#[tauri::command]
pub fn get_bugcheck_description(code: String) -> String {
    // Le journal donne le code sur 8 chiffres (« 0x0000009f ») : sans retirer
    // les zéros de tête, aucun code réel ne trouvait sa description.
    let upper = code.trim().to_uppercase();
    let short = upper.trim_start_matches("0X").trim_start_matches('0');
    match short {
        "3B" => "SYSTEM_SERVICE_EXCEPTION — Erreur dans un service système ou driver".to_string(),
        "50" => "PAGE_FAULT_IN_NONPAGED_AREA — Accès mémoire invalide (driver défectueux)".to_string(),
        "7E" => "SYSTEM_THREAD_EXCEPTION_NOT_HANDLED — Exception non gérée dans un thread système".to_string(),
        "7F" => "UNEXPECTED_KERNEL_MODE_TRAP — Trap CPU imprévue (surchauffe/RAM défectueuse)".to_string(),
        "9F" => "DRIVER_POWER_STATE_FAILURE — Driver ne gère pas correctement l'alimentation".to_string(),
        "A" | "0A" => "IRQL_NOT_LESS_OR_EQUAL — Driver accède à une mauvaise adresse mémoire".to_string(),
        "D1" => "DRIVER_IRQL_NOT_LESS_OR_EQUAL — Même que 0A mais spécifique aux drivers".to_string(),
        "1E" => "KMODE_EXCEPTION_NOT_HANDLED — Exception non gérée en mode kernel".to_string(),
        "C4" => "DRIVER_VERIFIER_DETECTED_VIOLATION — Driver Verifier a détecté un problème".to_string(),
        "C5" => "DRIVER_CORRUPTED_EXPOOL — Pool mémoire corrompu par un driver".to_string(),
        "E3" => "RESOURCE_NOT_OWNED — Thread libère une ressource qu'il ne possède pas".to_string(),
        "1A" => "MEMORY_MANAGEMENT — Problème grave de gestion mémoire (souvent RAM)".to_string(),
        "19" => "BAD_POOL_HEADER — Corruption du pool mémoire du kernel".to_string(),
        "F4" => "CRITICAL_OBJECT_TERMINATION — Processus critique terminé de façon inattendue".to_string(),
        "124" => "WHEA_UNCORRECTABLE_ERROR — Erreur matérielle non corrigeable (CPU/BIOS/RAM)".to_string(),
        "133" => "DPC_WATCHDOG_VIOLATION — Timeout DPC (disque lent ou driver bloqué)".to_string(),
        "139" => "KERNEL_SECURITY_CHECK_FAILURE — Corruption mémoire kernel détectée".to_string(),
        "154" => "UNEXPECTED_STORE_EXCEPTION — Erreur de stockage (SSD/HDD défaillant)".to_string(),
        "101" => "CLOCK_WATCHDOG_TIMEOUT — CPU secondaire ne répond plus (overclocking)".to_string(),
        _ => format!("Code {} — voir https://learn.microsoft.com/fr-fr/windows-hardware/drivers/debugger/bug-check-code-reference2", code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_code_uppercase() {
        let d = get_bugcheck_description("0x50".to_string());
        assert!(d.contains("PAGE_FAULT_IN_NONPAGED_AREA"));
    }

    #[test]
    fn known_code_without_prefix() {
        let d = get_bugcheck_description("3B".to_string());
        assert!(d.contains("SYSTEM_SERVICE_EXCEPTION"));
    }

    #[test]
    fn known_code_lowercase_input() {
        let d = get_bugcheck_description("0xa".to_string());
        assert!(d.contains("IRQL_NOT_LESS_OR_EQUAL"));
    }

    #[test]
    fn code_0a_and_a_same_result() {
        let a = get_bugcheck_description("A".to_string());
        let b = get_bugcheck_description("0A".to_string());
        assert_eq!(a, b);
    }

    #[test]
    fn whea_code_124() {
        let d = get_bugcheck_description("124".to_string());
        assert!(d.contains("WHEA_UNCORRECTABLE_ERROR"));
    }

    #[test]
    fn unknown_code_links_to_docs() {
        let d = get_bugcheck_description("DEAD".to_string());
        assert!(d.contains("DEAD"));
        assert!(d.contains("learn.microsoft.com"));
    }

    #[test]
    fn padded_event_log_code_is_recognised() {
        // Format réel de l'événement 1001 : code sur 8 chiffres hexadécimaux.
        assert!(get_bugcheck_description("0x0000009f".to_string()).contains("DRIVER_POWER_STATE_FAILURE"));
        assert!(get_bugcheck_description("0x0000000A".to_string()).contains("IRQL_NOT_LESS_OR_EQUAL"));
        assert!(get_bugcheck_description("0x00000124".to_string()).contains("WHEA"));
    }

    #[test]
    fn memory_management_code() {
        let d = get_bugcheck_description("1A".to_string());
        assert!(d.contains("MEMORY_MANAGEMENT"));
    }
}
