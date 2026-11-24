//! Collecteurs supplémentaires (sauvegarder plus de choses).
//! Chaque fonction a son timeout (via `output_timeout` / `run_ps_temp`) et
//! renvoie une sortie décodée en UTF-8 (jamais `from_utf8_lossy`).
use crate::error::NiTriTeError;
use crate::maintenance::commands::decode_output;
use super::{run_ps_temp, OutputTimeout};
use std::process::Command;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// Correctifs / KB installés (Get-HotFix).
pub fn collect_hotfixes() -> Result<String, NiTriTeError> {
    run_ps_temp(
        "Get-HotFix -EA SilentlyContinue | Sort-Object InstalledOn -Descending | \
         Select-Object HotFixID, Description, @{N='InstalledOn';E={if($_.InstalledOn){$_.InstalledOn.ToString('yyyy-MM-dd')}else{''}}} | \
         ConvertTo-Json -Depth 3",
    )
}

/// Services Windows : nom, état, type de démarrage, libellé.
pub fn collect_services() -> Result<String, NiTriTeError> {
    run_ps_temp(
        "Get-Service -EA SilentlyContinue | Sort-Object Name | \
         Select-Object Name, DisplayName, @{N='Status';E={$_.Status.ToString()}}, @{N='StartType';E={$_.StartType.ToString()}} | \
         ConvertTo-Json -Depth 3",
    )
}

/// Points de restauration système (vide si la protection système est désactivée).
pub fn collect_restore_points() -> Result<String, NiTriTeError> {
    run_ps_temp(
        "$rp = Get-ComputerRestorePoint -EA SilentlyContinue; \
         if($rp){ $rp | Select-Object SequenceNumber, Description, \
           @{N='CreationTime';E={[System.Management.ManagementDateTimeConverter]::ToDateTime($_.CreationTime).ToString('yyyy-MM-dd HH:mm')}}, \
           RestorePointType | ConvertTo-Json -Depth 3 } else { '' }",
    )
}

/// Journaux d'événements récents (System + Application, 7 derniers jours,
/// erreurs/avertissements, plafonné à 200 entrées).
pub fn collect_event_logs() -> Result<String, NiTriTeError> {
    run_ps_temp(
        "$ev = Get-WinEvent -FilterHashtable @{LogName='System','Application'; Level=1,2,3; StartTime=(Get-Date).AddDays(-7)} -MaxEvents 200 -EA SilentlyContinue; \
         if($ev){ $ev | Select-Object @{N='Time';E={$_.TimeCreated.ToString('yyyy-MM-dd HH:mm:ss')}}, LogName, LevelDisplayName, ProviderName, Id, \
           @{N='Message';E={if($_.Message){($_.Message -replace '\\s+',' ').Trim().Substring(0,[Math]::Min(300,$_.Message.Length))}else{''}}} | \
           ConvertTo-Json -Depth 3 } else { '' }",
    )
}

/// Profils VPN (par machine + par utilisateur). Vide si aucun VPN configuré.
pub fn collect_vpn_profiles() -> Result<String, NiTriTeError> {
    run_ps_temp(
        "$v = @(); \
         $v += Get-VpnConnection -EA SilentlyContinue; \
         $v += Get-VpnConnection -AllUserConnection -EA SilentlyContinue; \
         if($v.Count -gt 0){ $v | Select-Object Name, ServerAddress, TunnelType, AuthenticationMethod, EncryptionLevel, SplitTunneling, AllUserConnection | \
           ConvertTo-Json -Depth 3 } else { '' }",
    )
}

/// Toutes les tâches planifiées exportées en XML (réimportables via schtasks /create /xml).
/// `schtasks /xml` écrit en UTF-16LE (avec BOM) : `decode_output` ne gère que l'UTF-8
/// et le codepage OEM, donc on décode l'UTF-16 nous-mêmes pour éviter le mojibake.
pub fn collect_scheduled_tasks_xml() -> Result<String, NiTriTeError> {
    let output = Command::new("schtasks")
        .args(["/query", "/xml", "ONE"])
        .creation_flags(0x08000000)
        .output_timeout()?;
    Ok(decode_utf16_or_text(&output.stdout))
}

/// Décode une sortie UTF-16LE (BOM FF FE) sinon délègue à `decode_output` (UTF-8/OEM).
fn decode_utf16_or_text(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&u16s);
    }
    decode_output(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_utf16_le_with_bom_no_mojibake() {
        // « Tâche » encodé UTF-16LE + BOM : doit ressortir intact, pas en mojibake.
        let mut bytes = vec![0xFF, 0xFE];
        for u in "Tâche".encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(decode_utf16_or_text(&bytes), "Tâche");
    }

    #[test]
    fn decode_utf8_passthrough() {
        assert_eq!(decode_utf16_or_text("déjà".as_bytes()), "déjà");
    }
}
