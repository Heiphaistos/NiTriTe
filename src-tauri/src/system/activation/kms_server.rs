//! kms_server.rs — Répondeur de licence en volume (protocole KMS v4/v5/v6) en Rust
//! natif, écoutant UNIQUEMENT sur 127.0.0.2:1688. Aucun binaire tiers.
//!
//! ⚠️ ÉTAT HONNÊTE (lire le rapport) :
//! La couche DCE/RPC (bind/bind_ack, request/response) et le décodage de l'enveloppe
//! de requête sont implémentés d'après MS-RPCE et les implémentations libres de
//! référence (py-kms / vlmcsd, logique réécrite). Ils sont testés en aller-retour.
//! EN REVANCHE, l'assemblage cryptographique exact de la réponse v6 (dérivation de
//! sel, clé AES fixe, séquence HMAC) est reconstitué depuis la documentation publique
//! et N'A PAS PU ÊTRE PROUVÉ contre un vrai `slmgr` (pas d'accès VM). Il ne faut donc
//! PAS considérer l'activation KMS comme fonctionnelle tant qu'un test en VM ne l'a pas
//! confirmée. Les primitives (AES-128, HMAC-SHA256) sont, elles, prouvées par vecteurs
//! officiels (voir crypto.rs).
//!
//! Sécurité : liaison forcée sur 127.0.0.2 (jamais 0.0.0.0), arrêt propre sans port
//! ni thread orphelin (drapeau d'arrêt + écoute non bloquante).

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use super::crypto::{hmac_sha256, sha256, Aes128};

pub const KMS_BIND_ADDR: &str = "127.0.0.2:1688";

/// Clés AES-128 fixes publiques du protocole KMS (constantes bien connues des
/// implémentations libres). v4 utilise un MAC AES ; v5/v6 chiffrent en AES-128-CBC.
const KMS_KEY_V5: [u8; 16] = [
    0xCD, 0x7E, 0x79, 0x6F, 0x2A, 0xB2, 0x5D, 0xCB, 0x55, 0xFF, 0xC8, 0xEF, 0x83, 0x64, 0xC4, 0x70,
];
const KMS_KEY_V6: [u8; 16] = [
    0xA9, 0x4A, 0x41, 0x95, 0xE2, 0x01, 0x43, 0x2D, 0x9B, 0xCB, 0x46, 0x04, 0x05, 0xD8, 0x4A, 0x21,
];

// UUID de l'interface d'activation KMS (Activation 2.0) et de la syntaxe NDR.
const KMS_IFACE_UUID: [u8; 16] = [
    0x75, 0x21, 0xC8, 0x51, 0x4E, 0x84, 0x50, 0x47, 0xB0, 0xD8, 0xEC, 0x25, 0x55, 0x55, 0xBC, 0x06,
];
const NDR_SYNTAX_UUID: [u8; 16] = [
    0x04, 0x5D, 0x88, 0x8A, 0xEB, 0x1C, 0xC9, 0x11, 0x9F, 0xE8, 0x08, 0x00, 0x2B, 0x10, 0x48, 0x60,
];

// Types de PDU DCE/RPC.
const PT_REQUEST: u8 = 0;
const PT_RESPONSE: u8 = 2;
const PT_BIND: u8 = 11;
const PT_BIND_ACK: u8 = 12;

// ── Petites aides little-endian ─────────────────────────────────────────────

fn u16le(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(o)?, *b.get(o + 1)?]))
}
fn u32le(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*b.get(o)?, *b.get(o + 1)?, *b.get(o + 2)?, *b.get(o + 3)?]))
}
fn push_u16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn push_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// En-tête commun DCE/RPC (16 octets), drep little-endian.
fn rpc_header(ptype: u8, call_id: u32, frag_len: u16) -> Vec<u8> {
    let mut h = Vec::with_capacity(16);
    h.push(5); // rpc_vers
    h.push(0); // rpc_vers_minor
    h.push(ptype);
    h.push(0x03); // pfc_flags : first+last fragment
    h.extend_from_slice(&[0x10, 0x00, 0x00, 0x00]); // packed_drep (LE, ASCII)
    push_u16(&mut h, frag_len);
    push_u16(&mut h, 0); // auth_length
    push_u32(&mut h, call_id);
    h
}

