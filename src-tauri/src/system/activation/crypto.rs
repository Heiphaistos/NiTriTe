//! crypto.rs — Primitives cryptographiques autonomes pour le répondeur de licence
//! en volume (KMS v4/v5/v6). Aucune crate externe : AES-128 est implémenté ici
//! (algorithme FIPS-197, fini et déterministe), HMAC-SHA256 est bâti sur `sha2`
//! déjà présent dans le projet. Chaque primitive a un vecteur de test officiel
//! (FIPS-197 pour AES, RFC 4231 pour HMAC) : c'est ce qui rend ce fichier
//! vérifiable sans machine cible.
//!
//! Note d'honnêteté : ces primitives sont prouvées en isolation. Leur usage exact
//! dans le protocole v6 (dérivation de sel/clé) est porté depuis les docs publiques
//! et reste À PROUVER contre un vrai `slmgr` (voir kms_server.rs et le rapport).

use sha2::{Digest, Sha256};

// ── SHA-256 / HMAC-SHA256 ───────────────────────────────────────────────────

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    let out = h.finalize();
    let mut r = [0u8; 32];
    r.copy_from_slice(&out);
    r
}

/// HMAC-SHA256 (RFC 2104). Bloc de 64 octets.
pub fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut k0 = [0u8; BLOCK];
    if key.len() > BLOCK {
        k0[..32].copy_from_slice(&sha256(key));
    } else {
        k0[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] ^= k0[i];
        opad[i] ^= k0[i];
    }
    let mut inner = Vec::with_capacity(BLOCK + msg.len());
    inner.extend_from_slice(&ipad);
    inner.extend_from_slice(msg);
    let inner_hash = sha256(&inner);
    let mut outer = Vec::with_capacity(BLOCK + 32);
    outer.extend_from_slice(&opad);
    outer.extend_from_slice(&inner_hash);
    sha256(&outer)
}

// ── AES-128 (FIPS-197) ──────────────────────────────────────────────────────

const SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

fn inv_sbox() -> [u8; 256] {
    let mut inv = [0u8; 256];
    for (i, &b) in SBOX.iter().enumerate() {
        inv[b as usize] = i as u8;
    }
    inv
}

const RCON: [u8; 11] = [0x00, 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36];

/// Multiplication dans GF(2^8) (polynôme AES 0x11b).
fn gmul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0u8;
    for _ in 0..8 {
        if b & 1 != 0 {
            p ^= a;
        }
        let hi = a & 0x80;
        a <<= 1;
        if hi != 0 {
            a ^= 0x1b;
        }
        b >>= 1;
    }
    p
}

/// AES-128 : 11 sous-clés de 16 octets (clé maître + 10 tours).
pub struct Aes128 {
    round_keys: [[u8; 16]; 11],
    /// Variante « v6 » du protocole KMS : AES NON standard. Aux tours 4/6/8,
    /// l'octet 0 de l'état est XORé (0x73/0x09/0xE4) entre MixColumns et
    /// AddRoundKey (chemin direct), et entre AddRoundKey et InvMixColumns
    /// (chemin inverse). C'est ce tweak qui distingue le KMS v6 : un AES
    /// standard produit une réponse que `slmgr /ato` rejette (0xC004F074).
    v6: bool,
}

/// Tweak de tour KMS v6 : XOR de l'octet 0 de l'état selon le numéro de tour.
#[inline]
fn kms_v6_round_tweak(state: &mut [u8; 16], round: usize) {
    match round {
        4 => state[0] ^= 0x73,
        6 => state[0] ^= 0x09,
        8 => state[0] ^= 0xE4,
        _ => {}
    }
}

impl Aes128 {
    pub fn new(key: &[u8; 16]) -> Self {
        Self::build(key, false)
    }

    /// AES-128 avec le tweak de round du protocole KMS v6.
    pub fn new_v6(key: &[u8; 16]) -> Self {
        Self::build(key, true)
    }

