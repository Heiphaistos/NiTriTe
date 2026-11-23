use serde::Serialize;

// ── Structs ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Default)]
pub struct DiskSmartInfo {
    pub disk_index: u32,
    pub label: String,
    pub health: String,          // "Healthy" | "Warning" | "Unhealthy" | "Unknown"
    pub temperature: Option<u32>,
    pub power_on_hours: Option<u32>,
    pub serial: String,
    pub size_gb: f64,
    pub media_type: String,      // "SSD" | "HDD" | "SCM" | "Unspecified"
    pub wear_level: Option<u32>,
    pub reallocated_sectors: Option<u32>,
    /// "USB", "NVMe", "SATA", "RAID"… (Get-PhysicalDisk BusType)
    pub bus_type: String,
    /// Clé USB / carte mémoire (Win32_DiskDrive « Removable Media ») — un
    /// disque dur externe USB de 1-2 To est « External hard disk media », donc
    /// `false` : c'est ce qui distingue les deux, BusType valant USB pour tous.
    pub is_removable: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct PartitionDetail {
    pub disk_index: u32,
    pub part_index: u32,
    pub disk_label: String,
    pub letter: String,
    pub label: String,
    pub size_gb: f64,
    pub free_gb: f64,
    pub file_system: String,
    pub part_type: String,       // "Basic" | "System" | "Recovery" | "Unallocated" | etc.
    pub is_system: bool,
    pub is_boot: bool,
    pub health: String,
    /// Identifiant stable du disque (Get-Disk UniqueId) : renvoyé par l'UI avec
    /// chaque action pour revérifier que le numéro de disque désigne toujours
    /// le même disque (les numéros changent au branchement d'un disque USB).
    pub disk_unique_id: String,
}

// ── SMART via Get-PhysicalDisk + Get-StorageReliabilityCounter ─────────────────

pub fn get_disks_smart() -> Vec<DiskSmartInfo> {
    let ps = r#"
try {
    $result = @()
    $removable = @{}
    Get-CimInstance Win32_DiskDrive -ErrorAction SilentlyContinue | ForEach-Object {
        $removable[[int]$_.Index] = ([string]$_.MediaType -like 'Removable*')
    }
    Get-PhysicalDisk -ErrorAction SilentlyContinue | ForEach-Object {
        $d = $_
        $smart = $null
        try { $smart = Get-StorageReliabilityCounter -PhysicalDisk $d -ErrorAction SilentlyContinue } catch {}
        $result += @{
            disk_index          = [int]$d.DeviceId
            label               = [string]$d.FriendlyName
            health              = [string]$d.HealthStatus
            temperature         = if ($smart -and $smart.Temperature   -ne $null) { [int]$smart.Temperature   } else { $null }
            power_on_hours      = if ($smart -and $smart.PowerOnHours  -ne $null) { [int]$smart.PowerOnHours  } else { $null }
            serial              = [string]$d.SerialNumber
            size_gb             = [math]::Round([double]$d.Size / 1GB, 1)
            media_type          = [string]$d.MediaType
            wear_level          = if ($smart -and $smart.Wear          -ne $null) { [int]$smart.Wear          } else { $null }
            reallocated_sectors = if ($smart -and $smart.ReadErrorsTotal -ne $null) { [int]$smart.ReadErrorsTotal } else { $null }
            bus_type            = [string]$d.BusType
            is_removable        = [bool]$removable[[int]$d.DeviceId]
        }
    }
    if ($result.Count -eq 0) { Write-Output '[]' }
    elseif ($result.Count -eq 1) { Write-Output "[$($result | ConvertTo-Json -Compress -Depth 2)]" }
    else { $result | ConvertTo-Json -Compress -Depth 2 }
} catch { Write-Output '[]' }
"#;

    run_ps(ps)
        .and_then(|t| {
            let t = t.trim();
            if t.is_empty() || t == "[]" { return Some(vec![]); }
            let j = if t.starts_with('{') { format!("[{}]", t) } else { t.to_string() };
            serde_json::from_str::<Vec<serde_json::Value>>(&j).ok()
        })
        .map(|vals| vals.into_iter().map(|v| {
            DiskSmartInfo {
                disk_index:          v["disk_index"].as_u64().unwrap_or(0) as u32,
                label:               v["label"].as_str().unwrap_or("Disque").to_string(),
                health:              v["health"].as_str().unwrap_or("Unknown").to_string(),
                temperature:         v["temperature"].as_u64().map(|x| x as u32),
                power_on_hours:      v["power_on_hours"].as_u64().map(|x| x as u32),
                serial:              v["serial"].as_str().unwrap_or("").to_string(),
                size_gb:             v["size_gb"].as_f64().unwrap_or(0.0),
                media_type:          v["media_type"].as_str().unwrap_or("HDD").to_string(),
                wear_level:          v["wear_level"].as_u64().map(|x| x as u32),
                reallocated_sectors: v["reallocated_sectors"].as_u64().map(|x| x as u32),
                bus_type:            v["bus_type"].as_str().unwrap_or("").to_string(),
                is_removable:        v["is_removable"].as_bool().unwrap_or(false),
            }
        }).collect())
        .unwrap_or_default()
}

// ── Liste des partitions avec espace non alloué ────────────────────────────────

pub fn get_partition_list() -> Vec<PartitionDetail> {
    let ps = r#"
try {
    $result = @()
    $disks = Get-Disk -ErrorAction SilentlyContinue
    foreach ($disk in $disks) {
        $diskLabel = [string]$disk.FriendlyName
        $diskId = [string]$disk.UniqueId
        $parts = Get-Partition -DiskNumber $disk.Number -ErrorAction SilentlyContinue
        $totalUsed = [long]0
        foreach ($part in $parts) {
            $vol = $null
            try { $vol = Get-Volume -Partition $part -ErrorAction SilentlyContinue } catch {}
            $totalUsed += [long]$part.Size
            $result += @{
                disk_index  = [int]$disk.Number
                part_index  = [int]$part.PartitionNumber
                disk_label  = $diskLabel
                letter      = if ($part.DriveLetter) { "$($part.DriveLetter):" } else { '' }
                label       = if ($vol) { [string]$vol.FileSystemLabel } else { '' }
                size_gb     = [math]::Round([double]$part.Size / 1GB, 2)
                free_gb     = if ($vol) { [math]::Round([double]$vol.SizeRemaining / 1GB, 2) } else { 0 }
                file_system = if ($vol) { [string]$vol.FileSystem } else { 'RAW' }
                part_type   = [string]$part.Type
                is_system   = [bool]$part.IsSystem
                is_boot     = [bool]$part.IsBoot
                health      = if ($vol -and $vol.HealthStatus) { [string]$vol.HealthStatus } else { 'Unknown' }
                disk_unique_id = $diskId
            }
        }
        $unalloc = [double]$disk.Size - [double]$totalUsed
        if ($unalloc -gt 5MB) {
            $result += @{
                disk_index  = [int]$disk.Number
                part_index  = 0
                disk_label  = $diskLabel
                letter      = ''
                label       = 'Espace non alloue'
                size_gb     = [math]::Round($unalloc / 1GB, 2)
                free_gb     = [math]::Round($unalloc / 1GB, 2)
                file_system = 'RAW'
                part_type   = 'Unallocated'
                is_system   = $false
                is_boot     = $false
                health      = 'Unknown'
                disk_unique_id = $diskId
            }
        }
    }
    if ($result.Count -eq 0) { Write-Output '[]' }
    elseif ($result.Count -eq 1) { Write-Output "[$($result | ConvertTo-Json -Compress -Depth 2)]" }
    else { $result | ConvertTo-Json -Compress -Depth 2 }
} catch { Write-Output '[]' }
"#;

    run_ps(ps)
        .and_then(|t| {
            let t = t.trim();
            if t.is_empty() || t == "[]" { return Some(vec![]); }
            let j = if t.starts_with('{') { format!("[{}]", t) } else { t.to_string() };
            serde_json::from_str::<Vec<serde_json::Value>>(&j).ok()
        })
        .map(|vals| vals.into_iter().map(|v| {
            PartitionDetail {
                disk_index:  v["disk_index"].as_u64().unwrap_or(0) as u32,
                part_index:  v["part_index"].as_u64().unwrap_or(0) as u32,
                disk_label:  v["disk_label"].as_str().unwrap_or("").to_string(),
                letter:      v["letter"].as_str().unwrap_or("").to_string(),
                label:       v["label"].as_str().unwrap_or("").to_string(),
                size_gb:     v["size_gb"].as_f64().unwrap_or(0.0),
                free_gb:     v["free_gb"].as_f64().unwrap_or(0.0),
                file_system: v["file_system"].as_str().unwrap_or("").to_string(),
                part_type:   v["part_type"].as_str().unwrap_or("").to_string(),
                is_system:   v["is_system"].as_bool().unwrap_or(false),
                is_boot:     v["is_boot"].as_bool().unwrap_or(false),
                health:      v["health"].as_str().unwrap_or("Unknown").to_string(),
                disk_unique_id: v["disk_unique_id"].as_str().unwrap_or("").to_string(),
            }
        }).collect())
        .unwrap_or_default()
}

// ── Opérations sur partitions (Admin requis) ───────────────────────────────────

/// Délais des opérations : un Get-Disk sur un disque mourant ou un
/// Resize-Partition (déplacement de données NTFS) ne doivent ni geler le
/// thread indéfiniment ni être coupés en plein travail.
const TIMEOUT_QUERY: u64 = 90;
const TIMEOUT_OP: u64 = 600;
const TIMEOUT_RESIZE: u64 = 3600;

/// Préambule PowerShell : revérifie l'identité du disque juste avant l'action.
/// Les numéros de disque changent quand on branche/débranche un disque USB :
/// sans ce contrôle, une action choisie sur « Disque 2 » pouvait frapper un
/// autre disque devenu « Disque 2 » entre l'affichage et le clic.
/// `expected` = `PartitionDetail::disk_unique_id` vu par l'UI (ignoré si vide).
/// Définit `$__d` (le disque) pour la suite du script.
fn disk_identity_guard(disk_index: u32, expected: &Option<String>) -> String {
    let id = expected.as_deref().unwrap_or("").replace('\'', "''");
    format!(
        "$__d = Get-Disk -Number {disk_index} -ErrorAction Stop\n\
         if ('{id}' -ne '' -and [string]$__d.UniqueId -ne '{id}') {{ throw \"Le disque {disk_index} n'est plus celui affiché (disque débranché ou renuméroté). Actualisez la liste.\" }}\n"
    )
}

pub fn format_partition(letter: String, fs: String, label: String, expected_disk_id: Option<String>) -> Result<String, String> {
    let clean = letter.trim().trim_end_matches(':').to_uppercase();
    // Bloquer C: et toute lettre de plus d'un caractère (invalide)
    if clean == "C" || clean.len() != 1 || !clean.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false) {
        return Err("Lettre de lecteur invalide ou formatage du lecteur système interdit.".into());
    }
    // Valider le système de fichiers (case-insensitive) et normaliser vers la forme canonique PowerShell
    let fs_canonical = match fs.to_uppercase().as_str() {
        "NTFS"  => "NTFS",
        "FAT32" => "FAT32",
        "EXFAT" => "exFAT",
        "REFS"  => "ReFS",
        _ => return Err(format!("Système de fichiers non autorisé: {}", fs)),
    };
    // Limiter le label à 32 caractères (limite NTFS) — chars() évite le panic sur UTF-8 multi-octets
    let label: String = label.chars().take(32).collect();
    let id = expected_disk_id.as_deref().unwrap_or("").replace('\'', "''");

