use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct BluetoothDevice {
    pub name: String,
    pub address: String,
    pub device_class: String,
    pub paired: bool,
    pub connected: bool,
    pub trusted: bool,
    pub rssi: i32,
    pub manufacturer: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct BluetoothAdapter {
    pub name: String,
    pub address: String,
    pub enabled: bool,
    pub manufacturer: String,
    pub driver_version: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct BluetoothReport {
    pub adapters: Vec<BluetoothAdapter>,
    pub devices: Vec<BluetoothDevice>,
    pub bt_available: bool,
    pub error: String,
}

/// Accès à la radio Bluetooth par l'API WinRT `Windows.Devices.Radios` : c'est
/// l'interrupteur de Paramètres > Bluetooth, sans droits admin. Définit `$bt`
/// (radio Bluetooth ou $null) et la fonction `Await`.
#[cfg(target_os = "windows")]
const PS_RADIO_PRELUDE: &str = r#"
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$asTask = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object { $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1' })[0]
function Await($op, [Type]$t) { $task = $asTask.MakeGenericMethod($t).Invoke($null, @($op)); $null = $task.Wait(10000); $task.Result }
[void][Windows.Devices.Radios.Radio,Windows.System.Devices,ContentType=WindowsRuntime]
$bt = $null
try {
    $radios = Await ([Windows.Devices.Radios.Radio]::GetRadiosAsync()) ([System.Collections.Generic.IReadOnlyList[Windows.Devices.Radios.Radio]])
    $bt = $radios | Where-Object { $_.Kind -eq 'Bluetooth' } | Select-Object -First 1
} catch {}
"#;

// Anti-freeze : PowerShell/WMI est bloquant — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn get_bluetooth_info() -> BluetoothReport {
    tokio::task::spawn_blocking(get_bluetooth_info_blocking)
        .await
        .unwrap_or_default()
}

// Revu le 2026-11-23 sur un vrai PC (Intel AX + Windows 11 FR) : l'ancien script
// listait comme « adaptateurs » les 3 énumérateurs logiciels (BTH\MS_BTHBRB,
// MS_RFCOMM, MS_BTHLE) en plus de la vraie radio, ignorait tous les appareils
// BLE (BTHLE\DEV_*) et listait chaque service d'un appareil classique
// (BTHENUM\{uuid}…) comme un appareil séparé. Il faisait 3 énumérations WMI
// complètes, sans délai maximal.
#[cfg(target_os = "windows")]
const PS_BT_INFO: &str = r#"
try {
    $all = @(Get-CimInstance Win32_PnPEntity -EA Stop | Where-Object { $_.PNPClass -eq 'Bluetooth' -or $_.PNPDeviceID -match '^(BTHENUM|BTHLE)\\DEV_' })
    $radioOn = if ($bt) { "$($bt.State)" -eq 'On' } else { $null }
    $adapters = @($all | Where-Object { $_.PNPClass -eq 'Bluetooth' -and $_.PNPDeviceID -notmatch '^(BTH|BTHENUM|BTHLE|BTHLEDEVICE|SWD)\\' } | ForEach-Object {
        $drv = ''
        try { $drv = "$((Get-PnpDeviceProperty -InstanceId $_.PNPDeviceID -KeyName 'DEVPKEY_Device_DriverVersion' -EA Stop).Data)" } catch {}
        $ok = ($_.ConfigManagerErrorCode -eq 0)
        @{ name = "$($_.Name)"; address = "$($_.PNPDeviceID)"; enabled = ($ok -and $radioOn -ne $false); mfr = "$($_.Manufacturer)"; driver = $drv }
    })
    $devices = @($all | Where-Object { $_.PNPDeviceID -match '^(BTHENUM|BTHLE)\\DEV_' } | ForEach-Object {
        $n = "$($_.Name)"
        $cat = switch -Regex ($n) {
            'head|ear|bud|speaker|casque|couteur|enceinte|audio|sound|pods' { 'Audio'; break }
            'mouse|souris' { 'Souris'; break }
            'keyboard|clavier' { 'Clavier'; break }
            'gamepad|controller|manette|xbox|dualsense|joy' { 'Manette'; break }
            'phone|galaxy|pixel|xiaomi|redmi' { 'Telephone'; break }
            default { 'Autre' }
        }
        # Connexion réelle : DEVPKEY_Device_IsConnected (ConfigManagerErrorCode 0 = pilote OK, pas « connecté »)
        $isConn = $false
        try { $isConn = [bool](Get-PnpDeviceProperty -InstanceId $_.PNPDeviceID -KeyName '{83DA6326-97A6-4088-9453-A1923F573B29} 15' -EA Stop).Data } catch {}
        $mac = if ($_.PNPDeviceID -match 'DEV_([0-9A-F]{12})') { ($Matches[1] -split '(..)' -ne '') -join ':' } else { '' }
        @{ name = $n; address = $mac; class = $(if ($_.PNPDeviceID -match '^BTHLE') { 'Bluetooth LE' } else { 'Bluetooth' }); connected = $isConn; mfr = "$($_.Manufacturer)"; cat = $cat }
    })
    @{ adapters = $adapters; devices = $devices; available = ($adapters.Count -gt 0); error = '' } | ConvertTo-Json -Depth 4 -Compress
} catch {
    @{ adapters = @(); devices = @(); available = $false; error = $_.Exception.Message } | ConvertTo-Json -Compress
}
"#;

fn get_bluetooth_info_blocking() -> BluetoothReport {
    #[cfg(target_os = "windows")]
    {
        let script = format!("[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; {}{}", PS_RADIO_PRELUDE, PS_BT_INFO);
        let r = crate::maintenance::commands::execute_system_command(
            "powershell", &["-NoProfile", "-NonInteractive", "-Command", &script], 60,
        );
        let out = match r {
            Ok(o) => o,
            Err(e) => return BluetoothReport { error: e.to_string(), ..Default::default() },
        };
        if let Some(report) = parse_bt_report(out.stdout.trim()) {
            return report;
        }
    }
    BluetoothReport { error: "Erreur lecture Bluetooth".to_string(), ..Default::default() }
}

fn parse_bt_report(json: &str) -> Option<BluetoothReport> {
    let v = serde_json::from_str::<serde_json::Value>(json).ok()?;
    let adapters = v["adapters"].as_array().map(|arr| {
        arr.iter().map(|a| BluetoothAdapter {
            name: a["name"].as_str().unwrap_or("").to_string(),
            address: a["address"].as_str().unwrap_or("").to_string(),
            enabled: a["enabled"].as_bool().unwrap_or(false),
            manufacturer: a["mfr"].as_str().unwrap_or("").to_string(),
            driver_version: a["driver"].as_str().unwrap_or("").to_string(),
        }).collect()
    }).unwrap_or_default();
    let devices = v["devices"].as_array().map(|arr| {
        arr.iter().map(|d| BluetoothDevice {
            name: d["name"].as_str().unwrap_or("").to_string(),
            address: d["address"].as_str().unwrap_or("").to_string(),
            device_class: d["class"].as_str().unwrap_or("").to_string(),
            // Un nœud DEV_ n'existe que pour un appareil jumelé
            paired: true,
            connected: d["connected"].as_bool().unwrap_or(false),
            trusted: false,
            rssi: 0,
            manufacturer: d["mfr"].as_str().unwrap_or("").to_string(),
            category: d["cat"].as_str().unwrap_or("Autre").to_string(),
        }).collect()
    }).unwrap_or_default();
    Some(BluetoothReport {
        adapters,
        devices,
        bt_available: v["available"].as_bool().unwrap_or(false),
        error: v["error"].as_str().unwrap_or("").to_string(),
    })
}

// Anti-freeze : PowerShell est bloquant — jamais inline sur le thread de commande.
#[tauri::command]
pub async fn toggle_bluetooth(enable: bool) -> Result<String, String> {
    tokio::task::spawn_blocking(move || toggle_bluetooth_blocking(enable))
        .await
        .map_err(|e| e.to_string())?
}

/// Bascule la radio comme l'interrupteur de Paramètres (WinRT Radio.SetStateAsync),
/// puis relit son état réel. L'ancienne version « désactivait » en arrêtant le
/// service bthserv (admin requis, radio laissée allumée, service relancé à la
/// demande) et « activait » le premier périphérique de classe Bluetooth en
/// erreur, quel qu'il soit.
fn toggle_bluetooth_blocking(enable: bool) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let target = if enable { "On" } else { "Off" };
        // En activation : relancer bthserv (arrêté par l'ancienne méthode) et
        // réactiver une radio PHYSIQUE désactivée dans le Gestionnaire de périphériques.
        let pre = if enable {
            r#"Start-Service bthserv -EA SilentlyContinue
if (-not $bt) {
    $dev = Get-PnpDevice -Class Bluetooth -EA SilentlyContinue | Where-Object { $_.Status -ne 'OK' -and $_.InstanceId -notmatch '^(BTH|BTHENUM|BTHLE|BTHLEDEVICE|SWD)\\' } | Select-Object -First 1 -ExpandProperty InstanceId
    if ($dev) { Enable-PnpDevice -InstanceId $dev -Confirm:$false -EA Stop; Start-Sleep -Seconds 2
        $radios = Await ([Windows.Devices.Radios.Radio]::GetRadiosAsync()) ([System.Collections.Generic.IReadOnlyList[Windows.Devices.Radios.Radio]])
        $bt = $radios | Where-Object { $_.Kind -eq 'Bluetooth' } | Select-Object -First 1 }
}
"#
        } else { "" };
        let script = format!(
            r#"{prelude}
try {{
{pre}
    if (-not $bt) {{ Write-Output 'ERR:Aucune radio Bluetooth détectée'; exit 1 }}
    $null = Await ([Windows.Devices.Radios.Radio]::RequestAccessAsync()) ([Windows.Devices.Radios.RadioAccessStatus])
    $st = Await ($bt.SetStateAsync('{target}')) ([Windows.Devices.Radios.RadioAccessStatus])
    if ("$st" -ne 'Allowed') {{ Write-Output "ERR:Accès à la radio refusé ($st)"; exit 1 }}
    Start-Sleep -Milliseconds 800
    if ("$($bt.State)" -eq '{target}') {{ Write-Output 'OK' }} else {{ Write-Output "ERR:La radio est restée à l'état $($bt.State)"; exit 1 }}
}} catch {{
    Write-Output "ERR:$($_.Exception.Message)"
    exit 1
}}
"#,
            prelude = PS_RADIO_PRELUDE, pre = pre, target = target,
        );
        let o = crate::maintenance::commands::execute_system_command(
            "powershell", &["-NoProfile", "-NonInteractive", "-Command", &script], 60,
        ).map_err(|e| e.to_string())?;
        let out = o.stdout.trim();
        if o.success && out.ends_with("OK") {
            return Ok(if enable { "Bluetooth activé" } else { "Bluetooth désactivé" }.to_string());
        }
        let detail = out.lines().rev().find_map(|l| l.trim().strip_prefix("ERR:")).map(str::to_string)
            .unwrap_or_else(|| if out.is_empty() { o.stderr.trim().to_string() } else { out.to_string() });
        Err(detail)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = enable;
        Err("Non disponible".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_report_from_real_pc() {
        // Sortie réelle du script sur le PC de dev (radio Intel, aucun appareil jumelé)
        let json = r#"{"adapters":[{"mfr":"Intel Corporation","name":"Intel(R) Wireless Bluetooth(R)","driver":"23.60.5.10","address":"USB\\VID_8087&PID_0026\\5&23A82DDC&0&14","enabled":true}],"devices":[],"error":"","available":true}"#;
        let r = parse_bt_report(json).expect("json");
        assert!(r.bt_available);
        assert_eq!(r.adapters.len(), 1);
        assert_eq!(r.adapters[0].driver_version, "23.60.5.10");
        assert!(r.devices.is_empty());
    }

    #[test]
    fn parses_devices() {
        let json = r#"{"adapters":[],"devices":[{"name":"WH-1000XM4","address":"A1:B2:C3:D4:E5:F6","class":"Bluetooth","connected":true,"mfr":"","cat":"Audio"}],"error":"","available":false}"#;
        let r = parse_bt_report(json).expect("json");
        assert_eq!(r.devices[0].address, "A1:B2:C3:D4:E5:F6");
        assert!(r.devices[0].paired && r.devices[0].connected);
        assert!(parse_bt_report("pas du json").is_none());
    }
}
