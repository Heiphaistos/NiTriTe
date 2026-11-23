use serde::Serialize;
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[derive(Debug, Serialize, Clone)]
pub struct SecurityStatus {
    pub secure_boot: String,
    pub tpm_version: String,
    pub tpm_enabled: bool,
    pub tpm_activated: bool,
    pub uac_level: String,
    pub uac_enabled: bool,
    pub bitlocker_drives: Vec<BitLockerDrive>,
    pub windows_hello: bool,
    pub smartscreen_enabled: bool,
    pub credential_guard: bool,
    pub hvci_enabled: bool,
    pub vbs_enabled: bool,
    pub lsa_protection: bool,
    pub firewall_domain: bool,
    pub firewall_private: bool,
    pub firewall_public: bool,
    pub antivirus_name: String,
    pub antivirus_state: String,
    pub windows_defender_realtime: bool,
    pub defender_definitions_date: String,
    pub last_full_scan: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct BitLockerDrive {
    pub drive_letter: String,
    pub protection_status: String,
    pub encryption_method: String,
    pub conversion_status: String,
    pub lock_status: String,
}

#[cfg(target_os = "windows")]
fn run_ps(script: &str) -> String {
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(0x08000000)
        .output();
    match out {
        Ok(o) => crate::maintenance::commands::decode_output(&o.stdout).trim().to_string(),
        Err(_) => String::new(),
    }
}

pub fn collect_security_status() -> SecurityStatus {
    #[cfg(target_os = "windows")]
    {
        // PowerShell mega-query
        let ps = r#"
# Sortie UTF-8 : ce script renvoie des libellés FR accentués (« Activé »,
# « désactivé », « Non à jour »…) ; sans ça run_ps (from_utf8_lossy) les
# recevrait en codepage OEM → mojibake dans la vue Sécurité.
$OutputEncoding = [System.Text.Encoding]::UTF8; [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$out = @{}

# Secure Boot
try { $out.SecureBoot = if (Confirm-SecureBootUEFI -ErrorAction Stop) { "Activé" } else { "Désactivé" } }
catch { $out.SecureBoot = "Non supporté / BIOS Legacy" }

# TPM
try {
    $tpm = Get-Tpm -ErrorAction Stop
    $out.TpmPresent = $tpm.TpmPresent
    $out.TpmEnabled = $tpm.TpmEnabled
    $out.TpmActivated = $tpm.TpmActivated
    $tpmWmi = Get-WmiObject -Namespace "root\cimv2\security\microsofttpm" -Class Win32_Tpm -ErrorAction SilentlyContinue
    $out.TpmVersion = if ($tpmWmi) { $tpmWmi.SpecVersion } else { "N/A" }
} catch {
    $out.TpmPresent = $false; $out.TpmEnabled = $false; $out.TpmActivated = $false; $out.TpmVersion = "N/A"
}

# UAC
$uacKey = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System"
$uacVal = (Get-ItemProperty -Path $uacKey -ErrorAction SilentlyContinue).EnableLUA
$uacCon = (Get-ItemProperty -Path $uacKey -ErrorAction SilentlyContinue).ConsentPromptBehaviorAdmin
$out.UacEnabled = ($uacVal -eq 1)
$out.UacLevel = switch ($uacCon) {
    0 { "Jamais notifier (désactivé)" }
    1 { "Élever sans interface" }
    2 { "Élever avec interface" }
    3 { "Notifier (niveau intermédiaire)" }
    4 { "Notifier (classique)" }
    5 { "Notifier uniquement les changements" }
    default { "Inconnu ($uacCon)" }
}

# Credential Guard / VBS / HVCI
try {
    $dg = Get-CimInstance -ClassName Win32_DeviceGuard -Namespace root\Microsoft\Windows\DeviceGuard -ErrorAction SilentlyContinue
    $out.CredentialGuard = ($dg.SecurityServicesRunning -contains 1)
    $out.HvciEnabled     = ($dg.SecurityServicesRunning -contains 2)
    $out.VbsEnabled      = ($dg.VirtualizationBasedSecurityStatus -eq 2)
} catch {
    $out.CredentialGuard = $false; $out.HvciEnabled = $false; $out.VbsEnabled = $false
}

# LSA Protection
try {
    $lsa = (Get-ItemProperty "HKLM:\SYSTEM\CurrentControlSet\Control\Lsa" -ErrorAction SilentlyContinue).RunAsPPL
    $out.LsaProtection = ($lsa -eq 1)
} catch { $out.LsaProtection = $false }

# Firewall
try {
    $fw = Get-NetFirewallProfile -ErrorAction Stop
    $out.FwDomain  = ($fw | Where-Object { $_.Name -eq 'Domain'  } | Select-Object -ExpandProperty Enabled)
    $out.FwPrivate = ($fw | Where-Object { $_.Name -eq 'Private' } | Select-Object -ExpandProperty Enabled)
    $out.FwPublic  = ($fw | Where-Object { $_.Name -eq 'Public'  } | Select-Object -ExpandProperty Enabled)
} catch { $out.FwDomain = $false; $out.FwPrivate = $false; $out.FwPublic = $false }

# Windows Hello
try {
    $hello = Get-WmiObject -Namespace "root\cimv2" -Query "SELECT * FROM Win32_PnPEntity WHERE Name LIKE '%Windows Hello%'" -ErrorAction SilentlyContinue
    $out.WindowsHello = ($null -ne $hello)
} catch { $out.WindowsHello = $false }

# SmartScreen
try {
    $ss = (Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer" -ErrorAction SilentlyContinue).SmartScreenEnabled
    $out.SmartScreen = ($ss -ne "Off" -and $null -ne $ss)
} catch { $out.SmartScreen = $false }

# Antivirus (WSC)
try {
    $av = Get-CimInstance -Namespace "root\SecurityCenter2" -ClassName "AntiVirusProduct" -ErrorAction Stop | Select-Object -First 1
    $out.AvName = if ($av) { $av.displayName } else { "Aucun" }
    $state = if ($av) { $av.productState } else { 0 }
    $hexState = "0x{0:X6}" -f $state
    $defState = [int]("0x" + $hexState.Substring(4, 2))
    $out.AvState = switch ($defState) {
        16 { "À jour" }
        32 { "Non à jour" }
        default { "Inconnu" }
    }
} catch { $out.AvName = "Inconnu"; $out.AvState = "Inconnu" }

# Windows Defender
try {
    $wdPrefs = Get-MpPreference -ErrorAction Stop
    $wdStatus = Get-MpComputerStatus -ErrorAction Stop
    $out.DefenderRealtime = $wdStatus.RealTimeProtectionEnabled
    $out.DefenderDefsDate = $wdStatus.AntivirusSignatureLastUpdated.ToString("dd/MM/yyyy HH:mm")
    $out.DefenderLastScan = if ($wdStatus.FullScanEndTime -ne $null -and $wdStatus.FullScanEndTime.Year -gt 1970) { $wdStatus.FullScanEndTime.ToString("dd/MM/yyyy HH:mm") } else { "Jamais" }
} catch { $out.DefenderRealtime = $false; $out.DefenderDefsDate = "N/A"; $out.DefenderLastScan = "N/A" }

# BitLocker
try {
    $bl = Get-BitLockerVolume -ErrorAction Stop
    $out.BitLocker = @($bl | ForEach-Object {
        @{
            Drive = $_.MountPoint
            ProtectionStatus = $_.ProtectionStatus.ToString()
            EncryptionMethod = $_.EncryptionMethod.ToString()
            ConversionStatus = $_.VolumeStatus.ToString()
            LockStatus = $_.LockStatus.ToString()
        }
    })
} catch {
    try {
        $manage = manage-bde -status 2>$null
        $out.BitLocker = @()
    } catch { $out.BitLocker = @() }
}

$out | ConvertTo-Json -Depth 4 -Compress
"#;
        let raw = run_ps(ps);
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();

        let bl: Vec<BitLockerDrive> = v["BitLocker"].as_array().map(|arr| {
            arr.iter().filter_map(|item| {
                Some(BitLockerDrive {
                    drive_letter: item["Drive"].as_str()?.to_string(),
                    protection_status: item["ProtectionStatus"].as_str().unwrap_or("Unknown").to_string(),
                    encryption_method: item["EncryptionMethod"].as_str().unwrap_or("Unknown").to_string(),
                    conversion_status: item["ConversionStatus"].as_str().unwrap_or("Unknown").to_string(),
                    lock_status: item["LockStatus"].as_str().unwrap_or("Unknown").to_string(),
                })
            }).collect()
        }).unwrap_or_default();

        SecurityStatus {
            secure_boot: v["SecureBoot"].as_str().unwrap_or("Inconnu").to_string(),
            tpm_version: v["TpmVersion"].as_str().unwrap_or("N/A").to_string(),
            tpm_enabled: v["TpmEnabled"].as_bool().unwrap_or(false),
            tpm_activated: v["TpmActivated"].as_bool().unwrap_or(false),
            uac_level: v["UacLevel"].as_str().unwrap_or("Inconnu").to_string(),
            uac_enabled: v["UacEnabled"].as_bool().unwrap_or(false),
            bitlocker_drives: bl,
            windows_hello: v["WindowsHello"].as_bool().unwrap_or(false),
            smartscreen_enabled: v["SmartScreen"].as_bool().unwrap_or(false),
            credential_guard: v["CredentialGuard"].as_bool().unwrap_or(false),
            hvci_enabled: v["HvciEnabled"].as_bool().unwrap_or(false),
            vbs_enabled: v["VbsEnabled"].as_bool().unwrap_or(false),
            lsa_protection: v["LsaProtection"].as_bool().unwrap_or(false),
            firewall_domain: v["FwDomain"].as_bool().unwrap_or(false),
            firewall_private: v["FwPrivate"].as_bool().unwrap_or(false),
            firewall_public: v["FwPublic"].as_bool().unwrap_or(false),
            antivirus_name: v["AvName"].as_str().unwrap_or("Inconnu").to_string(),
            antivirus_state: v["AvState"].as_str().unwrap_or("Inconnu").to_string(),
            windows_defender_realtime: v["DefenderRealtime"].as_bool().unwrap_or(false),
            defender_definitions_date: v["DefenderDefsDate"].as_str().unwrap_or("N/A").to_string(),
            last_full_scan: v["DefenderLastScan"].as_str().unwrap_or("N/A").to_string(),
        }
    }
    #[cfg(not(target_os = "windows"))]
    SecurityStatus {
        secure_boot: "N/A".into(), tpm_version: "N/A".into(),
        tpm_enabled: false, tpm_activated: false,
        uac_level: "N/A".into(), uac_enabled: false,
        bitlocker_drives: vec![], windows_hello: false,
        smartscreen_enabled: false, credential_guard: false,
        hvci_enabled: false, vbs_enabled: false, lsa_protection: false,
        firewall_domain: false, firewall_private: false, firewall_public: false,
        antivirus_name: "N/A".into(), antivirus_state: "N/A".into(),
        windows_defender_realtime: false,
        defender_definitions_date: "N/A".into(), last_full_scan: "N/A".into(),
    }
}