    // Vérification : refuser les partitions système, boot, EFI ou Recovery
    // avant tout formatage destructif, et revérifier que la lettre est
    // toujours sur le disque affiché. FAIL-CLOSED : si la garde ne peut pas
    // vérifier (Get-Partition en erreur, WMI KO, pas d'admin), on REFUSE.
    let check_ps = format!(
        r#"try {{
  $p = Get-Partition -DriveLetter '{letter}' -ErrorAction Stop
  if ('{id}' -ne '' -and [string](Get-Disk -Number $p.DiskNumber -ErrorAction Stop).UniqueId -ne '{id}') {{
    Write-Output "ERR:le lecteur {letter}: n'est plus sur le disque affiché (disque débranché ou lettre réattribuée), actualisez la liste"; exit
  }}
  if ($p.IsSystem -or $p.IsBoot) {{ Write-Output 'SYSTEM'; exit }}
  $vol = $p | Get-Volume -ErrorAction SilentlyContinue
  if ($vol) {{
    $t = [string]$p.Type
    if ($t -eq 'System' -or $t -eq 'Recovery' -or $vol.FileSystem -eq 'FAT32' -and $vol.Size -lt 1GB -and $p.IsActive) {{
      Write-Output 'SYSTEM'; exit
    }}
  }}
  Write-Output 'OK'
}} catch {{ Write-Output "ERR:$($_.Exception.Message)" }}"#,
        letter = clean
    );
    let guard = run_ps_cmd(&check_ps, TIMEOUT_QUERY).unwrap_or_else(|e| format!("ERR:{}", e));
    let guard = guard.trim();
    if guard == "SYSTEM" {
        return Err(format!(
            "Formatage interdit : le lecteur {}:\\ est une partition système, boot, EFI ou Recovery.",
            clean
        ));
    }
    if guard != "OK" {
        let detail = guard.strip_prefix("ERR:").unwrap_or(guard);
        return Err(format!(
            "Formatage refusé : impossible de vérifier la partition {}:\\ ({}).",
            clean,
            if detail.is_empty() { "vérification indisponible" } else { detail }
        ));
    }

    let ps = format!(
        r#"Format-Volume -DriveLetter '{}' -FileSystem '{}' -NewFileSystemLabel '{}' -Confirm:$false -Force -ErrorAction Stop | Out-Null; 'OK'"#,
        clean,
        fs_canonical,
        label.replace('\'', "''")
    );
    run_ps_cmd(&ps, TIMEOUT_OP)
}

