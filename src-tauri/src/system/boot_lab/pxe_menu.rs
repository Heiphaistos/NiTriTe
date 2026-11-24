//! Menus iPXE autonomes, en-tête Range et adresses d'écoute admises.
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : pxe.py (menu_text, parse_range, eligible_address).

use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MenuEntry {
    pub index: usize,
    pub label: String,
    pub file: String,
    pub sha256: String,
}

fn clean_label(label: &str) -> String {
    label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || " ._()-".contains(c) { c } else { '-' })
        .take(90)
        .collect()
}

/// Script iPXE : un choix par image, `sanboot` pour les ISO, `wimboot` pour les WIM.
pub fn menu_text(entries: &[MenuEntry]) -> String {
    let mut lines: Vec<String> = vec!["#!ipxe".into(), String::new(), "menu NiTriTe - demarrage reseau".into()];
    for e in entries {
        lines.push(format!("item image{} {}", e.index, clean_label(&e.label)));
    }
    lines.extend(
        [
            "item exit Quitter",
            "choose --default exit --timeout 15000 entry || goto exit",
            "goto ${entry}",
            "",
            ":exit",
            "exit",
            "",
        ]
        .map(String::from),
    );
    for e in entries {
        lines.push(format!(":image{}", e.index));
        if e.file.to_lowercase().ends_with(".wim") {
            lines.push("kernel wimboot || goto failed".into());
            lines.push(format!("initrd {} boot.wim || goto failed", e.file));
            lines.push("boot || goto failed".into());
        } else {
            lines.push(format!("sanboot --no-describe {} || goto failed", e.file));
        }
        lines.push(String::new());
    }
    lines.extend([":failed", "echo Le chargement a echoue.", "shell", ""].map(String::from));
    lines.join("\n")
}

/// En-tête `Range` : renvoie (début, longueur, partiel). `None` = 416.
pub fn parse_range(value: Option<&str>, size: u64) -> Option<(u64, u64, bool)> {
    let Some(value) = value.filter(|v| !v.is_empty()) else {
        return Some((0, size, false));
    };
    let spec = value.strip_prefix("bytes=")?;
    let (first, last) = spec.split_once('-')?;
    let digits = |s: &str| s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(first) || !digits(last) {
        return None;
    }
    let num = |s: &str| s.parse::<u128>().ok();
    let size = size as u128;
    let (start, end) = if !first.is_empty() {
        let start = num(first)?;
        let end = if last.is_empty() { size.checked_sub(1)? } else { num(last)?.min(size.checked_sub(1)?) };
        (start, end)
    } else {
        let tail = num(last).filter(|n| *n > 0)?;
        (size.saturating_sub(tail), size.checked_sub(1)?)
    };
    if start > end || end >= size {
        return None;
    }
    Some((start as u64, (end - start + 1) as u64, true))
}

fn in_net(addr: Ipv4Addr, net: [u8; 4], prefix: u32) -> bool {
    let mask = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix) };
    (u32::from(addr) & mask) == (u32::from(Ipv4Addr::from(net)) & mask)
}

/// Adresse IPv4 privée, jamais 0.0.0.0, multicast, lien local ni réservée (comme `eligible_address`).
pub fn eligible_address(value: &str) -> bool {
    let Ok(addr) = value.parse::<Ipv4Addr>() else {
        return false;
    };
    const PRIVATE: [([u8; 4], u32); 10] = [
        ([10, 0, 0, 0], 8),
        ([127, 0, 0, 0], 8),
        ([172, 16, 0, 0], 12),
        ([192, 168, 0, 0], 16),
        ([192, 0, 0, 0], 29),
        ([192, 0, 0, 170], 31),
        ([192, 0, 2, 0], 24),
        ([198, 18, 0, 0], 15),
        ([198, 51, 100, 0], 24),
        ([203, 0, 113, 0], 24),
    ];
    PRIVATE.iter().any(|(net, p)| in_net(addr, *net, *p))
        && !(addr.is_unspecified() || addr.is_multicast() || addr.is_link_local() || in_net(addr, [240, 0, 0, 0], 4))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(index: usize, label: &str, file: &str) -> MenuEntry {
        MenuEntry { index, label: label.into(), file: file.into(), sha256: String::new() }
    }

    #[test]
    fn menu_iso_et_wim() {
        let text = menu_text(&[entry(1, "Debian é/12", "images/01.iso"), entry(2, "WinPE", "images/02.wim")]);
        assert!(text.starts_with("#!ipxe\n\nmenu NiTriTe - demarrage reseau\n"));
        assert!(text.contains("item image1 Debian --12\n"));
        assert!(text.contains(":image1\nsanboot --no-describe images/01.iso || goto failed\n"));
        assert!(text.contains(":image2\nkernel wimboot || goto failed\ninitrd images/02.wim boot.wim || goto failed\nboot || goto failed\n"));
        assert!(text.contains("goto ${entry}\n\n:exit\nexit\n"));
        assert!(text.ends_with(":failed\necho Le chargement a echoue.\nshell\n"));
        assert!(text.is_ascii());
    }

    #[test]
    fn label_tronque_a_90() {
        let text = menu_text(&[entry(1, &"a".repeat(200), "images/01.iso")]);
        assert!(text.contains(&format!("item image1 {}\n", "a".repeat(90))));
    }

    #[test]
    fn plages() {
        assert_eq!(parse_range(None, 100), Some((0, 100, false)));
        assert_eq!(parse_range(Some(""), 0), Some((0, 0, false)));
        assert_eq!(parse_range(Some("bytes=0-9"), 100), Some((0, 10, true)));
        assert_eq!(parse_range(Some("bytes=90-"), 100), Some((90, 10, true)));
        assert_eq!(parse_range(Some("bytes=90-500"), 100), Some((90, 10, true)));
        assert_eq!(parse_range(Some("bytes=-10"), 100), Some((90, 10, true)));
        assert_eq!(parse_range(Some("bytes=-500"), 100), Some((0, 100, true)));
        assert_eq!(parse_range(Some("bytes=100-"), 100), None);
        assert_eq!(parse_range(Some("bytes=9-3"), 100), None);
        assert_eq!(parse_range(Some("bytes=-0"), 100), None);
        assert_eq!(parse_range(Some("bytes=-"), 100), None);
        assert_eq!(parse_range(Some("bytes=0-1,5-6"), 100), None);
        assert_eq!(parse_range(Some("items=0-1"), 100), None);
        assert_eq!(parse_range(Some("bytes=0-"), 0), None);
        assert_eq!(parse_range(Some("bytes=99999999999999999999-"), 100), None);
    }

    #[test]
    fn adresses() {
        for ok in ["127.0.0.1", "10.1.2.3", "172.16.0.5", "172.31.255.1", "192.168.1.20"] {
            assert!(eligible_address(ok), "{ok}");
        }
        for ko in ["0.0.0.0", "8.8.8.8", "172.32.0.1", "169.254.1.1", "224.0.0.1", "255.255.255.255", "240.0.0.1", "192.168.1", "::1", "", "01.2.3.4"] {
            assert!(!eligible_address(ko), "{ko}");
        }
    }
}
