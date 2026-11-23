//! Fonctions de collecte d'informations système
use crate::error::NiTriTeError;
use super::formatters::*;
use super::{run_ps_temp, OutputTimeout};
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

pub fn collect_installed_apps() -> Result<String, NiTriTeError> {
    let output = Command::new("winget").args(["list", "--accept-source-agreements"]).creation_flags(0x08000000).output_timeout()?;
    Ok(crate::maintenance::commands::decode_output(&output.stdout))
}

pub fn collect_drivers() -> Result<String, NiTriTeError> {
    let script = r#"
$cats = @{
    'Bluetooth'    = 'Bluetooth'
    'Net'          = 'Cartes Reseau'
    'Display'      = 'Affichage / GPU'
    'DiskDrive'    = 'Disques / SSD'
    'USB'          = 'Controleurs USB'
    'AudioEndpoint'= 'Audio'
    'Media'        = 'Multimedia'
    'MEDIA'        = 'Multimedia'
    'HDC'          = 'Controleurs disque'
    'SCSIAdapter'  = 'Controleurs SCSI / NVMe'
    'Mouse'        = 'Souris'
    'Keyboard'     = 'Clavier'
    'HIDClass'     = 'Peripheriques HID'
    'Camera'       = 'Cameras'
    'System'       = 'Systeme'
    'Processor'    = 'Processeur'
    'Battery'      = 'Batterie'
    'Monitor'      = 'Ecrans'
    'PrintQueue'   = 'Imprimantes'
    'Biometric'    = 'Biometrique'
    'UCM'          = 'USB Type-C'
}
$all = Get-WmiObject Win32_PnPSignedDriver -ErrorAction SilentlyContinue |
    Where-Object { $_.DeviceName -and $_.DeviceName.Trim() -ne '' -and $_.DriverVersion } |
    Group-Object DeviceClass | Sort-Object Name

foreach ($group in $all) {
    $cat = if ($cats[$group.Name]) { $cats[$group.Name] }
           elseif ($group.Name)    { $group.Name }
           else                    { 'Autres' }
    Write-Output ""
    Write-Output "=== $cat ($($group.Count) pilote(s)) ==="
    foreach ($drv in ($group.Group | Sort-Object DeviceName)) {
        Write-Output "  Peripherique : $($drv.DeviceName)"
        if ($drv.Manufacturer -and $drv.Manufacturer -ne $drv.DeviceName) {
            Write-Output "  Fabricant    : $($drv.Manufacturer)"
        }
        Write-Output "  Version      : $($drv.DriverVersion)"
        if ($drv.DriverDate -and $drv.DriverDate.Length -ge 8) {
            try {
                $df = [datetime]::ParseExact($drv.DriverDate.Substring(0,8),'yyyyMMdd',$null)
                Write-Output "  Date         : $($df.ToString('dd/MM/yyyy'))"
            } catch {}
        }
        Write-Output ""
    }
}
"#;
    run_ps_temp(script)
}


pub fn collect_network_config() -> Result<String, NiTriTeError> {
    let output = Command::new("ipconfig").arg("/all").creation_flags(0x08000000).output_timeout()?;
    // ipconfig écrit en codepage OEM (CP850 sur Windows FR) : from_utf8_lossy
    // transformerait « Carte réseau », « Passerelle par défaut »… en mojibake
    // dans le fichier de sauvegarde. decode_output gère déjà les deux plateformes.
    Ok(crate::maintenance::commands::decode_output(&output.stdout))
}

pub fn collect_startup() -> Result<String, NiTriTeError> {
    let script = r#"
$items = Get-CimInstance Win32_StartupCommand -ErrorAction SilentlyContinue |
    Sort-Object Location, Name

$prev_loc = ""
foreach ($item in $items) {
    if ($item.Location -ne $prev_loc) {
        Write-Output ""
        Write-Output "=== $($item.Location) ==="
        $prev_loc = $item.Location
    }
    Write-Output "  Nom         : $($item.Name)"
    Write-Output "  Commande    : $($item.Command)"
    Write-Output "  Utilisateur : $($item.User)"
    Write-Output ""
}
"#;
    run_ps_temp(script)
}

pub fn collect_env_vars() -> Result<String, NiTriTeError> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", "[Environment]::GetEnvironmentVariables('Machine') | ConvertTo-Json"])
        .creation_flags(0x08000000).output_timeout()?;
    Ok(crate::maintenance::commands::decode_output(&output.stdout))
}

pub fn collect_firewall_rules() -> Result<String, NiTriTeError> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", "Get-NetFirewallRule | Select-Object -First 50 DisplayName, Direction, Action, Enabled | ConvertTo-Json"])
        .creation_flags(0x08000000).output_timeout()?;
    Ok(crate::maintenance::commands::decode_output(&output.stdout))
}