    fn build(key: &[u8; 16], v6: bool) -> Self {
        let mut w = [[0u8; 4]; 44];
        for i in 0..4 {
            w[i] = [key[4 * i], key[4 * i + 1], key[4 * i + 2], key[4 * i + 3]];
        }
        for i in 4..44 {
            let mut temp = w[i - 1];
            if i % 4 == 0 {
                temp.rotate_left(1);
                for t in temp.iter_mut() {
                    *t = SBOX[*t as usize];
                }
                temp[0] ^= RCON[i / 4];
            }
            for j in 0..4 {
                w[i][j] = w[i - 4][j] ^ temp[j];
            }
        }
        let mut round_keys = [[0u8; 16]; 11];
        for r in 0..11 {
            for c in 0..4 {
                for j in 0..4 {
                    round_keys[r][4 * c + j] = w[4 * r + c][j];
                }
            }
        }
        Aes128 { round_keys, v6 }
    }

    fn add_round_key(state: &mut [u8; 16], rk: &[u8; 16]) {
        for i in 0..16 {
            state[i] ^= rk[i];
        }
    }

    pub fn encrypt_block(&self, input: &[u8; 16]) -> [u8; 16] {
        let mut s = *input;
        Self::add_round_key(&mut s, &self.round_keys[0]);
        for round in 1..10 {
            for b in s.iter_mut() {
                *b = SBOX[*b as usize];
            }
            Self::shift_rows(&mut s);
            Self::mix_columns(&mut s);
            if self.v6 {
                kms_v6_round_tweak(&mut s, round);
            }
            Self::add_round_key(&mut s, &self.round_keys[round]);
        }
        for b in s.iter_mut() {
            *b = SBOX[*b as usize];
        }
        Self::shift_rows(&mut s);
        Self::add_round_key(&mut s, &self.round_keys[10]);
        s
    }

    pub fn decrypt_block(&self, input: &[u8; 16]) -> [u8; 16] {
        let inv = inv_sbox();
        let mut s = *input;
        Self::add_round_key(&mut s, &self.round_keys[10]);
        for round in (1..10).rev() {
            Self::inv_shift_rows(&mut s);
            for b in s.iter_mut() {
                *b = inv[*b as usize];
            }
            Self::add_round_key(&mut s, &self.round_keys[round]);
            if self.v6 {
                kms_v6_round_tweak(&mut s, round);
            }
            Self::inv_mix_columns(&mut s);
        }
        Self::inv_shift_rows(&mut s);
        for b in s.iter_mut() {
            *b = inv[*b as usize];
        }
        Self::add_round_key(&mut s, &self.round_keys[0]);
        s
    }

    // L'état AES est rangé en colonnes : octet (row r, col c) = state[4*c + r].
    fn shift_rows(s: &mut [u8; 16]) {
        let o = *s;
        for c in 0..4 {
            for r in 0..4 {
                s[4 * c + r] = o[4 * ((c + r) % 4) + r];
            }
        }
    }

    fn inv_shift_rows(s: &mut [u8; 16]) {
        let o = *s;
        for c in 0..4 {
            for r in 0..4 {
                s[4 * c + r] = o[4 * ((c + 4 - r) % 4) + r];
            }
        }
    }

    fn mix_columns(s: &mut [u8; 16]) {
        for c in 0..4 {
            let col = [s[4 * c], s[4 * c + 1], s[4 * c + 2], s[4 * c + 3]];
            s[4 * c] = gmul(col[0], 2) ^ gmul(col[1], 3) ^ col[2] ^ col[3];
            s[4 * c + 1] = col[0] ^ gmul(col[1], 2) ^ gmul(col[2], 3) ^ col[3];
            s[4 * c + 2] = col[0] ^ col[1] ^ gmul(col[2], 2) ^ gmul(col[3], 3);
            s[4 * c + 3] = gmul(col[0], 3) ^ col[1] ^ col[2] ^ gmul(col[3], 2);
        }
    }