#[derive(Debug)]
struct RpcHead {
    ptype: u8,
    call_id: u32,
}

fn parse_rpc_header(b: &[u8]) -> Option<RpcHead> {
    if b.len() < 16 || b[0] != 5 {
        return None;
    }
    Some(RpcHead {
        ptype: b[2],
        call_id: u32le(b, 12)?,
    })
}

/// Construit un bind_ack acceptant la syntaxe NDR, adresse secondaire "1688".
fn build_bind_ack(call_id: u32, assoc_group: u32) -> Vec<u8> {
    let mut body = Vec::new();
    push_u16(&mut body, 5840); // max_xmit_frag
    push_u16(&mut body, 5840); // max_recv_frag
    push_u32(&mut body, if assoc_group == 0 { 0x0105 } else { assoc_group });
    // sec_addr : "1688\0"
    let port = b"1688\0";
    push_u16(&mut body, port.len() as u16);
    body.extend_from_slice(port);
    // Alignement sur 4 octets (le padding compte depuis le début du PDU = header 16 + body).
    while (16 + body.len()) % 4 != 0 {
        body.push(0);
    }
    // p_result_list : 1 résultat, acceptance (0,0), syntaxe NDR v2.
    body.push(1); // n_results
    body.push(0); // reserved
    push_u16(&mut body, 0); // reserved2
    push_u16(&mut body, 0); // result = acceptance
    push_u16(&mut body, 0); // reason
    body.extend_from_slice(&NDR_SYNTAX_UUID);
    push_u32(&mut body, 2); // NDR version 2.0

    let frag_len = (16 + body.len()) as u16;
    let mut pkt = rpc_header(PT_BIND_ACK, call_id, frag_len);
    pkt.extend_from_slice(&body);
    pkt
}

// ── Requête KMS (enveloppe v4/v5/v6) ────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct KmsRequest {
    pub version_major: u16,
    pub version_minor: u16,
    /// 0x00040000 / 0x00050000 / 0x00060000.
    pub protocol: u32,
    /// clientMachineId (16 octets) tel que reçu — repris verbatim dans la réponse.
    pub client_machine_id: [u8; 16],
    /// requestTime (FILETIME 8 octets) — repris dans la réponse.
    pub request_time: [u8; 8],
    /// requiredClientCount déclaré par le client.
    pub required_count: u32,
    /// Sel de l'enveloppe de la requête (IV du corps chiffré).
    pub salt_c: [u8; 16],
    /// Sel déchiffré (DSaltC) = CBC-déchiffrement du bloc `salt_c` avec IV = `salt_c`.
    pub dsalt_c: [u8; 16],
}

/// HWID du serveur (8 octets) — valeur conventionnelle des serveurs KMS (vlmcsd).
const KMS_HWID: [u8; 8] = [0x36, 0x4F, 0x46, 0x3A, 0x88, 0x63, 0xD3, 0x5F];

/// Aléa non cryptographique (xorshift semé par l'horloge + un compteur de process).
/// Le sel KMS n'est pas une frontière de sécurité (le serveur simule l'activation).
fn fill_random(buf: &mut [u8]) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static CTR: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15);
    let mut x = t ^ CTR.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed).wrapping_mul(0x2545_F491_4F6C_DD1D);
    for b in buf.iter_mut() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *b = (x & 0xff) as u8;
    }
}