pub fn collect_browser_bookmarks(browser_subpath: &str) -> Result<String, NiTriTeError> {
    let local_app = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let bookmarks_path = std::path::PathBuf::from(&local_app)
        .join(browser_subpath)
        .join("User Data").join("Default").join("Bookmarks");

    if bookmarks_path.exists() {
        Ok(std::fs::read_to_string(bookmarks_path)?)
    } else {
        Err(NiTriTeError::System("Fichier bookmarks introuvable".into()))
    }
}

pub fn collect_windows_license() -> Result<String, NiTriTeError> {
    // Méthode 1 : clé OEM gravée dans le BIOS/UEFI (laptops OEM)
    let oem_out = Command::new("powershell")
        .args(["-NoProfile", "-Command",
            "$k=(Get-WmiObject SoftwareLicensingService -EA SilentlyContinue).OA3xOriginalProductKey; if($k -and $k.Trim().Length -gt 0){$k.Trim()} else {''}"])
        .creation_flags(0x08000000).output_timeout()?;
    let oem_key = crate::maintenance::commands::decode_output(&oem_out.stdout).trim().to_string();

    // Méthode 2 : décode DigitalProductId depuis le registre (fonctionne retail, volume, KMS/Massgrave)
    let decode_script = concat!(
        "$m='BCDFGHJKMPQRTVWXY2346789';",
        "$d=Get-ItemPropertyValue 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion' -Name DigitalProductId -EA SilentlyContinue;",
        "if($d -and $d.Count -ge 67){",
          "$k=$d[52..66]; $r='';",
          "for($i=24;$i -ge 0;$i--){",
            "$n=0;",
            "for($j=14;$j -ge 0;$j--){$n=$n*256 -bxor $k[$j];$k[$j]=[math]::Floor($n/24);$n=$n%24};",
            "$r=$m[$n]+$r;",
            "if((24-$i)%5 -eq 0 -and $i -ne 0 -and $i -ne 24){$r='-'+$r}",
          "};",
          "$r",
        "}else{''}"
    );
    let reg_out = Command::new("powershell")
        .args(["-NoProfile", "-Command", decode_script])
        .creation_flags(0x08000000).output_timeout()?;
    let reg_key = crate::maintenance::commands::decode_output(&reg_out.stdout).trim().to_string();

    // Méthode 3 : slmgr /dli — statut lisible (fonctionne avec Massgrave HWID/KMS38)
    let slmgr_out = Command::new("cscript")
        .args(["//nologo", "C:\\Windows\\System32\\slmgr.vbs", "/dli"])
        .creation_flags(0x08000000).output_timeout();
    let slmgr = if let Ok(o) = slmgr_out {
        crate::maintenance::commands::decode_output(&o.stdout).trim().to_string()
    } else { String::new() };

    // Méthode 4 : informations WMI (canal, clé partielle)
    let wmi_out = Command::new("powershell")
        .args(["-NoProfile", "-Command",
            "Get-WmiObject SoftwareLicensingProduct -EA SilentlyContinue | Where-Object {$_.PartialProductKey -and $_.Name -like '*Windows*'} | Select-Object -First 1 | ForEach-Object { \"Edition         : \" + $_.Name; \"Cle partielle   : ...\" + $_.PartialProductKey; \"Canal           : \" + $_.LicenseFamily; \"Statut          : \" + $(switch($_.LicenseStatus){0{'Non licence'} 1{'ACTIVE'} 2{'Grace period'} 3{'Modifie (tampered)'} 4{'Notification'} 5{'Grace etendue'} default{'Inconnu'}}) }"])
        .creation_flags(0x08000000).output_timeout()?;
    let wmi = crate::maintenance::commands::decode_output(&wmi_out.stdout).trim().to_string();

    let mut r = String::new();
    r.push_str("╔══════════════════════════════════════╗\n");
    r.push_str("║     LICENCE WINDOWS — INFORMATIONS   ║\n");
    r.push_str("╚══════════════════════════════════════╝\n\n");

    if !oem_key.is_empty() {
        r.push_str(&format!("Cle OEM (BIOS)  : {}\n", oem_key));
    }
    if !reg_key.is_empty() && reg_key != "DigitalProductId introuvable" {
        r.push_str(&format!("Cle registre    : {}\n", reg_key));
    }
    if oem_key.is_empty() && (reg_key.is_empty() || reg_key == "DigitalProductId introuvable") {
        r.push_str("Cle produit     : Non disponible (activation par hardware ID ou KMS)\n");
    }
    if !wmi.is_empty() {
        r.push('\n');
        r.push_str(&wmi);
        r.push('\n');
    }
    if !slmgr.is_empty() {
        r.push_str("\n─── Détail complet (slmgr) ───\n");
        r.push_str(&slmgr);
    }
    Ok(r)
}

