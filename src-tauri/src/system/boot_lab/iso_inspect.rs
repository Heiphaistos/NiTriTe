//! Lecture des descripteurs ISO 9660 / UDF et des modes de démarrage El Torito.
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : library.py (inspect_iso, partie format + démarrage).

use serde::Serialize;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct IsoReport {
    /// « ISO 9660 », « UDF » ou « ISO à confirmer ».
    pub format: String,
    /// « BIOS » et/ou « UEFI ».
    pub boot: Vec<String>,
}

fn read_at<S: Read + Seek>(stream: &mut S, offset: u64, length: u64, limit: u64) -> Vec<u8> {
    if offset >= limit {
        return Vec::new();
    }
    let length = length.min(limit - offset);
    let mut buf = vec![0u8; length as usize];
    if stream.seek(SeekFrom::Start(offset)).is_err() {
        return Vec::new();
    }
    let mut filled = 0;
    while filled < buf.len() {
        match stream.read(&mut buf[filled..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => filled += n,
        }
    }
    buf.truncate(filled);
    buf
}

pub fn inspect_stream<S: Read + Seek>(stream: &mut S, limit: u64) -> IsoReport {
    let sectors_count = (limit.saturating_sub(32768) / 2048).min(16);
    let sectors = read_at(stream, 32768, sectors_count * 2048, limit);
    let (mut root, mut catalog) = (false, 0u64);
    for sector in sectors.chunks_exact(2048) {
        if &sector[1..6] != b"CD001" {
            continue;
        }
        if sector[0] == 1 {
            root = true;
        } else if sector[0] == 0 {
            let id: &[u8] = &sector[7..39];
            let trimmed = id.iter().rposition(|b| *b != 0 && *b != b' ').map(|i| &id[..=i]).unwrap_or(&[]);
            if trimmed == b"EL TORITO SPECIFICATION" {
                let lba = u32::from_le_bytes([sector[71], sector[72], sector[73], sector[74]]);
                catalog = u64::from(lba) * 2048;
            }
        }
    }
    let udf = sectors.windows(5).any(|w| w == b"NSR02" || w == b"NSR03");
    let format = if root { "ISO 9660" } else if udf { "UDF" } else { "ISO à confirmer" };
    let mut boot = Vec::new();
    if catalog > 0 {
        let entries = read_at(stream, catalog, 2048, limit);
        if entries.len() == 2048 && entries[0] == 1 && entries[30..32] == [0x55, 0xaa] {
            let mut platform = entries[1];
            let mut modes = Vec::new();
            for entry in entries[32..].chunks_exact(32) {
                if entry[0] == 0x90 || entry[0] == 0x91 {
                    platform = entry[1];
                } else if entry[0] == 0x88 && u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) > 0 {
                    modes.push(platform);
                }
            }
            for (code, name) in [(0u8, "BIOS"), (0xef, "UEFI")] {
                if modes.contains(&code) {
                    boot.push(name.to_string());
                }
            }
        }
    }
    IsoReport { format: format.into(), boot }
}

pub fn inspect(path: &Path) -> super::R<IsoReport> {
    let mut file = std::fs::File::open(path).map_err(|_| "L’image ISO est illisible.".to_string())?;
    let limit = file.metadata().map(|m| m.len()).unwrap_or(0);
    Ok(inspect_stream(&mut file, limit))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn iso(bios: bool, uefi: bool) -> Vec<u8> {
        let mut data = vec![0u8; 2048 * 24];
        let pvd = 16 * 2048;
        data[pvd] = 1;
        data[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
        let brvd = 17 * 2048;
        data[brvd + 1..brvd + 6].copy_from_slice(b"CD001");
        data[brvd + 7..brvd + 30].copy_from_slice(b"EL TORITO SPECIFICATION");
        data[brvd + 71..brvd + 75].copy_from_slice(&20u32.to_le_bytes());
        let cat = 20 * 2048;
        data[cat] = 1;
        data[cat + 30] = 0x55;
        data[cat + 31] = 0xaa;
        if bios {
            data[cat + 32] = 0x88;
            data[cat + 40..cat + 44].copy_from_slice(&30u32.to_le_bytes());
        }
        if uefi {
            data[cat + 64] = 0x91;
            data[cat + 65] = 0xef;
            data[cat + 96] = 0x88;
            data[cat + 104..cat + 108].copy_from_slice(&40u32.to_le_bytes());
        }
        data
    }

    #[test]
    fn hybride_bios_uefi() {
        let data = iso(true, true);
        let r = inspect_stream(&mut Cursor::new(&data), data.len() as u64);
        assert_eq!(r.format, "ISO 9660");
        assert_eq!(r.boot, vec!["BIOS", "UEFI"]);
    }

    #[test]
    fn uefi_seul_et_inconnu() {
        let data = iso(false, true);
        assert_eq!(inspect_stream(&mut Cursor::new(&data), data.len() as u64).boot, vec!["UEFI"]);
        let blank = vec![0u8; 4096];
        let r = inspect_stream(&mut Cursor::new(&blank), 4096);
        assert_eq!(r.format, "ISO à confirmer");
        assert!(r.boot.is_empty());
    }
}
