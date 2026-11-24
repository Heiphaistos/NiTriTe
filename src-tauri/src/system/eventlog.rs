use serde::Serialize;
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::error::NiTriTeError;

#[derive(Debug, Clone, Serialize)]
pub struct EventLogEntry {
    pub id: u64,
    pub level: String,
    pub source: String,
    pub timestamp: String,
    pub message: String,
}

/// Recupere les logs Windows Event Viewer via wevtutil
pub fn get_event_logs(log_name: &str, count: u32) -> Result<Vec<EventLogEntry>, NiTriTeError> {
    // Valider le nom du log (securite)
    let valid_logs = ["System", "Application", "Security", "Setup"];
    if !valid_logs.contains(&log_name) {
        return Err(NiTriTeError::System(format!("Log invalide: {}. Valides: {:?}", log_name, valid_logs)));
    }

    let count = count.min(200); // Limiter a 200 max

    // Niveau dérivé de l'entier .Level (INVARIANT : 1=Critical..5=Verbose) et
    // non de LevelDisplayName, qui est LOCALISÉ (« Erreur »/« Avertissement » sur
    // Windows FR) — le frontend colore par les noms anglais, donc sur FR tous les
    // événements tombaient en « info » (aucune coloration rouge/jaune).
    let ps_cmd = format!(
        "$OutputEncoding = [System.Text.Encoding]::UTF8; [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Get-WinEvent -LogName '{}' -MaxEvents {} -ErrorAction SilentlyContinue | Select-Object Id, @{{Name='Level';Expression={{switch([int]$_.Level){{1{{'Critical'}}2{{'Error'}}3{{'Warning'}}4{{'Information'}}5{{'Verbose'}}default{{'Information'}}}}}}}}, ProviderName, TimeCreated, Message | ConvertTo-Json -Depth 2",
        log_name, count
    );
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_cmd])
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| NiTriTeError::System(format!("Erreur wevtutil: {}", e)))?;

    let stdout = crate::maintenance::commands::decode_output(&output.stdout);

    if stdout.trim().is_empty() {
        return Ok(Vec::new());
    }

    // Parser le JSON PowerShell
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|_| serde_json::Value::Array(Vec::new()));

    let entries = match parsed {
        serde_json::Value::Array(arr) => arr,
        obj @ serde_json::Value::Object(_) => vec![obj], // Un seul resultat
        _ => Vec::new(),
    };

    let result: Vec<EventLogEntry> = entries
        .iter()
        .filter_map(|e| {
            Some(EventLogEntry {
                id: e.get("Id")?.as_u64().unwrap_or(0),
                level: e.get("Level")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Information")
                    .to_string(),
                source: e.get("ProviderName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string(),
                timestamp: extract_timestamp(e.get("TimeCreated")?),
                message: e.get("Message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .chars()
                    .take(500) // Tronquer les messages trop longs
                    .collect(),
            })
        })
        .collect();

    Ok(result)
}

/// Extrait le timestamp d'un objet PowerShell DateTime serialise
fn extract_timestamp(val: &serde_json::Value) -> String {
    // PowerShell serialise DateTime comme "/Date(timestamp)/" ou comme string
    if let Some(s) = val.as_str() {
        return s.to_string();
    }
    if let Some(obj) = val.as_object() {
        if let Some(date_str) = obj.get("DateTime").and_then(|v| v.as_str()) {
            return date_str.to_string();
        }
        if let Some(ticks) = obj.get("Ticks").and_then(|v| v.as_i64()) {
            // Convertir les ticks .NET en timestamp lisible
            let secs = (ticks / 10_000_000) - 62_135_596_800; // Epoch .NET -> Unix
            if let Some(dt) = chrono::DateTime::from_timestamp(secs, 0) {
                return dt.format("%Y-%m-%d %H:%M:%S").to_string();
            }
        }
    }
    "Unknown".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_timestamp_from_string() {
        let v = json!("2026-03-15 10:30:00");
        assert_eq!(extract_timestamp(&v), "2026-03-15 10:30:00");
    }

    #[test]
    fn extract_timestamp_from_datetime_obj() {
        let v = json!({ "DateTime": "2026-01-01 00:00:00" });
        assert_eq!(extract_timestamp(&v), "2026-01-01 00:00:00");
    }

    #[test]
    fn extract_timestamp_unknown_on_empty_obj() {
        let v = json!({});
        assert_eq!(extract_timestamp(&v), "Unknown");
    }

    #[test]
    fn extract_timestamp_unknown_on_null() {
        let v = json!(null);
        assert_eq!(extract_timestamp(&v), "Unknown");
    }

    #[test]
    fn extract_timestamp_from_ticks() {
        // .NET ticks for 2000-01-01 00:00:00 UTC = 630822816000000000
        let v = json!({ "Ticks": 630822816000000000_i64 });
        let result = extract_timestamp(&v);
        assert!(result.starts_with("2000-01-01"), "Got: {}", result);
    }
}