pub fn collect_bitlocker_keys() -> Result<String, NiTriTeError> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", "Get-BitLockerVolume -ErrorAction SilentlyContinue | Select-Object MountPoint, VolumeStatus, EncryptionPercentage, KeyProtector | ConvertTo-Json"])
        .creation_flags(0x08000000).output_timeout()?;
    Ok(crate::maintenance::commands::decode_output(&output.stdout))
}

pub fn collect_office_license() -> Result<String, NiTriTeError> {
    // Méthode 1 : DigitalProductId dans les chemins MSI classiques
    let decode_script = concat!(
        "$m='BCDFGHJKMPQRTVWXY2346789';",
        "$paths=@('HKLM:\\SOFTWARE\\Microsoft\\Office\\*\\Registration','HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\Office\\*\\Registration');",
        "$found=$false;",
        "foreach($p in $paths){",
          "Get-ChildItem $p -EA SilentlyContinue | ForEach-Object {",
            "$reg=Get-ItemProperty $_.PSPath -EA SilentlyContinue;",
            "$name=if($reg.ProductName){$reg.ProductName}else{'Microsoft Office'};",
            "$dpid=$reg.DigitalProductID;",
            "if($dpid -and $dpid.Count -ge 67){",
              "$k=$dpid[52..66]; $r='';",
              "for($i=24;$i -ge 0;$i--){",
                "$n=0;",
                "for($j=14;$j -ge 0;$j--){$n=$n*256 -bxor $k[$j];$k[$j]=[math]::Floor($n/24);$n=$n%24};",
                "$r=$m[$n]+$r;",
                "if((24-$i)%5 -eq 0 -and $i -ne 0 -and $i -ne 24){$r='-'+$r}",
              "};",
              "\"Produit  : $name\";\"Cle      : $r\";'';$found=$true",
            "}",
          "}",
        "};",
        "if(-not $found){'(Aucune cle MSI trouvee — voir section Click-to-Run ci-dessous)'}"
    );
    let reg_out = Command::new("powershell")
        .args(["-NoProfile", "-Command", decode_script])
        .creation_flags(0x08000000).output_timeout()?;
    let reg_keys = crate::maintenance::commands::decode_output(&reg_out.stdout).trim().to_string();

    // Méthode 2 : Click-to-Run / Office 365 / Massgrave
    let c2r_out = Command::new("powershell")
        .args(["-NoProfile", "-Command", concat!(
            "$c2r=Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Office\\ClickToRun\\Configuration' -EA SilentlyContinue;",
            "if($c2r){",
              "\"Produit C2R    : \" + $c2r.ProductReleaseIds;",
              "\"Canal          : \" + $c2r.CDNBaseUrl;",
              "\"Version        : \" + $c2r.VersionToReport;",
              "$lic=Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Office\\ClickToRun\\REGISTRY\\MACHINE\\SOFTWARE\\Microsoft\\Office\\16.0\\Common\\Licensing' -EA SilentlyContinue;",
              "if($lic.LastAcknowledgedLicenseToken){'Licence token  : presente (abonnement/KMS actif)'}",
            "}else{'(Office Click-to-Run non detecte)'}"
        )])
        .creation_flags(0x08000000).output_timeout()?;
    let c2r = crate::maintenance::commands::decode_output(&c2r_out.stdout).trim().to_string();

    // Méthode 3 : OSPP.VBS statut d'activation
    let ospp_dirs = [
        r"C:\Program Files\Microsoft Office\Office16",
        r"C:\Program Files (x86)\Microsoft Office\Office16",
        r"C:\Program Files\Microsoft Office\Office15",
    ];
    let mut ospp_status = String::new();
    for dir in &ospp_dirs {
        let vbs = format!("{}\\OSPP.VBS", dir);
        if std::path::Path::new(&vbs).exists() {
            if let Ok(o) = Command::new("cscript")
                .args(["//nologo", &vbs, "/dstatus"])
                .creation_flags(0x08000000).output_timeout()
            {
                let s = crate::maintenance::commands::decode_output(&o.stdout).trim().to_string();
                if !s.is_empty() { ospp_status = s; break; }
            }
        }
    }

    let mut r = String::new();
    r.push_str("╔══════════════════════════════════════╗\n");
    r.push_str("║     LICENCE OFFICE — INFORMATIONS    ║\n");
    r.push_str("╚══════════════════════════════════════╝\n\n");
    r.push_str("─── Clé produit (registre MSI) ───\n");
    r.push_str(&reg_keys);
    r.push_str("\n\n─── Office Click-to-Run / Office 365 ───\n");
    r.push_str(&c2r);
    if !ospp_status.is_empty() {
        r.push_str("\n\n─── Statut d'activation (OSPP) ───\n");
        r.push_str(&ospp_status);
    }
    Ok(r)
}