pub fn assign_drive_letter(disk_index: u32, part_index: u32, letter: String, expected_disk_id: Option<String>) -> Result<String, String> {
    let clean = letter.trim_end_matches(':').to_uppercase();
    if clean.len() != 1 || !clean.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false) {
        return Err("Lettre invalide (A-Z uniquement).".into());
    }
    let ps = format!(
        r#"{}Set-Partition -DiskNumber {} -PartitionNumber {} -NewDriveLetter '{}' -ErrorAction Stop; 'OK'"#,
        disk_identity_guard(disk_index, &expected_disk_id), disk_index, part_index, clean
    );
    run_ps_cmd(&ps, TIMEOUT_QUERY)
}

pub fn create_partition(disk_index: u32, size_mb: Option<u32>, expected_disk_id: Option<String>) -> Result<String, String> {
    let size_arg = match size_mb {
        Some(mb) if mb > 0 => format!("-Size {}MB", mb),
        _ => "-UseMaximumSize".to_string(),
    };
    let ps = format!(
        r#"{}New-Partition -DiskNumber {} {} -AssignDriveLetter -ErrorAction Stop | Format-Volume -FileSystem NTFS -NewFileSystemLabel 'Nouveau volume' -Confirm:$false -Force -ErrorAction Stop | Out-Null; 'OK'"#,
        disk_identity_guard(disk_index, &expected_disk_id), disk_index, size_arg
    );
    run_ps_cmd(&ps, TIMEOUT_OP)
}