/// Décode l'enveloppe de requête KMS à partir du stub RPC. Pour v5/v6 le corps est
/// chiffré en AES-128-CBC (clé fixe, IV = sel) ; v6 utilise l'AES tweaké. On déchiffre
/// le bloc de sel (DSaltC) et la structure de requête (CMID, requestTime, count).
fn decode_kms_request(stub: &[u8]) -> Option<KmsRequest> {
    // Le stub RPC commence DIRECTEMENT par l'enveloppe KMS (vérifié par vidage dans la
    // VM) : BodyLength1(4) BodyLength2(4) versionMinor(2) versionMajor(2) Salt(16)
    // Encrypted(...). PAS de préfixe de taille NDR à sauter — le faire décalait tout
    // et classait la requête v6 comme v4 (réponse fausse depuis le départ).
    let data = stub;
    // versionMinor(2)+versionMajor(2) relus en u32 LE servent de discriminant de version.
    let protocol = u32le(data, 8).unwrap_or(0);
    if protocol == 0x0006_0000 || protocol == 0x0005_0000 {
        let v6 = protocol == 0x0006_0000;
        let salt_c: [u8; 16] = data.get(12..28)?.try_into().ok()?;
        let enc = data.get(28..)?;
        let aes = if v6 { Aes128::new_v6(&KMS_KEY_V6) } else { Aes128::new(&KMS_KEY_V5) };
        // DSaltC = déchiffrement du bloc de sel avec IV NUL (comme vlmcsd
        // AesDecryptCbc(NULL, IV)) = dec_block(SaltC). C'est l'« IV de requête
        // déchiffré » qui sert à RandomXoredIVs et XoredIVs de la réponse v6.
        let dsalt_vec = aes.cbc_decrypt(&[0u8; 16], &salt_c);
        let dsalt_c: [u8; 16] = dsalt_vec.get(0..16)?.try_into().ok()?;
        // Le corps chiffré se déchiffre avec IV = sel (le bloc de ciphertext précédent) ;
        // `full` = kmsRequestStruct : versionMinor(2) versionMajor(2) isClientVm(4)
        // licenseStatus(4) graceTime(4) appId(16) skuId(16) kmsId(16) CMID(16) count(4) time(8)...
        let full = aes.cbc_decrypt(&salt_c, enc);
        let version_minor = u16le(&full, 0).unwrap_or(0);
        let version_major = u16le(&full, 2).unwrap_or(0);
        let cmid_off = 2 + 2 + 4 + 4 + 4 + 16 + 16 + 16; // = 64
        let client_machine_id: [u8; 16] = full.get(cmid_off..cmid_off + 16)?.try_into().ok()?;
        let required_count = u32le(&full, cmid_off + 16).unwrap_or(0);
        let request_time: [u8; 8] = full.get(cmid_off + 20..cmid_off + 28)?.try_into().ok()?;
        Some(KmsRequest {
            version_major, version_minor, protocol, client_machine_id,
            request_time, required_count, salt_c, dsalt_c,
        })
    } else {
        // v4 (en clair).
        let version_minor = u16le(data, 0).unwrap_or(0);
        let version_major = u16le(data, 2).unwrap_or(0);
        let cmid_off = 2 + 2 + 4 + 4 + 4 + 16 + 16 + 16;
        let client_machine_id: [u8; 16] = data.get(cmid_off..cmid_off + 16)?.try_into().ok()?;
        let required_count = u32le(data, cmid_off + 16).unwrap_or(0);
        let request_time: [u8; 8] = data.get(cmid_off + 20..cmid_off + 28)?.try_into().ok()?;
        Some(KmsRequest {
            version_major, version_minor, protocol: 0x0004_0000, client_machine_id,
            request_time, required_count, salt_c: [0; 16], dsalt_c: [0; 16],
        })
    }
}

/// ePID (KMS PID) bien formé en UTF-16LE terminé par un nul.
fn make_epid() -> Vec<u8> {
    let s = "03612-00206-471-000000-03-1036-9600.0000-2512025";
    let mut out = Vec::with_capacity(s.len() * 2 + 2);
    for c in s.encode_utf16() {
        out.extend_from_slice(&c.to_le_bytes());
    }
    out.extend_from_slice(&[0, 0]); // nul terminal UTF-16
    out
}