pub fn collect_installed_fonts() -> Result<String, NiTriTeError> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command",
            "(Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts' | Get-Member -MemberType NoteProperty | Select-Object Name).Name | Sort-Object | ConvertTo-Json"])
        .creation_flags(0x08000000).output_timeout()?;
    Ok(crate::maintenance::commands::decode_output(&output.stdout))
}

pub fn collect_scheduled_tasks() -> Result<String, NiTriTeError> {
    let script = r#"
$tasks = Get-ScheduledTask -ErrorAction SilentlyContinue |
    Where-Object { $_.State -ne 'Disabled' -and $_.TaskPath -notmatch '\\Microsoft\\' } |
    Sort-Object TaskPath, TaskName

$prev_path = ""
foreach ($t in $tasks) {
    $path = if ($t.TaskPath -and $t.TaskPath -ne '\') { $t.TaskPath.TrimEnd('\') } else { "Racine" }
    if ($path -ne $prev_path) {
        Write-Output ""
        Write-Output "=== Dossier : $path ==="
        $prev_path = $path
    }
    $etat = switch ($t.State) {
        'Ready'   { 'Pret' }
        'Running' { 'En cours' }
        'Queued'  { 'En attente' }
        default   { $t.State }
    }
    Write-Output "  Tache       : $($t.TaskName)"
    Write-Output "  Etat        : $etat"
    if ($t.Description -and $t.Description.Trim() -ne '') {
        $desc = $t.Description.Trim() -replace '\r?\n', ' '
        if ($desc.Length -gt 120) { $desc = $desc.Substring(0,120) + '...' }
        Write-Output "  Description : $desc"
    }
    # Déclencheur simplifié
    $trig = $t.Triggers | Select-Object -First 1
    if ($trig) {
        $type = $trig.CimClass.CimClassName -replace 'MSFT_Task','` -replace 'Trigger',''
        Write-Output "  Declencheur : $type"
    }
    Write-Output ""
}
if (-not $tasks) { Write-Output "Aucune tache planifiee active (hors Microsoft) trouvee." }
"#;
    run_ps_temp(script)
}

pub fn collect_windows_features() -> Result<String, NiTriTeError> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command",
            "Get-WindowsOptionalFeature -Online -ErrorAction SilentlyContinue | Where-Object {$_.State -eq 'Enabled'} | Select-Object FeatureName | ConvertTo-Json"])
        .creation_flags(0x08000000).output_timeout()?;
    Ok(crate::maintenance::commands::decode_output(&output.stdout))
}

pub fn collect_folder_sizes() -> Result<String, NiTriTeError> {
    // Sortie directement formatée en Mo/Go — pas de JSON brut
    let script = concat!(
        "Get-ChildItem C:\\ -Directory -EA SilentlyContinue",
        " | ForEach-Object {",
        "   $bytes = (Get-ChildItem $_.FullName -Recurse -EA SilentlyContinue | Measure-Object Length -Sum).Sum;",
        "   $bytes = if($bytes){[long]$bytes}else{0};",
        "   $taille = if($bytes -ge 1073741824){ \"{0:N2} Go\" -f ($bytes/1GB) }",
        "             elseif($bytes -ge 1048576){ \"{0:N0} Mo\" -f ($bytes/1MB) }",
        "             elseif($bytes -ge 1024){ \"{0:N0} Ko\" -f ($bytes/1KB) }",
        "             else{ \"$bytes o\" };",
        "   [PSCustomObject]@{Dossier=$_.Name; Taille=$taille; TailleBytes=$bytes}",
        " }",
        " | Sort-Object TailleBytes -Descending",
        " | Select-Object -First 30",
        " | ForEach-Object { \"{0,-40} {1}\" -f $_.Dossier, $_.Taille }"
    );
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .creation_flags(0x08000000).output_timeout()?;
    Ok(crate::maintenance::commands::decode_output(&output.stdout))
}

pub fn collect_desktop_files() -> Result<String, NiTriTeError> {
    let desktop = std::env::var("USERPROFILE").unwrap_or_default();
    let desktop_path = std::path::PathBuf::from(&desktop).join("Desktop");
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&desktop_path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            files.push(format!("{} ({})", name, format_size(size)));
        }
    }
    Ok(files.join("\n"))
}