/// Types de partition jamais supprimables depuis l'app : EFI (« System »),
/// MSR (« Reserved ») et environnement de récupération (« Recovery », aussi
/// reconnu par son GUID GPT) — ni IsSystem ni IsBoot ne couvrent Recovery/MSR.
fn delete_partition_script(disk_index: u32, part_index: u32, expected_disk_id: &Option<String>) -> String {
    format!(
        r#"{guard}$p = Get-Partition -DiskNumber {di} -PartitionNumber {pi} -ErrorAction Stop
if ($p.IsSystem -or $p.IsBoot) {{ throw 'Suppression interdite : partition système ou de démarrage.' }}
if ([string]$p.Type -in @('System','Reserved','Recovery') -or [string]$p.GptType -eq '{{de94bba4-06d1-4d40-a16a-bfd50179d6ac}}') {{ throw "Suppression interdite : partition $($p.Type) (EFI, MSR ou récupération Windows)." }}
Remove-Partition -DiskNumber {di} -PartitionNumber {pi} -Confirm:$false -ErrorAction Stop; 'OK'"#,
        guard = disk_identity_guard(disk_index, expected_disk_id), di = disk_index, pi = part_index
    )
}

pub fn delete_partition(disk_index: u32, part_index: u32, expected_disk_id: Option<String>) -> Result<String, String> {
    run_ps_cmd(&delete_partition_script(disk_index, part_index, &expected_disk_id), TIMEOUT_OP)
}

