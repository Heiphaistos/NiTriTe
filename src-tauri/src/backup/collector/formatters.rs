//! Utilitaires de formatage pour les données de sauvegarde

/// Nom de fichier sûr (ASCII, sans accents ni caractères interdits Windows)
/// pour un élément du lot de sauvegarde.
pub fn safe_filename(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        let m = match c {
            'à'|'â'|'ä'|'á' => 'a', 'é'|'è'|'ê'|'ë' => 'e', 'î'|'ï'|'í' => 'i',
            'ô'|'ö'|'ó' => 'o', 'ù'|'û'|'ü'|'ú' => 'u', 'ç' => 'c',
            'À'|'Â'|'Ä'|'Á' => 'a', 'É'|'È'|'Ê'|'Ë' => 'e', 'Ç' => 'c',
            c if c.is_ascii_alphanumeric() => c.to_ascii_lowercase(),
            _ => '-',
        };
        out.push(m);
    }
    while out.contains("--") { out = out.replace("--", "-"); }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() { "element".to_string() } else { trimmed }
}

/// Transforme une clé snake_case en libellé lisible (« cpu_name » → « Cpu name »).
pub fn prettify_key(k: &str) -> String {
    let spaced = k.replace('_', " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => spaced,
    }
}

pub fn format_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

// Champs PowerShell internes à ignorer
const PS_SKIP: &[&str] = &[
    "PSPath","PSParentPath","PSChildName","PSProvider",
    "PSComputerName","RunspaceId","PSShowComputerName",
];

// Traduction des clés techniques → labels lisibles
pub fn friendly_label(k: &str) -> &str {
    match k {
        "Name"|"TaskName"|"DisplayName"   => "Nom",
        "ProductName"                      => "Produit",
        "Description"                      => "Description",
        "State"                            => "Etat",
        "Status"                           => "Statut",
        "Path"|"TaskPath"|"FullName"       => "Chemin",
        "Id"|"ProcessId"                   => "PID",
        "ProcessName"                      => "Processus",
        "SizeMB"|"SizeGB"                  => "Taille",
        "Folder"|"Dossier"                 => "Dossier",
        "LicenseStatus"                    => "Statut licence",
        "PartialProductKey"                => "Cle partielle",
        "LicenseFamily"                    => "Canal",
        "MemMB"                            => "Memoire",
        "Username"|"UserName"              => "Utilisateur",
        "Version"|"DisplayVersion"         => "Version",
        "Publisher"|"InstallLocation"      => "Editeur",
        "InstallDate"                      => "Date installation",
        "FeatureName"                      => "Fonctionnalite",
        "MountPoint"                       => "Lecteur",
        "VolumeStatus"                     => "Statut volume",
        "EncryptionPercentage"             => "Chiffrement (%)",
        "KeyProtector"                     => "Protecteur cle",
        "MemoryMB"                         => "Memoire",
        other                              => other,
    }
}

// Formate une valeur numérique selon le nom de la clé (Mo, Go, %)
pub fn fmt_numeric(k: &str, n: &serde_json::Number) -> Option<String> {
    let kl = k.to_lowercase();
    let f = n.as_f64()?;
    if kl.ends_with("mb") || kl == "sizemb" || kl == "memmb" || kl == "memorymb" {
        return Some(if f >= 1024.0 {
            format!("{:.2} Go", f / 1024.0)
        } else {
            format!("{:.0} Mo", f)
        });
    }
    if kl.ends_with("gb") || kl == "sizegb" {
        return Some(format!("{:.2} Go", f));
    }
    if kl.contains("bytes") || (kl.contains("size") && !kl.contains("mb") && !kl.contains("gb")) {
        if let Some(u) = n.as_u64() { return Some(format_size(u)); }
    }
    if kl.contains("percent") { return Some(format!("{}%", f)); }
    None
}

/// Convertit du JSON en texte lisible pour un non-informaticien
pub fn json_to_readable(s: &str) -> String {
    let trimmed = s.trim();
    if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
        return s.to_string();
    }
    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(v)  => fmt_val(&v, 0),
        Err(_) => s.to_string(),
    }
}