pub fn collect_system_components() -> Result<String, NiTriTeError> {
    let script = r#"
# === SYSTEME ===
$cs = Get-WmiObject Win32_ComputerSystem -EA SilentlyContinue
$os = Get-WmiObject Win32_OperatingSystem -EA SilentlyContinue
Write-Output "=== INFORMATIONS SYSTEME ==="
if ($cs) {
    Write-Output "  Fabricant PC    : $($cs.Manufacturer)"
    Write-Output "  Modele PC       : $($cs.Model)"
    Write-Output "  Nom machine     : $($cs.Name)"
}
if ($os) {
    Write-Output "  Systeme         : $($os.Caption) $($os.OSArchitecture)"
    Write-Output "  Build           : $($os.BuildNumber)"
}
Write-Output ""

# === CPU ===
$cpu = Get-WmiObject Win32_Processor -EA SilentlyContinue | Select-Object -First 1
Write-Output "=== PROCESSEUR (CPU) ==="
if ($cpu) {
    Write-Output "  Modele          : $($cpu.Name.Trim())"
    Write-Output "  Coeurs          : $($cpu.NumberOfCores)"
    Write-Output "  Threads         : $($cpu.NumberOfLogicalProcessors)"
    $ghz = [math]::Round($cpu.MaxClockSpeed / 1000.0, 2)
    Write-Output "  Frequence       : $($cpu.MaxClockSpeed) MHz ($ghz GHz)"
    if ($cpu.L2CacheSize -gt 0) {
        $l2 = [math]::Round($cpu.L2CacheSize / 1024.0, 1)
        Write-Output "  Cache L2        : $l2 Mo"
    }
    if ($cpu.L3CacheSize -gt 0) {
        $l3 = [math]::Round($cpu.L3CacheSize / 1024.0, 1)
        Write-Output "  Cache L3        : $l3 Mo"
    }
    Write-Output "  Socket          : $($cpu.SocketDesignation)"
    $arch = switch ($cpu.Architecture) { 0 { 'x86' } 9 { 'x64 (AMD64)' } 12 { 'ARM64' } default { $cpu.Architecture } }
    Write-Output "  Architecture    : $arch"
}
Write-Output ""

# === GPU ===
Write-Output "=== CARTE(S) GRAPHIQUE(S) ==="
$gpus = Get-WmiObject Win32_VideoController -EA SilentlyContinue
foreach ($gpu in $gpus) {
    Write-Output "  Modele          : $($gpu.Name)"
    # VRAM via registre 64-bit (QWORD) — WMI AdapterRAM est limité à 32 bits (max ~4 Go)
    $vramBytes = $null
    try {
        $adPaths = Get-ChildItem "HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}" -EA SilentlyContinue |
                   Where-Object { $_.PSChildName -match '^\d{4}$' }
        foreach ($ap in $adPaths) {
            $qw = (Get-ItemProperty $ap.PSPath -EA SilentlyContinue).'HardwareInformation.qwMemorySize'
            if ($qw -and [long]$qw -gt 0) { $vramBytes = [long]$qw; break }
        }
    } catch {}
    if ($vramBytes -and $vramBytes -gt 0) {
        $vram = "$([math]::Round($vramBytes / 1GB, 1)) Go"
        Write-Output "  VRAM            : $vram"
    } elseif ($gpu.AdapterRAM -and $gpu.AdapterRAM -gt 0) {
        $mb = [math]::Round($gpu.AdapterRAM / 1MB)
        $vram = if ($mb -ge 1024) { "$([math]::Round($mb/1024.0,1)) Go" } else { "$mb Mo" }
        Write-Output "  VRAM (approx)   : $vram"
    }
    Write-Output "  Version pilote  : $($gpu.DriverVersion)"
    if ($gpu.CurrentHorizontalResolution) {
        Write-Output "  Resolution      : $($gpu.CurrentHorizontalResolution) x $($gpu.CurrentVerticalResolution)"
    }
    Write-Output ""
}

# === RAM ===
Write-Output "=== MEMOIRE RAM ==="
$ramList = Get-WmiObject Win32_PhysicalMemory -EA SilentlyContinue
$ramTotal = ($ramList | Measure-Object Capacity -Sum).Sum
if ($ramTotal) {
    Write-Output "  Total           : $([math]::Round($ramTotal/1GB,1)) Go"
}
foreach ($stick in $ramList) {
    $slot = if ($stick.DeviceLocator) { $stick.DeviceLocator } else { 'Slot inconnu' }
    $cap  = [math]::Round($stick.Capacity / 1GB, 1)
    $type = switch ($stick.SMBIOSMemoryType) {
        26 { 'DDR4' } 34 { 'DDR5' } 24 { 'DDR3' } 21 { 'DDR2' } 20 { 'DDR' } default { 'RAM' }
    }
    $speed = if ($stick.ConfiguredClockSpeed -gt 0) { $stick.ConfiguredClockSpeed }
             elseif ($stick.Speed -gt 0) { $stick.Speed } else { 0 }
    $speedStr = if ($speed -gt 0) { " @ $speed MHz" } else { '' }
    Write-Output "  $slot          : $cap Go $type$speedStr"
}
Write-Output ""

# === STOCKAGE ===
Write-Output "=== STOCKAGE (SSD / HDD) ==="
# Essai Get-PhysicalDisk (Storage module), repli sur Win32_DiskDrive
$disks = $null
try { $disks = Get-PhysicalDisk -ErrorAction Stop } catch {}
if ($disks) {
    foreach ($d in ($disks | Sort-Object DeviceId)) {
        $type = switch ($d.MediaType) {
            'SSD' { 'SSD' } 'HDD' { 'Disque dur (HDD)' } 'SCM' { 'Storage Class Memory' } default { $d.MediaType }
        }
        $size = if ($d.Size -ge 1GB) { "$([math]::Round($d.Size/1GB,1)) Go" } else { "$([math]::Round($d.Size/1MB,0)) Mo" }
        Write-Output "  $($d.FriendlyName)"
        Write-Output "    Type            : $type"
        Write-Output "    Capacite        : $size"
        Write-Output "    Sante           : $($d.HealthStatus)"
        Write-Output ""
    }
} else {
    foreach ($d in (Get-WmiObject Win32_DiskDrive -EA SilentlyContinue | Sort-Object Index)) {
        $size = if ($d.Size -ge 1GB) { "$([math]::Round($d.Size/1GB,1)) Go" } else { "$([math]::Round($d.Size/1MB,0)) Mo" }
        Write-Output "  $($d.Model)"
        Write-Output "    Interface       : $($d.InterfaceType)"
        Write-Output "    Capacite        : $size"
        Write-Output ""
    }
}

# === CARTE MERE ===
Write-Output "=== CARTE MERE ==="
$mb = Get-WmiObject Win32_BaseBoard -EA SilentlyContinue
if ($mb) {
    Write-Output "  Fabricant       : $($mb.Manufacturer)"
    Write-Output "  Modele          : $($mb.Product)"
    $sn = if ($mb.SerialNumber -and $mb.SerialNumber -notmatch 'Default|None|To Be|N/A|^\s*$') { $mb.SerialNumber } else { 'Non disponible' }
    Write-Output "  Numero de serie : $sn"
}
Write-Output ""

# === BIOS ===
Write-Output "=== BIOS / UEFI ==="
$bios = Get-WmiObject Win32_BIOS -EA SilentlyContinue
if ($bios) {
    Write-Output "  Fabricant       : $($bios.Manufacturer)"
    Write-Output "  Version         : $($bios.SMBIOSBIOSVersion)"
    if ($bios.ReleaseDate -and $bios.ReleaseDate.Length -ge 8) {
        try {
            $bd = [datetime]::ParseExact($bios.ReleaseDate.Substring(0,8), 'yyyyMMdd', $null)
            Write-Output "  Date            : $($bd.ToString('dd/MM/yyyy'))"
        } catch {}
    }
}
"#;
    run_ps_temp(script)
}