pub fn initialize_disk(disk_index: u32, style: String, expected_disk_id: Option<String>) -> Result<String, String> {
    let ps_style = if style.eq_ignore_ascii_case("MBR") { "MBR" } else { "GPT" };
    // Initialize-Disk n'agit que sur un disque RAW : on le dit clairement et on
    // refuse explicitement le disque système (message clair plutôt que l'erreur
    // CIM brute).
    let ps = format!(
        r#"{}if ($__d.IsBoot -or $__d.IsSystem) {{ throw 'Initialisation interdite : disque système.' }}
if ([string]$__d.PartitionStyle -ne 'RAW') {{ throw "Le disque {} est déjà initialisé ($($__d.PartitionStyle)) : supprimez d'abord ses partitions ou utilisez diskpart clean en connaissance de cause." }}
Initialize-Disk -Number {} -PartitionStyle {} -Confirm:$false -ErrorAction Stop; 'OK'"#,
        disk_identity_guard(disk_index, &expected_disk_id), disk_index, disk_index, ps_style
    );
    run_ps_cmd(&ps, TIMEOUT_OP)
}

// ── Resize Partition ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Default)]
pub struct PartitionSizeLimits {
    pub min_bytes: u64,
    pub max_bytes: u64,
    pub current_bytes: u64,
}

pub fn get_partition_resize_limits(disk_index: u32, part_index: u32) -> Result<PartitionSizeLimits, String> {
    let ps = format!(
        r#"$s = Get-PartitionSupportedSize -DiskNumber {di} -PartitionNumber {pi} -ErrorAction Stop
$p = Get-Partition -DiskNumber {di} -PartitionNumber {pi} -ErrorAction Stop
@{{ min_bytes = [string]$s.SizeMin; max_bytes = [string]$s.SizeMax; current_bytes = [string]$p.Size }} | ConvertTo-Json -Compress"#,
        di = disk_index, pi = part_index
    );
    let out = run_ps_cmd(&ps, TIMEOUT_QUERY)?;
    let v: serde_json::Value = serde_json::from_str(out.trim()).map_err(|e| e.to_string())?;
    Ok(PartitionSizeLimits {
        min_bytes:     v["min_bytes"].as_str().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0),
        max_bytes:     v["max_bytes"].as_str().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0),
        current_bytes: v["current_bytes"].as_str().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0),
    })
}

pub fn resize_partition_ps(disk_index: u32, part_index: u32, new_size_mb: u64, expected_disk_id: Option<String>) -> Result<String, String> {
    if new_size_mb == 0 {
        return Err("Taille invalide (0 MB).".into());
    }
    let ps = format!(
        r#"{}Resize-Partition -DiskNumber {} -PartitionNumber {} -Size {}MB -Confirm:$false -ErrorAction Stop; 'OK'"#,
        disk_identity_guard(disk_index, &expected_disk_id), disk_index, part_index, new_size_mb
    );
    run_ps_cmd(&ps, TIMEOUT_RESIZE)
}