    fn inv_mix_columns(s: &mut [u8; 16]) {
        for c in 0..4 {
            let col = [s[4 * c], s[4 * c + 1], s[4 * c + 2], s[4 * c + 3]];
            s[4 * c] = gmul(col[0], 14) ^ gmul(col[1], 11) ^ gmul(col[2], 13) ^ gmul(col[3], 9);
            s[4 * c + 1] = gmul(col[0], 9) ^ gmul(col[1], 14) ^ gmul(col[2], 11) ^ gmul(col[3], 13);
            s[4 * c + 2] = gmul(col[0], 13) ^ gmul(col[1], 9) ^ gmul(col[2], 14) ^ gmul(col[3], 11);
            s[4 * c + 3] = gmul(col[0], 11) ^ gmul(col[1], 13) ^ gmul(col[2], 9) ^ gmul(col[3], 14);
        }
    }

    /// CBC sans padding (les blocs KMS sont déjà multiples de 16).
    pub fn cbc_encrypt(&self, iv: &[u8; 16], data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len());
        let mut prev = *iv;
        for chunk in data.chunks(16) {
            let mut block = [0u8; 16];
            block[..chunk.len()].copy_from_slice(chunk);
            for i in 0..16 {
                block[i] ^= prev[i];
            }
            let enc = self.encrypt_block(&block);
            out.extend_from_slice(&enc);
            prev = enc;
        }
        out
    }

    pub fn cbc_decrypt(&self, iv: &[u8; 16], data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len());
        let mut prev = *iv;
        for chunk in data.chunks(16) {
            let mut block = [0u8; 16];
            block[..chunk.len()].copy_from_slice(chunk);
            let dec = self.decrypt_block(&block);
            let mut plain = [0u8; 16];
            for i in 0..16 {
                plain[i] = dec[i] ^ prev[i];
            }
            out.extend_from_slice(&plain);
            prev = block;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    /// FIPS-197 Appendix B : vecteur de référence AES-128.
    #[test]
    fn aes128_fips197_known_answer() {
        let key: [u8; 16] = hex("000102030405060708090a0b0c0d0e0f").try_into().unwrap();
        let pt: [u8; 16] = hex("00112233445566778899aabbccddeeff").try_into().unwrap();
        let expect = hex("69c4e0d86a7b0430d8cdb78070b4c55a");
        let aes = Aes128::new(&key);
        let ct = aes.encrypt_block(&pt);
        assert_eq!(&ct[..], &expect[..], "chiffrement AES-128 FIPS-197");
        assert_eq!(aes.decrypt_block(&ct), pt, "déchiffrement = identité");
    }

    #[test]
    fn aes128_cbc_round_trip() {
        let key = [0x11u8; 16];
        let iv = [0x22u8; 16];
        let data: Vec<u8> = (0u8..48).collect();
        let aes = Aes128::new(&key);
        let enc = aes.cbc_encrypt(&iv, &data);
        assert_eq!(aes.cbc_decrypt(&iv, &enc), data);
    }

    /// AES v6 tweaké : le chiffrement doit être l'inverse EXACT du déchiffrement
    /// (sinon la réponse KMS chiffrée ne correspond pas à ce que MS déchiffre).
    #[test]
    fn aes128_v6_encrypt_is_inverse_of_decrypt() {
        let key = [0x33u8; 16];
        let aes = Aes128::new_v6(&key);
        let block = [0x44u8; 16];
        assert_eq!(aes.decrypt_block(&aes.encrypt_block(&block)), block, "dec(enc(x)) = x (v6)");
        assert_eq!(aes.encrypt_block(&aes.decrypt_block(&block)), block, "enc(dec(x)) = x (v6)");
        let data: Vec<u8> = (0u8..64).collect();
        let iv = [0u8; 16];
        assert_eq!(aes.cbc_decrypt(&iv, &aes.cbc_encrypt(&iv, &data)), data, "CBC v6 round-trip");
    }

    /// RFC 4231 cas de test 2.
    #[test]
    fn hmac_sha256_rfc4231_case2() {
        let mac = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        let expect = hex("5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
        assert_eq!(&mac[..], &expect[..]);
    }
}
