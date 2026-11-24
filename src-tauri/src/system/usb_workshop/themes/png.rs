//! PNG minimal sans dépendance (CRC-32, zlib en blocs stored).
//! Port de TokaISO 7.9 (MIT, Cédrick Roy) : ventoy_design.py (png_chunk).

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

pub fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// Flux zlib en blocs « stored ».
// ponytail: pas de compression (aucune crate zlib directe) : un fond 640x480 pèse ~0,9 Mio
// au lieu de quelques Kio ; brancher un vrai deflate si la taille du thème sur la clé gêne.
pub fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let mut chunks: Vec<&[u8]> = data.chunks(65535).collect();
    if chunks.is_empty() {
        chunks.push(&[]);
    }
    let last = chunks.len() - 1;
    for (i, chunk) in chunks.iter().enumerate() {
        out.push(u8::from(i == last));
        let len = chunk.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(chunk);
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn png_chunk(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut body = kind.to_vec();
    body.extend_from_slice(payload);
    let mut out = (payload.len() as u32).to_be_bytes().to_vec();
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
    out
}

pub fn png(width: u32, height: u32, color_type: u8, raw: &[u8]) -> Vec<u8> {
    let mut header = width.to_be_bytes().to_vec();
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, color_type, 0, 0, 0]);
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    out.extend(png_chunk(b"IHDR", &header));
    out.extend(png_chunk(b"IDAT", &zlib_stored(raw)));
    out.extend(png_chunk(b"IEND", &[]));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_has_valid_signature_and_crc() {
        let data = png(1, 1, 2, &[0, 1, 2, 3]);
        assert!(data.starts_with(b"\x89PNG\r\n\x1a\n"));
        let ihdr = &data[8..8 + 25];
        assert_eq!(&ihdr[4..8], b"IHDR");
        let crc = u32::from_be_bytes([ihdr[21], ihdr[22], ihdr[23], ihdr[24]]);
        assert_eq!(crc, crc32(&ihdr[4..21]));
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn zlib_stored_roundtrip_markers() {
        assert_eq!(adler32(b"Wikipedia"), 0x11e6_0398);
        let big = vec![7u8; 70_000];
        let z = zlib_stored(&big);
        assert_eq!(&z[..2], &[0x78, 0x01]);
        assert_eq!(z[2], 0, "premier bloc non final");
        assert_eq!(z.len(), 2 + 2 * 5 + 70_000 + 4);
    }

}