/// kmsResponseStruct (clair) : versionMinor versionMajor epidLen kmsEpid CMID responseTime
/// currentCount vLActivationInterval vLRenewalInterval. currentCount = required*2 (seuil franchi).
fn build_response_base(req: &KmsRequest) -> Vec<u8> {
    let epid = make_epid();
    let mut b = Vec::new();
    push_u16(&mut b, req.version_minor);
    push_u16(&mut b, req.version_major);
    push_u32(&mut b, epid.len() as u32); // epidLen = octets du PID (nul inclus)
    b.extend_from_slice(&epid);
    b.extend_from_slice(&req.client_machine_id); // CMID repris
    b.extend_from_slice(&req.request_time); // responseTime = requestTime
    let current = req.required_count.saturating_mul(2).max(50);
    push_u32(&mut b, current);
    push_u32(&mut b, 120); // VLActivationInterval (minutes)
    push_u32(&mut b, 10080); // VLRenewalInterval (minutes, 7 j)
    b
}

/// Clé HMAC v6 : créneau horaire dérivé du requestTime (TIME_C1/2/3), SHA-256,
/// puis les 16 derniers octets. Identique à vlmcsd CreateV6Hmac (tolérance 0).
fn v6_hmac(request_time: u64, plaintext_sans_hmac: &[u8]) -> [u8; 16] {
    const TIME_C1: u64 = 0x0000_0022_8168_89BD;
    const TIME_C2: u64 = 0x0000_0020_8CBA_B5ED;
    const TIME_C3: u64 = 0x3156_CD5A_C628_477A;
    let time_slot = (request_time / TIME_C1).wrapping_mul(TIME_C2).wrapping_add(TIME_C3);
    let hashed = sha256(&time_slot.to_le_bytes());
    let key = &hashed[16..32]; // 16 derniers octets = clé HMAC
    let digest = hmac_sha256(key, plaintext_sans_hmac);
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[16..32]); // 16 derniers octets = HMAC
    out
}

/// Assemble la réponse KMS v5/v6 — portage octet-à-octet de vlmcsd CreateResponseV6.
/// Disposition (RESPONSE_V6) : Version(4, EN CLAIR) puis, CHIFFRÉ en AES-128-CBC à IV
/// NUL (v6 = AES tweaké) : IV(16) ResponseBase RandomXoredIVs(16) Hash(32) HwId(8)
/// XoredIVs(16) HMAC(16) + padding PKCS7. Le HMAC couvre le clair de IV à XoredIVs.
fn build_kms_response(req: &KmsRequest) -> Vec<u8> {
    let v6 = req.protocol == 0x0006_0000;
    if !v6 && req.protocol != 0x0005_0000 {
        return build_response_base(req); // v4 non assemblé ici
    }
    let aes = if v6 { Aes128::new_v6(&KMS_KEY_V6) } else { Aes128::new(&KMS_KEY_V5) };

    // Sel aléatoire -> Hash = SHA-256(random). RandomXoredIVs = random ^ DSaltC.
    let mut random = [0u8; 16];
    fill_random(&mut random);
    let hash = sha256(&random);
    let mut random_xored = [0u8; 16];
    for i in 0..16 {
        random_xored[i] = random[i] ^ req.dsalt_c[i];
    }
    // XoredIVs = IV de requête déchiffré (DSaltC) ; HwId fixe.
    let xored_ivs = req.dsalt_c;

    // IV de réponse (SaltS) : chiffré AVEC le reste (IV nul).
    let mut salt_s = [0u8; 16];
    fill_random(&mut salt_s);

    // Clair à chiffrer, SANS le HMAC d'abord : IV + base + randomXored + hash + hwid + xoredIVs.
    let base = build_response_base(req);
    let mut plain = Vec::new();
    plain.extend_from_slice(&salt_s); // IV (sera chiffré, IV nul)
    plain.extend_from_slice(&base); // ResponseBase (Version, PIDSize, ePID, CMID, time, count, intervalles)
    if v6 {
        plain.extend_from_slice(&random_xored);
        plain.extend_from_slice(&hash);
        plain.extend_from_slice(&KMS_HWID);
        plain.extend_from_slice(&xored_ivs);
        // HMAC sur tout le clair précédent (IV..XoredIVs), puis ajouté.
        let hmac = v6_hmac(u64::from_le_bytes(req.request_time), &plain);
        plain.extend_from_slice(&hmac);
    } else {
        // v5 : randomXored + hash seulement (pas de hwid/xored/hmac).
        plain.extend_from_slice(&random_xored);
        plain.extend_from_slice(&hash);
    }

    // Padding PKCS7 puis chiffrement CBC à IV NUL.
    let pad = 16 - (plain.len() % 16);
    plain.extend(std::iter::repeat(pad as u8).take(pad));
    let encrypted = aes.cbc_encrypt(&[0u8; 16], &plain);

    // RESPONSE_V6 = Version(4 en clair) + chiffré.
    let mut out = Vec::with_capacity(4 + encrypted.len());
    push_u16(&mut out, req.version_minor);
    push_u16(&mut out, req.version_major);
    out.extend_from_slice(&encrypted);
    out
}