// ── Backup / Restore MBR (512 premiers octets du disque physique) ──────────────

pub fn backup_mbr(disk_index: u32, output_path: String) -> Result<String, String> {
    #[cfg(not(target_os = "windows"))]
    return Err("Windows uniquement.".into());

    #[cfg(target_os = "windows")]
    {
        use std::fs::OpenOptions;
        use std::os::windows::fs::OpenOptionsExt;
        use std::io::Read;

        let disk_path = format!(r"\\.\PhysicalDrive{}", disk_index);
        let mut f = OpenOptions::new()
            .read(true)
            .share_mode(3)
            .custom_flags(0x08000000) // FILE_FLAG_SEQUENTIAL_SCAN
            .open(&disk_path)
            .map_err(|e| format!("Ouverture disque impossible: {} (admin requis)", e))?;

        let mut mbr = [0u8; 512];
        f.read_exact(&mut mbr).map_err(|e| format!("Lecture MBR échouée: {}", e))?;

        std::fs::write(&output_path, mbr)
            .map_err(|e| format!("Écriture fichier MBR: {}", e))?;

        // Vérifie signature MBR (0x55AA en fin de secteur)
        let sig = u16::from_le_bytes([mbr[510], mbr[511]]);
        let sig_str = if sig == 0xAA55 { "valide (0x55AA)" } else { "non-standard" };
        Ok(format!("MBR sauvegardé ({} octets, signature {}) → {}", mbr.len(), sig_str, output_path))
    }
}

/// Règles de restauration MBR (testables sans disque). `style` = Get-Disk
/// PartitionStyle, `is_system` = disque de démarrage/système.
fn mbr_restore_allowed(style: &str, is_system: bool, mbr: &[u8]) -> Result<(), String> {
    if is_system {
        return Err("Restauration refusée : c'est le disque système en cours d'utilisation. Faites-la depuis un environnement de secours (WinPE).".into());
    }
    if style.eq_ignore_ascii_case("GPT") && mbr.get(450) != Some(&0xEE) {
        return Err("Restauration refusée : le disque est GPT et ce fichier n'est pas un MBR protecteur GPT (type 0xEE).".into());
    }
    Ok(())
}

pub fn restore_mbr(disk_index: u32, mbr_path: String, expected_disk_id: Option<String>) -> Result<String, String> {
    #[cfg(not(target_os = "windows"))]
    return Err("Windows uniquement.".into());

    #[cfg(target_os = "windows")]
    {
        use std::fs::OpenOptions;
        use std::os::windows::fs::OpenOptionsExt;
        use std::io::Write;

        let mbr_data = std::fs::read(&mbr_path)
            .map_err(|e| format!("Lecture fichier MBR: {}", e))?;

        // Validation taille : exactement 512 octets (secteur MBR complet).
        // 446 octets (code boot seul) est rejeté : écrire 446 octets puis [..512]
        // causerait un panic, et restaurer un MBR partiel corrompt la table de partitions.
        if mbr_data.len() != 512 {
            return Err(format!(
                "Fichier MBR invalide ({} octets, exactement 512 attendus).",
                mbr_data.len()
            ));
        }

        // Validation signature MBR : bytes 510-511 doivent être 0x55 0xAA
        let sig = u16::from_le_bytes([mbr_data[510], mbr_data[511]]);
        if sig != 0xAA55 {
            return Err(format!(
                "Fichier MBR invalide : signature boot absente (attendu 0x55AA, trouvé 0x{:04X}).",
                sig
            ));
        }

        // Garde-fous avant d'écrire le secteur 0 (fail-closed) : identité du
        // disque revérifiée, disque système refusé (Windows en cours d'exécution
        // dessus), et sur un disque GPT seul un MBR protecteur (type 0xEE) est
        // accepté — un MBR classique écrasé sur un disque GPT le ferait passer
        // pour un disque MBR et masquerait toutes ses partitions.
        let facts = run_ps_cmd(
            &format!(
                "{}@{{ style = [string]$__d.PartitionStyle; sys = [bool]($__d.IsBoot -or $__d.IsSystem) }} | ConvertTo-Json -Compress",
                disk_identity_guard(disk_index, &expected_disk_id)
            ),
            TIMEOUT_QUERY,
        )?;
        let facts: serde_json::Value = serde_json::from_str(facts.trim())
            .map_err(|_| "Restauration refusée : impossible de vérifier le disque cible.".to_string())?;
        mbr_restore_allowed(
            facts["style"].as_str().unwrap_or(""),
            facts["sys"].as_bool().unwrap_or(true),
            &mbr_data,
        )?;

        let disk_path = format!(r"\\.\PhysicalDrive{}", disk_index);
        let mut f = OpenOptions::new()
            .write(true)
            .share_mode(3)
            .custom_flags(0x08000000)
            .open(&disk_path)
            .map_err(|e| format!("Ouverture disque en écriture impossible: {} (admin requis)", e))?;

        f.write_all(&mbr_data[..512])
            .map_err(|e| format!("Écriture MBR échouée: {}", e))?;

        Ok(format!("MBR restauré depuis {} → Disque physique {}.", mbr_path, disk_index))
    }
}