pub fn collect_wifi_passwords() -> Result<String, NiTriTeError> {
    let script = r#"
$raw = netsh wlan show profiles 2>$null
if (-not $raw) {
    Write-Output "Wi-Fi desactive ou aucun profil sauvegarde."
    return
}
$profiles = @()
foreach ($line in $raw) {
    # Compatible Windows EN + FR (locale-agnostic)
    if ($line -match '(?:All User Profile|Profil Tous les utilisateurs|User Profile|Profil utilisateur)\s*:\s*(.+)') {
        $profiles += $Matches[1].Trim()
    }
}
if ($profiles.Count -eq 0) {
    Write-Output "Aucun profil WiFi sauvegarde."
    return
}
foreach ($name in $profiles) {
    $detail = netsh wlan show profile name="$name" key=clear 2>$null
    # Compatible EN "Key Content" + FR "Contenu de la cle/clé"
    $passLine = $detail | Where-Object { $_ -match '(?:Key Content|Contenu de la cl[eé])\s*:\s*(.+)' }
    Write-Output "Reseau       : $name"
    if ($passLine) {
        $pw = ($passLine[0] -replace '.*:\s*', '').Trim()
        Write-Output "Mot de passe : $pw"
    } else {
        Write-Output "Mot de passe : (aucun / reseau ouvert ou securise autrement)"
    }
    # Authentification — compatible EN + FR
    $auth = $detail | Where-Object { $_ -match '(?:Authentication|Authentification)\s*:\s*(.+)' }
    if ($auth) {
        $a = ($auth[0] -replace '.*:\s*', '').Trim()
        Write-Output "Securite     : $a"
    }
    Write-Output ""
}
"#;
    run_ps_temp(script)
}