pub fn fmt_val(v: &serde_json::Value, depth: usize) -> String {
    let pad = "  ".repeat(depth);
    match v {
        serde_json::Value::Array(arr) => {
            arr.iter().enumerate().map(|(i, item)| {
                match item {
                    serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                        format!("{}── Entrée {} ──\n{}", pad, i + 1, fmt_val(item, depth))
                    }
                    _ => fmt_val(item, depth),
                }
            }).collect::<Vec<_>>().join("\n\n")
        }
        serde_json::Value::Object(map) => {
            map.iter()
                .filter(|(k, v)| !v.is_null() && !PS_SKIP.contains(&k.as_str()))
                .map(|(k, v)| {
                    let label = friendly_label(k);
                    let val = match v {
                        serde_json::Value::String(s) if s.is_empty() => return String::new(),
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Bool(b) => if *b { "Oui".into() } else { "Non".into() },
                        serde_json::Value::Number(n) => {
                            fmt_numeric(k, n).unwrap_or_else(|| n.to_string())
                        }
                        other => fmt_val(other, depth + 1),
                    };
                    if val.is_empty() { return String::new(); }
                    format!("{}  {:<26} {}", pad, format!("{}:", label), val)
                })
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        }
        serde_json::Value::String(s) => format!("{}{}", pad, s),
        serde_json::Value::Null       => String::new(),
        _                             => format!("{}{}", pad, v),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn format_size_bytes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1023), "1023 B");
    }

    #[test]
    fn format_size_kilobytes() {
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(2048), "2.0 KB");
    }

    #[test]
    fn format_size_megabytes() {
        assert_eq!(format_size(1_048_576), "1.0 MB");
        assert_eq!(format_size(5_242_880), "5.0 MB");
    }

    #[test]
    fn format_size_gigabytes() {
        assert_eq!(format_size(1_073_741_824), "1.0 GB");
        assert_eq!(format_size(2_147_483_648), "2.0 GB");
    }

    #[test]
    fn friendly_label_known_keys() {
        assert_eq!(friendly_label("Name"), "Nom");
        assert_eq!(friendly_label("DisplayName"), "Nom");
        assert_eq!(friendly_label("Version"), "Version");
        assert_eq!(friendly_label("Status"), "Statut");
        assert_eq!(friendly_label("InstallDate"), "Date installation");
    }

    #[test]
    fn friendly_label_unknown_passthrough() {
        assert_eq!(friendly_label("SomeUnknownKey"), "SomeUnknownKey");
        assert_eq!(friendly_label(""), "");
    }

    #[test]
    fn fmt_numeric_mb_key() {
        let n = serde_json::Number::from_f64(512.0).unwrap();
        let result = fmt_numeric("SizeMB", &n).unwrap();
        assert!(result.contains("Mo") || result.contains("Go"));
    }

    #[test]
    fn fmt_numeric_percent_key() {
        let n = serde_json::Number::from_f64(75.0).unwrap();
        let result = fmt_numeric("EncryptionPercentage", &n).unwrap();
        assert_eq!(result, "75%");
    }

    #[test]
    fn fmt_numeric_unknown_key_returns_none() {
        let n = serde_json::Number::from(42u64);
        assert!(fmt_numeric("SomeOtherKey", &n).is_none());
    }

    #[test]
    fn json_to_readable_plain_string() {
        let result = json_to_readable("hello world");
        assert_eq!(result, "hello world");
    }

    #[test]
    fn json_to_readable_invalid_json_passthrough() {
        let result = json_to_readable("{not valid json}");
        assert_eq!(result, "{not valid json}");
    }

    #[test]
    fn fmt_val_bool_renders_oui_non() {
        let v = json!({"Active": true, "Connected": false});
        let result = fmt_val(&v, 0);
        assert!(result.contains("Oui"));
        assert!(result.contains("Non"));
    }

    #[test]
    fn fmt_val_ps_fields_skipped() {
        let v = json!({"PSPath": "some/path", "Name": "disk1"});
        let result = fmt_val(&v, 0);
        assert!(!result.contains("PSPath"));
        assert!(result.contains("disk1"));
    }

    #[test]
    fn fmt_val_null_excluded() {
        let v = json!({"Name": "test", "Description": null});
        let result = fmt_val(&v, 0);
        assert!(result.contains("test"));
        assert!(!result.contains("Description"));
    }
}