/// Emballe la réponse KMS dans un PDU RPC response, exactement comme vlmcsd (NDR32).
/// En-tête NDR du paramètre [out] : DataLength, DataSizeMax(0x00020000 = pointeur
/// référent), DataSizeIs, puis les données, puis le code retour RPC (0) et un
/// padding d'alignement 32 bits. Un en-tête NDR incomplet fait rejeter slmgr
/// (RPC_NT_INVALID_TAG 0xC0020017).
fn build_rpc_response(call_id: u32, cont_id: u16, stub: &[u8]) -> Vec<u8> {
    let data_len = stub.len() as u32;
    // Ndr (12) + données + code retour (4) + padding.
    let mut ndr = Vec::new();
    push_u32(&mut ndr, data_len); // Ndr.DataLength
    push_u32(&mut ndr, 0x0002_0000); // Ndr.DataSizeMax (référent du pointeur [out])
    push_u32(&mut ndr, data_len); // Ndr.DataSizeIs
    ndr.extend_from_slice(stub); // données = enveloppe KMS
    push_u32(&mut ndr, 0); // code retour RPC = 0 (succès)
    let len = ndr.len() as u32;
    let pad = ((!len & 3) + 1) & 3; // alignement 32 bits (comme Windows RPC)
    ndr.extend(std::iter::repeat(0u8).take(pad as usize));

    let mut body = Vec::new();
    push_u32(&mut body, ndr.len() as u32); // AllocHint = Ndr + données + code + pad
    push_u16(&mut body, cont_id); // ContextId
    body.push(0); // CancelCount
    body.push(0); // Pad1
    body.extend_from_slice(&ndr);

    let frag_len = (16 + body.len()) as u16;
    let mut pkt = rpc_header(PT_RESPONSE, call_id, frag_len);
    pkt.extend_from_slice(&body);
    pkt
}

/// Lit un PDU complet (header 16 o + corps selon frag_length) depuis le flux.
fn read_pdu(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut head = [0u8; 16];
    stream.read_exact(&mut head)?;
    let frag_len = u16::from_le_bytes([head[8], head[9]]) as usize;
    if frag_len < 16 || frag_len > 16 * 1024 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "frag_length invalide"));
    }
    let mut pkt = head.to_vec();
    pkt.resize(frag_len, 0);
    stream.read_exact(&mut pkt[16..])?;
    Ok(pkt)
}

/// Traite une connexion : bind → bind_ack, puis request → response.
fn handle_connection(mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
    loop {
        let pkt = match read_pdu(&mut stream) {
            Ok(p) => p,
            Err(_) => return,
        };
        let Some(head) = parse_rpc_header(&pkt) else { return };
        match head.ptype {
            PT_BIND => {
                // assoc_group à l'offset 16+4 (après max_xmit/recv frag).
                let assoc = u32le(&pkt, 20).unwrap_or(0);
                // Vérifie que c'est bien l'interface KMS (sinon on répond quand même, inoffensif).
                let _is_kms = pkt.windows(16).any(|w| w == KMS_IFACE_UUID);
                let resp = build_bind_ack(head.call_id, assoc);
                if stream.write_all(&resp).is_err() {
                    return;
                }
            }
            PT_REQUEST => {
                // body: alloc_hint(4) cont_id(2) opnum(2) stub...
                let cont_id = u16le(&pkt, 20).unwrap_or(0);
                let stub = pkt.get(24..).unwrap_or(&[]);
                let Some(req) = decode_kms_request(stub) else { return };
                let stub_resp = build_kms_response(&req);
                let resp = build_rpc_response(head.call_id, cont_id, &stub_resp);
                let _ = stream.write_all(&resp);
                return; // une activation = un aller-retour
            }
            _ => return,
        }
    }
}