pub fn collect_registry_export() -> Result<String, NiTriTeError> {
    // Export partiel du registre HKCU (Software uniquement, taille limitee)
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command",
            "Get-ItemProperty 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run' -ErrorAction SilentlyContinue | ConvertTo-Json; Get-ItemProperty 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\User Shell Folders' -ErrorAction SilentlyContinue | ConvertTo-Json"])
        .creation_flags(0x08000000).output_timeout()?;
    Ok(crate::maintenance::commands::decode_output(&output.stdout))
}

pub fn collect_suspicious_processes() -> Result<String, NiTriTeError> {
    // Sortie formatée directement (plus besoin de json_to_readable pour ce collecteur)
    let script = concat!(
        "Get-Process -EA SilentlyContinue",
        " | Where-Object { $_.Path -and $_.Path -notmatch 'Windows|Microsoft|System32|SysWOW64|Program Files' }",
        " | ForEach-Object {",
        "   $mem = $_.WorkingSet64;",
        "   $memStr = if($mem -ge 1073741824){ \"{0:N2} Go\" -f ($mem/1GB) }",
        "             elseif($mem -ge 1048576){ \"{0:N0} Mo\" -f ($mem/1MB) }",
        "             else{ \"{0:N0} Ko\" -f ($mem/1KB) };",
        "   [PSCustomObject]@{Processus=$_.ProcessName; PID=$_.Id; Memoire=$memStr; Chemin=$_.Path}",
        " }",
        " | Sort-Object {$_.Memoire} -Descending",
        " | Select-Object -First 30",
        " | ForEach-Object { \"$($_.Processus) (PID $($_.PID)) — $($_.Memoire)`n  Chemin : $($_.Chemin)\n\" }"
    );
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .creation_flags(0x08000000).output_timeout()?;
    Ok(crate::maintenance::commands::decode_output(&output.stdout))
}


pub fn collect_winget_export() -> Result<String, NiTriTeError> {
    let tmp = std::env::temp_dir().join("nitrite_winget_export.json");
    let _ = Command::new("winget")
        .args(["export", "-o", tmp.to_str().unwrap_or(""), "--accept-source-agreements"])
        .creation_flags(0x08000000)
        .output_timeout();
    if tmp.exists() {
        let content = std::fs::read_to_string(&tmp)?;
        let _ = std::fs::remove_file(&tmp);
        Ok(content)
    } else {
        Err(NiTriTeError::System("winget export échoué".into()))
    }
}

pub fn collect_network_shares() -> Result<String, NiTriTeError> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command",
            "Get-SmbShare | Select-Object Name, Path, Description | Format-Table -AutoSize | Out-String"])
        .creation_flags(0x08000000)
        .output_timeout()?;
    Ok(crate::maintenance::commands::decode_output(&output.stdout))
}

pub fn collect_hosts_file() -> Result<String, NiTriTeError> {
    let hosts = std::path::Path::new(r"C:\Windows\System32\drivers\etc\hosts");
    if hosts.exists() {
        Ok(std::fs::read_to_string(hosts)?)
    } else {
        Err(NiTriTeError::System("Fichier hosts introuvable".into()))
    }
}

pub fn collect_ssh_keys() -> Result<String, NiTriTeError> {
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    let ssh_dir = std::path::PathBuf::from(&home).join(".ssh");
    let mut lines = vec![format!("Dossier : {}", ssh_dir.display())];
    if ssh_dir.exists() {
        for entry in std::fs::read_dir(&ssh_dir).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            lines.push(format!("  {} ({})", name, format_size(size)));
        }
    } else {
        lines.push("Dossier .ssh introuvable".into());
    }
    Ok(lines.join("\n"))
}

pub fn collect_pip_packages() -> Result<String, NiTriTeError> {
    let output = Command::new("pip")
        .args(["freeze"])
        .creation_flags(0x08000000)
        .output_timeout()
        .or_else(|_| Command::new("pip3").args(["freeze"]).creation_flags(0x08000000).output_timeout())?;
    let s = crate::maintenance::commands::decode_output(&output.stdout);
    if s.trim().is_empty() {
        Err(NiTriTeError::System("pip non disponible ou aucun package installé".into()))
    } else {
        Ok(s)
    }
}

