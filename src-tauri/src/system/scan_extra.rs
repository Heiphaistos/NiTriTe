use serde::Serialize;
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

// ============================================================================
// Structs
// ============================================================================

#[derive(Debug, Clone, Serialize, Default)]
pub struct TopProcess {
    pub name: String,
    pub pid: u32,
    pub value: f64, // CPU% ou RAM MB
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct SuspTask {
    pub name: String,
    pub path: String,
    pub exec: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct WmiSubscriptionDetail {
    pub name: String,
    pub consumer_type: String,
    pub path: String,
}

#[derive(Debug, Default)]
pub struct ScanExtra {
    // Sécurité système
    pub tpm_present: bool,
    pub tpm_enabled: bool,
    pub tpm_version: String,
    pub secure_boot: bool,
    pub uac_level: String,
    pub rdp_enabled: bool,
    pub smbv1_enabled: bool,
    pub wmi_subscriptions: u32,
    pub wmi_subscription_details: Vec<WmiSubscriptionDetail>,
    // Comptes
    pub local_admins: Vec<String>,
    pub guest_enabled: bool,
    // Hardware identité
    pub system_manufacturer: String,
    pub system_model: String,
    pub system_serial: String,
    pub bios_manufacturer: String,
    pub bios_version: String,
    pub bios_date: String,
    pub license_type: String,
    // Restauration + maintenance
    pub last_restore_point: String,
    pub pending_updates_cached: i32,
    // Performances snapshot
    pub top_cpu: Vec<TopProcess>,
    pub top_ram: Vec<TopProcess>,
    // Tâches planifiées suspectes
    pub susp_tasks_count: u32,
    pub susp_tasks: Vec<SuspTask>,
    pub windows_updates_pending: Vec<String>,
    pub activation_type: String,
    pub office_activation_type: String,
}

// ============================================================================
// Collecte PowerShell
// ============================================================================

pub fn collect_scan_extra() -> ScanExtra {
    let ps = r#"
# Sortie UTF-8 : libellés accentués codés en dur (« Désactivé (risque ⚠) »,
# « Élever… », « Défaut… », « Aucun point trouvé ») → sinon mojibake côté Rust.
$OutputEncoding = [System.Text.Encoding]::UTF8; [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$out = @{}

# === TPM ===
try {
    $tpm = Get-Tpm -ErrorAction SilentlyContinue
    $out.TpmPresent = [bool]$tpm.TpmPresent
    $out.TpmEnabled = [bool]$tpm.TpmEnabled
    $spec = [string]$tpm.SpecVersion
    $tpmVer = if ($spec -match '2\.0') { '2.0' } elseif ($spec -match '1\.2') { '1.2' } elseif ($spec -and $spec.Length -gt 0) { ($spec -split ',')[0].Trim() } else { [string]$tpm.ManufacturerVersion }
    $out.TpmVersion = if ($tpmVer -and $tpmVer.Length -gt 0) { $tpmVer } else { "N/A" }
} catch { $out.TpmPresent = $false; $out.TpmEnabled = $false; $out.TpmVersion = "N/A" }

# === Secure Boot ===
try {
    $out.SecureBoot = [bool](Confirm-SecureBootUEFI -ErrorAction SilentlyContinue 2>$null)
} catch { $out.SecureBoot = $false }

# === UAC ===
try {
    $uac = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System' -ErrorAction SilentlyContinue).ConsentPromptBehaviorAdmin
    $out.UacLevel = switch ([int]$uac) {
        0 { "Désactivé (risque ⚠)" } 1 { "Élever sans prompt (niveau 1)" }
        2 { "Demander toujours (niveau 2)" } 5 { "Défaut — apps tierces (niveau 5)" }
        default { "Niveau $uac" }
    }
} catch { $out.UacLevel = "Inconnu" }

# === RDP ===
try {
    $rdp = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Terminal Server' -ErrorAction SilentlyContinue).fDenyTSConnections
    $out.RdpEnabled = [bool]($rdp -eq 0)
} catch { $out.RdpEnabled = $false }

# === SMBv1 ===
try {
    $smb1 = (Get-SmbServerConfiguration -ErrorAction SilentlyContinue).EnableSMB1Protocol
    $out.SmbV1 = [bool]$smb1
} catch {
    try {
        $f = Get-WindowsOptionalFeature -Online -FeatureName SMB1Protocol -ErrorAction SilentlyContinue
        $out.SmbV1 = [bool]($f.State -eq 'Enabled')
    } catch { $out.SmbV1 = $false }
}

# === Comptes locaux administrateurs ===
try {
    $out.LocalAdmins = @(Get-LocalGroupMember -Group "Administrators" -ErrorAction SilentlyContinue |
        Select-Object -ExpandProperty Name | ForEach-Object { [string]$_ })
} catch { $out.LocalAdmins = @() }

# === Compte Invité ===
try {
    $out.GuestEnabled = [bool](Get-LocalUser -Name "Guest" -ErrorAction SilentlyContinue).Enabled
} catch { $out.GuestEnabled = $false }

# === BIOS + Système ===
try {
    $bios = Get-WmiObject Win32_BIOS -ErrorAction SilentlyContinue | Select-Object -First 1
    $out.BiosMfr  = [string]$bios.Manufacturer
    $out.BiosVer  = [string]$bios.SMBIOSBIOSVersion
    $out.BiosDate = if ($bios.ReleaseDate) { $bios.ReleaseDate.Substring(0,[Math]::Min(8,$bios.ReleaseDate.Length)) } else { "" }
    $cs = Get-WmiObject Win32_ComputerSystem -ErrorAction SilentlyContinue | Select-Object -First 1
    $out.SysMfr   = [string]$cs.Manufacturer
    $out.SysModel = [string]$cs.Model
    $bb = Get-WmiObject Win32_BaseBoard -ErrorAction SilentlyContinue | Select-Object -First 1
    $out.SysSerial = if ($bb.SerialNumber -and $bb.SerialNumber -ne 'Default string') { [string]$bb.SerialNumber } else { "N/A" }
} catch { $out.BiosMfr=""; $out.BiosVer=""; $out.BiosDate=""; $out.SysMfr=""; $out.SysModel=""; $out.SysSerial="N/A" }

# === Type de licence Windows ===
try {
    $desc = (Get-WmiObject -Query "SELECT Description FROM SoftwareLicensingProduct WHERE PartialProductKey IS NOT NULL AND Name LIKE '*Windows*'" -ErrorAction SilentlyContinue | Select-Object -First 1).Description
    $lt = [string]$desc
    $out.LicenseType = if ($lt -match 'OEM') { 'OEM' } elseif ($lt -match 'Retail|RETAIL') { 'Retail' } elseif ($lt -match 'Volume|KMS|MAK') { 'Volume/KMS' } else { if ($lt.Length -gt 40) { $lt.Substring(0,40) } else { $lt } }
} catch { $out.LicenseType = "" }

# === Type d'activation enrichi (OEM / Retail / KMS / MAS) ===
try {
    $lt = [string]$out.LicenseType
    $kms = [string](Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\SoftwareProtectionPlatform' -ErrorAction SilentlyContinue).KeyManagementServiceName
    $actStatus = [int](Get-WmiObject -Query "SELECT LicenseStatus FROM SoftwareLicensingProduct WHERE PartialProductKey IS NOT NULL AND Name LIKE '*Windows*'" -ErrorAction SilentlyContinue | Select-Object -First 1).LicenseStatus
    $out.ActivationType = if ($actStatus -ne 1) { 'Non activé' }
        elseif ($lt -match 'OEM') { 'OEM — lié à la carte mère' }
        elseif ($lt -match 'Retail') { 'Retail — clé officielle' }
        elseif ($lt -match 'Volume|KMS') {
            if ($kms -match 'localhost|127\.|0\.0\.0\.0') { 'KMS local (MAS / activateur tiers)' }
            elseif ($kms -and $kms.Length -gt 0) { "Volume/KMS — $kms" }
            else { 'Volume/KMS' }
        }
        else { 'Activé' }
} catch { $out.ActivationType = '' }
# === Type activation Office ===
try {
    $offKms = [string](Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Office\16.0\Registration' -ErrorAction SilentlyContinue).KeyManagementServiceName
    if (-not $offKms) { $offKms = [string](Get-ItemProperty 'HKLM:\SOFTWARE\Wow6432Node\Microsoft\Office\16.0\Registration' -ErrorAction SilentlyContinue).KeyManagementServiceName }
    $offStatus = [int](Get-WmiObject -Query "SELECT LicenseStatus FROM SoftwareLicensingProduct WHERE PartialProductKey IS NOT NULL AND Name LIKE '*Office*'" -ErrorAction SilentlyContinue | Select-Object -First 1).LicenseStatus
    $out.OfficeActType = if ($offStatus -ne 1) { 'Non activé' }
        elseif ($offKms -match 'localhost|127\.|0\.0\.0\.0') { 'KMS local (MAS / activateur tiers)' }
        elseif ($offKms -and $offKms.Length -gt 0) { "Volume/KMS — $offKms" }
        else { '' }
} catch { $out.OfficeActType = '' }

# === Dernier point de restauration ===
try {
    $rp = Get-ComputerRestorePoint -ErrorAction SilentlyContinue | Sort-Object CreationTime -Descending | Select-Object -First 1
    $out.LastRestore = if ($rp) { $rp.CreationTime.ToString('yyyy-MM-dd HH:mm') } else { "Aucun point trouvé" }
} catch { $out.LastRestore = "N/A" }

# === WMI Subscriptions (indicateur malware) ===
try {
    $consumers = Get-WmiObject -Namespace root\subscription -Class __EventConsumer -ErrorAction SilentlyContinue
    $out.WmiSubs = ([int]($consumers | Measure-Object).Count)
    $out.WmiSubsList = @($consumers | ForEach-Object {
        @{name=[string]$_.Name; type=[string]$_.__CLASS; path=[string]$_.__PATH}
    })
} catch { $out.WmiSubs = 0; $out.WmiSubsList = @() }

# === Mises à jour en cache (rapide, sans connexion) ===
try {
    $sess = New-Object -ComObject Microsoft.Update.Session -ErrorAction SilentlyContinue
    $sr = $sess.CreateUpdateSearcher()
    $sr.Online = $false
    $res = $sr.Search("IsInstalled=0 and Type='Software'")
    $out.PendingUpd = [int]$res.Updates.Count
    $out.PendingUpdList = @($res.Updates | Select-Object -First 20 | ForEach-Object { [string]$_.Title })
} catch { $out.PendingUpd = -1; $out.PendingUpdList = @() }

# === Top 5 CPU ===
try {
    $out.TopCpu = @(Get-Process -ErrorAction SilentlyContinue | Sort-Object CPU -Descending | Select-Object -First 5 |
        ForEach-Object { @{name=$_.ProcessName; pid=[int]$_.Id; val=[math]::Round($_.CPU,1)} })
} catch { $out.TopCpu = @() }

# === Top 5 RAM ===
try {
    $out.TopRam = @(Get-Process -ErrorAction SilentlyContinue | Sort-Object WorkingSet64 -Descending | Select-Object -First 5 |
        ForEach-Object { @{name=$_.ProcessName; pid=[int]$_.Id; val=[math]::Round($_.WorkingSet64/1MB,0)} })
} catch { $out.TopRam = @() }

# === Tâches planifiées suspectes ===
try {
    $tasks = Get-ScheduledTask -ErrorAction SilentlyContinue | Where-Object {
        $_.State -ne 'Disabled' -and $_.TaskPath -notmatch 'Microsoft\\Windows' -and
        ($_.Actions | Where-Object { $_ -and $_.Execute -and $_.Execute -notmatch 'System32|SysWOW64|Program Files' })
    } | Select-Object -First 15
    $out.SuspTasks = [int]($tasks | Measure-Object).Count
    $out.SuspTasksList = @($tasks | ForEach-Object {
        $exec = if ($_.Actions -and $_.Actions.Count -gt 0) { [string]$_.Actions[0].Execute } else { "" }
        @{name=$_.TaskName; path=$_.TaskPath; exec=$exec}
    })
} catch { $out.SuspTasks = 0; $out.SuspTasksList = @() }

$out | ConvertTo-Json -Depth 3 -Compress
"#;

    #[cfg(target_os = "windows")]
    {
        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", ps])
            .creation_flags(0x08000000)
            .output();

        if let Ok(o) = output {
            let text = crate::maintenance::commands::decode_output(&o.stdout);
            let v: serde_json::Value = match serde_json::from_str(text.trim()) {
                Ok(val) => val,
                Err(_) => return ScanExtra::default(),
            };

            let local_admins = v["LocalAdmins"].as_array()
                .map(|a| a.iter().filter_map(|s| s.as_str().map(|x| x.to_string())).collect())
                .unwrap_or_default();

            let top_cpu = v["TopCpu"].as_array()
                .map(|a| a.iter().map(|p| TopProcess {
                    name: p["name"].as_str().unwrap_or("").to_string(),
                    pid: p["pid"].as_u64().unwrap_or(0) as u32,
                    value: p["val"].as_f64().unwrap_or(0.0),
                }).collect())
                .unwrap_or_default();

            let top_ram = v["TopRam"].as_array()
                .map(|a| a.iter().map(|p| TopProcess {
                    name: p["name"].as_str().unwrap_or("").to_string(),
                    pid: p["pid"].as_u64().unwrap_or(0) as u32,
                    value: p["val"].as_f64().unwrap_or(0.0),
                }).collect())
                .unwrap_or_default();

            let susp_tasks = v["SuspTasksList"].as_array()
                .map(|a| a.iter().map(|t| SuspTask {
                    name: t["name"].as_str().unwrap_or("").to_string(),
                    path: t["path"].as_str().unwrap_or("").to_string(),
                    exec: t["exec"].as_str().unwrap_or("").to_string(),
                }).collect())
                .unwrap_or_default();

            let wmi_subscription_details = v["WmiSubsList"].as_array()
                .map(|a| a.iter().map(|s| WmiSubscriptionDetail {
                    name: s["name"].as_str().unwrap_or("").to_string(),
                    consumer_type: s["type"].as_str().unwrap_or("").to_string(),
                    path: s["path"].as_str().unwrap_or("").to_string(),
                }).collect())
                .unwrap_or_default();

            return ScanExtra {
                tpm_present: v["TpmPresent"].as_bool().unwrap_or(false),
                tpm_enabled: v["TpmEnabled"].as_bool().unwrap_or(false),
                tpm_version: v["TpmVersion"].as_str().unwrap_or("N/A").to_string(),
                secure_boot: v["SecureBoot"].as_bool().unwrap_or(false),
                uac_level: v["UacLevel"].as_str().unwrap_or("Inconnu").to_string(),
                rdp_enabled: v["RdpEnabled"].as_bool().unwrap_or(false),
                smbv1_enabled: v["SmbV1"].as_bool().unwrap_or(false),
                wmi_subscriptions: v["WmiSubs"].as_u64().unwrap_or(0) as u32,
                wmi_subscription_details,
                local_admins,
                guest_enabled: v["GuestEnabled"].as_bool().unwrap_or(false),
                system_manufacturer: v["SysMfr"].as_str().unwrap_or("").to_string(),
                system_model: v["SysModel"].as_str().unwrap_or("").to_string(),
                system_serial: v["SysSerial"].as_str().unwrap_or("N/A").to_string(),
                bios_manufacturer: v["BiosMfr"].as_str().unwrap_or("").to_string(),
                bios_version: v["BiosVer"].as_str().unwrap_or("").to_string(),
                bios_date: v["BiosDate"].as_str().unwrap_or("").to_string(),
                license_type: v["LicenseType"].as_str().unwrap_or("").to_string(),
                last_restore_point: v["LastRestore"].as_str().unwrap_or("N/A").to_string(),
                pending_updates_cached: v["PendingUpd"].as_i64().unwrap_or(-1) as i32,
                top_cpu,
                top_ram,
                susp_tasks_count: v["SuspTasks"].as_u64().unwrap_or(0) as u32,
                susp_tasks,
                windows_updates_pending: v["PendingUpdList"].as_array()
                    .map(|a| a.iter().filter_map(|s| s.as_str().map(|x| x.to_string())).collect())
                    .unwrap_or_default(),
                activation_type: v["ActivationType"].as_str().unwrap_or("").to_string(),
                office_activation_type: v["OfficeActType"].as_str().unwrap_or("").to_string(),
            };
        }
    }
    ScanExtra::default()
}