// ── Scan partitions perdues ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Default)]
pub struct LostPartition {
    pub disk_index: u32,
    pub offset_bytes: u64,
    pub size_bytes: u64,
    pub signature: String,
    pub fs_hint: String,
    pub description: String,
}

pub fn scan_lost_partitions(disk_index: u32) -> Vec<LostPartition> {
    // Étape 1 — via PowerShell : chercher espace non alloué et résidus de partition
    // OutputEncoding UTF-8 : les descriptions FR accentuées (« Région non
    // allouée », « Partition étendue »…) seraient sinon mojibake sur Windows FR.
    let ps = format!(r#"
$OutputEncoding = [System.Text.Encoding]::UTF8; [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
try {{
    $disk = Get-Disk -Number {di} -ErrorAction Stop
    $parts = @(Get-Partition -DiskNumber {di} -ErrorAction SilentlyContinue)
    $allocated = 0
    foreach ($p in $parts) {{ $allocated += [long]$p.Size }}
    $free = [long]$disk.Size - $allocated
    $result = @()
    if ($free -gt 10MB) {{
        $result += @{{
            disk_index   = [int]{di}
            offset_bytes = [string]$allocated
            size_bytes   = [string]$free
            signature    = 'UNALLOCATED'
            fs_hint      = 'Espace non alloué'
            description  = "Région non allouée de $([math]::Round($free / 1GB, 1)) GB détectée"
        }}
    }}
    # Cherche partitions cachées via Get-Disk PartitionStyle
    if ($disk.PartitionStyle -eq 'MBR') {{
        # MBR peut avoir des partitions étendues non déclarées
        $extParts = $parts | Where-Object {{ $_.Type -eq 'Extended' }}
        foreach ($ep in $extParts) {{
            $result += @{{
                disk_index   = [int]{di}
                offset_bytes = [string]$ep.Offset
                size_bytes   = [string]$ep.Size
                signature    = 'MBR_EXTENDED'
                fs_hint      = 'Partition étendue MBR'
                description  = "Partition étendue MBR de $([math]::Round($ep.Size/1GB,1)) GB"
            }}
        }}
    }}
    if ($result.Count -eq 0) {{ Write-Output '[]' }}
    elseif ($result.Count -eq 1) {{ Write-Output "[$($result | ConvertTo-Json -Compress -Depth 2)]" }}
    else {{ $result | ConvertTo-Json -Compress -Depth 2 }}
}} catch {{ Write-Output '[]' }}
"#, di = disk_index);

    run_ps(&ps)
        .and_then(|t| {
            let t = t.trim();
            if t.is_empty() || t == "[]" { return Some(vec![]); }
            let j = if t.starts_with('{') { format!("[{}]", t) } else { t.to_string() };
            serde_json::from_str::<Vec<serde_json::Value>>(&j).ok()
        })
        .map(|vals| vals.into_iter().map(|v| {
            LostPartition {
                disk_index:   v["disk_index"].as_u64().unwrap_or(0) as u32,
                offset_bytes: v["offset_bytes"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0),
                size_bytes:   v["size_bytes"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0),
                signature:    v["signature"].as_str().unwrap_or("").to_string(),
                fs_hint:      v["fs_hint"].as_str().unwrap_or("").to_string(),
                description:  v["description"].as_str().unwrap_or("").to_string(),
            }
        }).collect())
        .unwrap_or_default()
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Exécute une action PowerShell avec délai maximal et code de sortie réel.
/// Le script est enveloppé dans try/catch : toute erreur (y compris un `throw`
/// des garde-fous) sort en code 1 avec le seul message, décodé OEM/UTF-8
/// (`from_utf8_lossy` rendait les erreurs PowerShell FR illisibles).
fn run_ps_cmd(script: &str, timeout_secs: u64) -> Result<String, String> {
    let wrapped = format!(
        "$ErrorActionPreference = 'Stop'
try {{
{}
}} catch {{ [Console]::Error.WriteLine($_.Exception.Message); exit 1 }}",
        script
    );
    let out = crate::maintenance::commands::execute_system_command(
        "powershell", &["-NoProfile", "-NonInteractive", "-Command", &wrapped], timeout_secs,
    ).map_err(|e| e.to_string())?;
    if out.success {
        Ok(out.stdout.trim().to_string())
    } else {
        let err = out.stderr.trim().to_string();
        Err(if err.is_empty() { "Échec (droits admin requis?)".into() } else { err })
    }
}

/// Lecture seule (listes de disques/partitions) : délai maximal, sortie décodée.
fn run_ps(script: &str) -> Option<String> {
    crate::maintenance::commands::execute_system_command(
        "powershell", &["-NoProfile", "-NonInteractive", "-Command", script], TIMEOUT_QUERY,
    ).ok().map(|o| o.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_guard_escapes_quotes_and_skips_when_empty() {
        let g = disk_identity_guard(2, &Some(r"USBSTOR\DISK'X".into()));
        assert!(g.contains("Get-Disk -Number 2"));
        assert!(g.contains(r"'USBSTOR\DISK''X'"), "apostrophe doublée : {g}");
        // Sans identifiant attendu, le test PS `'' -ne ''` est faux : pas de refus.
        assert!(disk_identity_guard(0, &None).contains("if ('' -ne ''"));
    }

    #[test]
    fn delete_refuses_recovery_msr_and_efi() {
        let s = delete_partition_script(1, 4, &None);
        assert!(s.contains("'System','Reserved','Recovery'"));
        assert!(s.contains("de94bba4-06d1-4d40-a16a-bfd50179d6ac"));
        assert!(s.find("throw").unwrap() < s.find("Remove-Partition").unwrap());
    }

    #[test]
    fn mbr_restore_rules() {
        let mut mbr = vec![0u8; 512];
        mbr[510] = 0x55; mbr[511] = 0xAA;
        assert!(mbr_restore_allowed("MBR", true, &mbr).is_err(), "disque système refusé");
        assert!(mbr_restore_allowed("MBR", false, &mbr).is_ok());
        assert!(mbr_restore_allowed("GPT", false, &mbr).is_err(), "MBR classique sur GPT refusé");
        mbr[450] = 0xEE;
        assert!(mbr_restore_allowed("GPT", false, &mbr).is_ok(), "MBR protecteur accepté");
    }

    /// Lecture seule : la requête SMART renvoie bien bus et amovibilité.
    #[test]
    #[ignore]
    fn smart_list_reports_bus_type() {
        let d = get_disks_smart();
        println!("{d:#?}");
        assert!(!d.is_empty() && d.iter().all(|x| !x.bus_type.is_empty()));
        let p = get_partition_list();
        assert!(p.iter().all(|x| !x.disk_unique_id.is_empty()));
    }
}