pub fn collect_vscode_extensions() -> Result<String, NiTriTeError> {
    let output = Command::new("code")
        .args(["--list-extensions"])
        .creation_flags(0x08000000)
        .output_timeout()?;
    let s = crate::maintenance::commands::decode_output(&output.stdout);
    if s.trim().is_empty() {
        Err(NiTriTeError::System("VSCode non disponible ou aucune extension".into()))
    } else {
        Ok(s)
    }
}

pub fn collect_wsl_config() -> Result<String, NiTriTeError> {
    let output = Command::new("wsl")
        .args(["--list", "--verbose"])
        .creation_flags(0x08000000)
        .output_timeout()?;
    let mut content = crate::maintenance::commands::decode_output(&output.stdout);
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    let wslconfig = std::path::PathBuf::from(&home).join(".wslconfig");
    if wslconfig.exists() {
        if let Ok(cfg) = std::fs::read_to_string(&wslconfig) {
            content.push_str("\n\n--- .wslconfig ---\n");
            content.push_str(&cfg);
        }
    }
    Ok(content)
}

pub fn collect_powershell_profile() -> Result<String, NiTriTeError> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command",
            "if (Test-Path $PROFILE) { \"=== \" + $PROFILE + \" ===\"; Get-Content $PROFILE } else { 'Profil inexistant : ' + $PROFILE }"])
        .creation_flags(0x08000000)
        .output_timeout()?;
    Ok(crate::maintenance::commands::decode_output(&output.stdout))
}

pub fn collect_power_plans() -> Result<String, NiTriTeError> {
    let output = Command::new("powercfg")
        .args(["/list"])
        .creation_flags(0x08000000)
        .output_timeout()?;
    Ok(crate::maintenance::commands::decode_output(&output.stdout))
}

pub fn collect_printer_config() -> Result<String, NiTriTeError> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command",
            "Get-Printer | Select-Object Name, PortName, DriverName, PrinterStatus | Format-Table -AutoSize | Out-String"])
        .creation_flags(0x08000000)
        .output_timeout()?;
    Ok(crate::maintenance::commands::decode_output(&output.stdout))
}

pub fn collect_battery() -> Result<String, NiTriTeError> {
    let script = r#"
$b = Get-WmiObject Win32_Battery -ErrorAction SilentlyContinue
if (-not $b) { "Aucune batterie détectée sur ce système." ; exit }
$statMap = @{ 1='Autre'; 2='Inconnu'; 3='Chargement complet'; 4='Faible'; 5='Critique';
              6='En charge'; 7='En charge + haute'; 8='En charge + faible'; 9='En charge + critique';
              10='Non défini'; 11='Partiellement chargée' }
foreach ($bat in $b) {
    $charge  = $bat.EstimatedChargeRemaining
    $design  = $bat.DesignCapacity
    $full    = $bat.FullChargeCapacity
    $runtime = $bat.EstimatedRunTime
    $status  = if ($bat.BatteryStatus -and $statMap[$bat.BatteryStatus]) { $statMap[$bat.BatteryStatus] } else { 'Inconnu' }
    $health  = if ($design -and $full -and $design -gt 0) { [math]::Round(($full / $design) * 100, 1) } else { 'N/A' }
    $rt      = if ($runtime -and $runtime -lt 71582) { "$runtime min" } else { 'Non disponible' }
    "Nom             : $($bat.Name)"
    "Statut          : $status"
    "Charge actuelle : $charge %"
    "Capacité design : $design mWh"
    "Capacité max    : $full mWh"
    "Santé batterie  : $health %"
    "Autonomie est.  : $rt"
    ""
}
# Infos supplémentaires via powercfg
$cfg = powercfg /batteryreport /xml /output "$env:TEMP\batt_report.xml" 2>$null
$xml = [xml](Get-Content "$env:TEMP\batt_report.xml" -ErrorAction SilentlyContinue)
if ($xml) {
    $design2 = $xml.BatteryReport.Batteries.Battery.DesignCapacity
    $full2   = $xml.BatteryReport.Batteries.Battery.FullChargeCapacity
    $cycleCount = $xml.BatteryReport.Batteries.Battery.CycleCount
    if ($design2) { "--- Données powercfg ---" }
    if ($design2) { "Capacité design (powercfg) : $design2 mWh" }
    if ($full2)   { "Capacité max (powercfg)   : $full2 mWh" }
    if ($cycleCount) { "Cycles de charge          : $cycleCount" }
    Remove-Item "$env:TEMP\batt_report.xml" -Force -ErrorAction SilentlyContinue
}
"#;
    run_ps_temp(script)
}