/// Serveur KMS local : thread d'écoute + drapeau d'arrêt. `Drop` garantit l'arrêt
/// (aucun thread ni port orphelin à la fermeture de l'app).
pub struct KmsServer {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl KmsServer {
    /// Démarre l'écoute sur 127.0.0.2:1688. Erreur si le port est déjà pris.
    pub fn start() -> std::io::Result<Self> {
        let addr = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 2), 1688);
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        let handle = std::thread::spawn(move || {
            while !stop_thread.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        std::thread::spawn(move || handle_connection(stream));
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(KmsServer { stop, handle: Some(handle) })
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for KmsServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_ack_bien_forme() {
        let p = build_bind_ack(7, 0);
        let h = parse_rpc_header(&p).unwrap();
        assert_eq!(h.ptype, PT_BIND_ACK);
        assert_eq!(h.call_id, 7);
        // frag_length (offset 8) doit égaler la taille réelle du paquet.
        assert_eq!(u16le(&p, 8).unwrap() as usize, p.len());
        // acceptance présente + syntaxe NDR.
        assert!(p.windows(16).any(|w| w == NDR_SYNTAX_UUID));
    }

    #[test]
    fn requete_v6_aller_retour() {
        // Fabrique une base de requête v6, la chiffre, et vérifie que decode retrouve le CMID.
        let cmid = [0xABu8; 16];
        let time = [0x11u8, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        let mut base = vec![0u8; 64];
        base.extend_from_slice(&cmid);
        base.extend_from_slice(&3u32.to_le_bytes()); // required_count
        base.extend_from_slice(&time);
        while base.len() % 16 != 0 {
            base.push(0);
        }
        let salt = [0x5u8; 16];
        let enc = Aes128::new_v6(&KMS_KEY_V6).cbc_encrypt(&salt, &base);
        // Format filaire réel : bodyLength1(4) bodyLength2(4) version(4) salt(16) enc.
        let mut stub = Vec::new();
        let body_len = (4 + 16 + enc.len()) as u32;
        stub.extend_from_slice(&body_len.to_le_bytes()); // bodyLength1
        stub.extend_from_slice(&body_len.to_le_bytes()); // bodyLength2
        stub.extend_from_slice(&0x0006_0000u32.to_le_bytes()); // versionMinor(0)+versionMajor(6)
        stub.extend_from_slice(&salt);
        stub.extend_from_slice(&enc);

        let req = decode_kms_request(&stub).expect("décodage v6");
        assert_eq!(req.protocol, 0x0006_0000);
        assert_eq!(req.client_machine_id, cmid);
        assert_eq!(req.request_time, time);
        assert_eq!(req.required_count, 3);
    }

    #[test]
    fn reponse_v6_enveloppe_structuree() {
        let req = KmsRequest {
            version_major: 6,
            version_minor: 0,
            protocol: 0x0006_0000,
            client_machine_id: [0x1; 16],
            request_time: [0x2; 8],
            required_count: 10,
            salt_c: [0x7; 16],
            dsalt_c: [0x9; 16],
        };
        let resp = build_kms_response(&req);
        // RESPONSE_V6 = versionMinor(2) versionMajor(2) EN CLAIR + chiffré (multiple de 16).
        assert_eq!(u16le(&resp, 0).unwrap(), 0, "versionMinor en clair");
        assert_eq!(u16le(&resp, 2).unwrap(), 6, "versionMajor en clair");
        assert!(resp.len() > 4 + 16, "réponse non vide");
        assert_eq!((resp.len() - 4) % 16, 0, "corps chiffré multiple de 16");
    }
}
