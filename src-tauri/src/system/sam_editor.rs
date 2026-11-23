//! sam_editor.rs — Éditeur de comptes Windows HORS LIGNE (type NTPWEdit / chntpw / Lockpick)
//!
//! Tout en pur Rust : montage de la ruche SAM offline via `reg load`, lecture/écriture
//! des valeurs REG_BINARY `F` / `V` / `C` via `reg query` / `reg add`. Aucun exe tiers,
//! aucune crate de parsing de ruche.
//!
//! Structures SAM (confirmées sur les en-têtes chntpw `ntreg.h` / `sam.h`, 140201) :
//!   - `...\Users\<RID>\F`  (≥ 0x48 o) : ACB flags = u16 LE à 0x38.
//!   - `...\Users\<RID>\V`  (≥ 0xCC o) : en-tête de 0xCC o de triplets {ofs,len,…} ;
//!     données à 0xCC. LM : ofs 0x9c / len 0xa0 ; NT : ofs 0xa8 / len 0xac ;
//!     nom d'utilisateur : ofs 0x0c / len 0x10 (UTF-16LE).
//!   - `...\Builtin\Aliases\<GID>\C` : members_ofs 0x28 / members_len 0x2c /
//!     grp_members (compte) 0x30 ; données à 0x34 ; SIDs membres à 0x34+members_ofs.
//!   - SID machine : `...\Account\V` (accountdb_V) sid_ofs 0x38 / sid_len 0x3c ;
//!     données à 0x40 → SID = [0x40+sid_ofs .. +(sid_len+4)] (24 o, S-1-5-21-X-Y-Z).
//!
//! ⚠ Les bits ACB du brief (LOCKOUT 0x0010, PASSWD_NOTREQD 0x0020) étaient FAUX :
//!    valeurs réelles DISABLED 0x0001, AUTOLOCK 0x0400, PWNOTREQ 0x0004 (0x0010 = NORMAL).
//!    On utilise les vraies valeurs, sinon on corromprait le type de compte.

use serde::{Deserialize, Serialize};
use crate::error::NiTriTeError;

// ── Constantes de structure ───────────────────────────────────────────────────

const ACB_DISABLED: u16 = 0x0001;
const ACB_AUTOLOCK: u16 = 0x0400;

const F_ACB_OFS: usize = 0x38; // u16 LE
const F_MIN_LEN: usize = 0x3a;

const V_DATA_BASE: usize = 0xcc;
const V_USERNAME_OFS: usize = 0x0c;
const V_USERNAME_LEN: usize = 0x10;
const V_LMPW_LEN: usize = 0xa0;
const V_NTPW_LEN: usize = 0xac;

const C_MEMBERS_OFS: usize = 0x28;
const C_MEMBERS_LEN: usize = 0x2c;
const C_GRP_MEMBERS: usize = 0x30;
const C_DATA_BASE: usize = 0x34;

const ACCOUNTV_SID_OFS: usize = 0x38;
const ACCOUNTV_SID_LEN: usize = 0x3c;
const ACCOUNTV_DATA_BASE: usize = 0x40;

/// Point de montage dédié (distinct de `OFFLINE_SAM` de list_offline_users pour éviter
/// toute collision si les deux tournent).
#[cfg(target_os = "windows")]
const MOUNT: &str = r"HKLM\NITRITE_SAM";
/// GID du groupe Administrateurs intégré (Builtin\Aliases\00000220 = RID 544).
#[cfg(target_os = "windows")]
const GID_ADMINS: &str = "00000220";

// ── Structures exposées ────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SamUser {
    pub username: String,
    pub rid: u32,
    pub enabled: bool,
    pub locked: bool,
    pub is_admin: bool,
    pub password_blank: bool,
    pub is_builtin: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SamActionResult {
    pub success: bool,
    pub message: String,
    pub backup_path: String,
}

// ── Fonctions PURES (byte-manipulation, unit-testées sans registre) ────────────

fn u32_le(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn write_u32_le(b: &mut [u8], o: usize, v: u32) -> bool {
    match b.get_mut(o..o + 4) {
        Some(slot) => { slot.copy_from_slice(&v.to_le_bytes()); true }
        None => false,
    }
}

fn read_acb(f: &[u8]) -> Option<u16> {
    f.get(F_ACB_OFS..F_ACB_OFS + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}

fn write_acb(f: &mut [u8], acb: u16) -> bool {
    match f.get_mut(F_ACB_OFS..F_ACB_OFS + 2) {
        Some(slot) => { slot.copy_from_slice(&acb.to_le_bytes()); true }
        None => false,
    }
}

/// Efface les bits DISABLE+AUTOLOCK (déverrouiller) ou pose DISABLE (désactiver),
/// ou efface DISABLE seul (activer). Renvoie false si la valeur F est trop courte.
fn apply_acb(f: &mut [u8], op: AcbOp) -> bool {
    let Some(mut acb) = read_acb(f) else { return false };
    match op {
        AcbOp::Enable => acb &= !ACB_DISABLED,
        AcbOp::Disable => acb |= ACB_DISABLED,
        AcbOp::Unlock => acb &= !(ACB_DISABLED | ACB_AUTOLOCK),
    }
    write_acb(f, acb)
}

#[derive(Clone, Copy)]
enum AcbOp { Enable, Disable, Unlock }

/// Blanchit le mot de passe : met les longueurs de hash LM et NT à 0 (méthode NTPWEdit /
/// chntpw : « setting hash lengths to zero seems to make NT think it is blank »).
fn blank_password(v: &mut [u8]) -> bool {
    if v.len() < V_DATA_BASE { return false; }
    write_u32_le(v, V_LMPW_LEN, 0) && write_u32_le(v, V_NTPW_LEN, 0)
}

fn nt_len(v: &[u8]) -> u32 { u32_le(v, V_NTPW_LEN).unwrap_or(0) }

fn extract_username(v: &[u8]) -> String {
    let ofs = u32_le(v, V_USERNAME_OFS).unwrap_or(0) as usize;
    let len = u32_le(v, V_USERNAME_LEN).unwrap_or(0) as usize;
    let start = V_DATA_BASE.saturating_add(ofs);
    if len == 0 || start.saturating_add(len) > v.len() { return String::new(); }
    let u16s: Vec<u16> = v[start..start + len]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&u16s)
}

/// SID machine (24 o) depuis la valeur V du domaine Account.
fn extract_machine_sid(account_v: &[u8]) -> Option<Vec<u8>> {
    let sid_ofs = u32_le(account_v, ACCOUNTV_SID_OFS)? as usize;
    let sid_len = u32_le(account_v, ACCOUNTV_SID_LEN)? as usize;
    let start = ACCOUNTV_DATA_BASE.checked_add(sid_ofs)?;
    let len = sid_len.checked_add(4)?;
    account_v.get(start..start.checked_add(len)?).map(|s| s.to_vec())
}

/// SID complet d'un compte = SID machine + RID (sections +1, 4 octets ajoutés → 28 o).
fn build_user_sid(machine_sid: &[u8], rid: u32) -> Option<Vec<u8>> {
    if machine_sid.len() < 8 { return None; }
    let mut s = machine_sid.to_vec();
    s[1] = s[1].saturating_add(1); // sous-autorités : 4 → 5
    s.extend_from_slice(&rid.to_le_bytes());
    Some(s)
}

/// (offset absolu dans C, longueur) du bloc des SIDs membres.
fn group_members_range(c: &[u8]) -> Option<(usize, usize)> {
    if c.len() < C_DATA_BASE { return None; }
    let mofs = u32_le(c, C_MEMBERS_OFS)? as usize;
    let mlen = u32_le(c, C_MEMBERS_LEN)? as usize;
    let start = C_DATA_BASE.checked_add(mofs)?;
    if start.checked_add(mlen)? > c.len() { return None; }
    Some((start, mlen))
}

/// Découpe le bloc membres en SIDs individuels (chaque SID : 8 + sections*4 octets).
fn split_sids(members: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 2 <= members.len() {
        let sections = members[i + 1] as usize;
        let len = 8 + sections * 4;
        if len == 0 || i + len > members.len() { break; }
        out.push(&members[i..i + len]);
        i += len;
    }
    out
}

fn group_contains_sid(c: &[u8], sid: &[u8]) -> bool {
    match group_members_range(c) {
        Some((start, len)) => split_sids(&c[start..start + len]).iter().any(|m| *m == sid),
        None => false,
    }
}

/// Reconstruit la valeur C avec un nouveau bloc membres (en-tête + grpname inchangés,
/// members_len et grp_members recalculés).
fn rebuild_group(c: &[u8], new_members: &[u8]) -> Option<Vec<u8>> {
    let (start, mlen) = group_members_range(c)?;
    let mut out = Vec::with_capacity(start + new_members.len() + c.len().saturating_sub(start + mlen));
    out.extend_from_slice(&c[..start]);
    out.extend_from_slice(new_members);
    out.extend_from_slice(&c[start + mlen..]);
    write_u32_le(&mut out, C_MEMBERS_LEN, new_members.len() as u32);
    write_u32_le(&mut out, C_GRP_MEMBERS, split_sids(new_members).len() as u32);
    Some(out)
}

fn add_sid_to_group(c: &[u8], sid: &[u8]) -> Option<Vec<u8>> {
    if group_contains_sid(c, sid) { return Some(c.to_vec()); }
    let (start, mlen) = group_members_range(c)?;
    let mut members = c[start..start + mlen].to_vec();
    members.extend_from_slice(sid);
    rebuild_group(c, &members)
}

fn remove_sid_from_group(c: &[u8], sid: &[u8]) -> Option<Vec<u8>> {
    let (start, mlen) = group_members_range(c)?;
    let mut members = Vec::with_capacity(mlen);
    for m in split_sids(&c[start..start + mlen]) {
        if m != sid { members.extend_from_slice(m); }
    }
    rebuild_group(c, &members)
}

fn hex_encode(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for x in b { s.push_str(&format!("{:02x}", x)); }
    s
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    let clean: Vec<char> = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if clean.len() % 2 != 0 { return None; }
    clean.chunks_exact(2)
        .map(|c| u8::from_str_radix(&format!("{}{}", c[0], c[1]), 16).ok())
        .collect()
}

/// Extrait les octets d'une valeur REG_BINARY depuis la sortie texte de `reg query`.
fn parse_reg_binary(stdout: &str) -> Option<Vec<u8>> {
    stdout.lines()
        .find(|l| l.contains("REG_BINARY"))
        .and_then(|l| l.split_whitespace().last())
        .and_then(hex_decode)
}

// ── Couche registre (Windows uniquement) ───────────────────────────────────────

#[cfg(target_os = "windows")]
use crate::maintenance::commands::execute_system_command;
#[cfg(target_os = "windows")]
use crate::system::winpe::RegHiveGuard;

#[cfg(target_os = "windows")]
fn reg_read_binary(key: &str, value: &str) -> Option<Vec<u8>> {
    let r = execute_system_command("reg", &["query", key, "/v", value], 20).ok()?;
    if !r.success { return None; }
    parse_reg_binary(&r.stdout)
}

#[cfg(target_os = "windows")]
fn reg_write_binary(key: &str, value: &str, data: &[u8]) -> Result<(), NiTriTeError> {
    let hex = hex_encode(data);
    let r = execute_system_command(
        "reg",
        &["add", key, "/v", value, "/t", "REG_BINARY", "/d", &hex, "/f"],
        20,
    )?;
    if r.success { Ok(()) } else {
        Err(NiTriTeError::System(format!("Écriture de la valeur {value} refusée : {}", r.stderr.trim())))
    }
}

/// Énumère les sous-clés (feuilles) d'une clé via `reg query`.
#[cfg(target_os = "windows")]
fn reg_subkeys(key: &str) -> Vec<String> {
    let prefix = format!("{}\\", key);
    execute_system_command("reg", &["query", key], 20)
        .map(|r| r.stdout)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.trim().strip_prefix("HKEY_LOCAL_MACHINE\\").map(|s| format!("HKLM\\{}", s)))
        .filter(|l| l.len() > prefix.len() && l.starts_with(&prefix) && !l[prefix.len()..].contains('\\'))
        .map(|l| l[prefix.len()..].to_string())
        .collect()
}

/// Garde fail-closed : refuse d'écrire la ruche du Windows en cours d'exécution.
/// Autorisé seulement si le volume ciblé ≠ volume système vivant (en WinPE le vivant
/// est X:, donc toute installation réelle passe ; hors WinPE le volume système est refusé).
#[cfg(target_os = "windows")]
fn garde_sam_ecriture(windows_dir: &str) -> Result<(), NiTriTeError> {
    let cible = windows_dir.chars().next().map(|c| c.to_ascii_uppercase());
    let vivant = std::env::var("SystemRoot").ok()
        .and_then(|r| r.chars().next().map(|c| c.to_ascii_uppercase()));
    match (cible, vivant) {
        (Some(c), Some(v)) if c == v => Err(NiTriTeError::CommandDenied(format!(
            "Écriture refusée : {windows_dir} est la ruche SAM du Windows en cours d'exécution. \
             Opération réservée à une installation HORS LIGNE (WinPE, double amorçage ou disque secondaire)."
        ))),
        (Some(_), _) => Ok(()),
        _ => Err(NiTriTeError::CommandDenied(
            "Écriture refusée : volume cible indéterminé (chemin windows_dir invalide).".into(),
        )),
    }
}

#[cfg(target_os = "windows")]
fn sam_path(windows_dir: &str) -> String {
    format!("{}\\System32\\config\\SAM", windows_dir.trim_end_matches('\\'))
}

/// Sauvegarde horodatée de la ruche SAM AVANT toute écriture. Échec = écriture refusée.
#[cfg(target_os = "windows")]
fn sauvegarder_sam(sam: &str) -> Result<String, NiTriTeError> {
    let dir = std::env::temp_dir().join("nitrite_sam_backup");
    std::fs::create_dir_all(&dir)
        .map_err(|e| NiTriTeError::System(format!("Dossier de sauvegarde impossible : {e}")))?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dst = dir.join(format!("SAM_{ts}"));
    std::fs::copy(sam, &dst)
        .map_err(|e| NiTriTeError::System(format!("Sauvegarde de la SAM échouée (écriture refusée) : {e}")))?;
    Ok(dst.to_string_lossy().into_owned())
}

/// garde → sauvegarde → `reg load`. Renvoie le guard (décharge auto) et le chemin de sauvegarde.
#[cfg(target_os = "windows")]
fn monter_ruche(windows_dir: &str) -> Result<(RegHiveGuard, String), NiTriTeError> {
    garde_sam_ecriture(windows_dir)?;
    let sam = sam_path(windows_dir);
    if !std::path::Path::new(&sam).exists() {
        return Err(NiTriTeError::System(format!("SAM introuvable : {sam}")));
    }
    let backup = sauvegarder_sam(&sam)?;
    let r = execute_system_command("reg", &["load", MOUNT, &sam], 30)?;
    if !r.success {
        return Err(NiTriTeError::System(format!(
            "Chargement de la ruche impossible (Windows en cours d'exécution ou droits administrateur manquants) : {}",
            r.stderr.trim()
        )));
    }
    Ok((RegHiveGuard { hive_key: MOUNT.to_string() }, backup))
}

#[cfg(target_os = "windows")]
fn users_key() -> String { format!(r"{MOUNT}\SAM\Domains\Account\Users") }
#[cfg(target_os = "windows")]
fn user_key(rid: u32) -> String { format!(r"{}\{:08X}", users_key(), rid) }
#[cfg(target_os = "windows")]
fn admins_c_key() -> String { format!(r"{MOUNT}\SAM\Domains\Builtin\Aliases\{GID_ADMINS}") }

#[cfg(target_os = "windows")]
fn machine_sid_monte() -> Option<Vec<u8>> {
    let v = reg_read_binary(&format!(r"{MOUNT}\SAM\Domains\Account"), "V")?;
    extract_machine_sid(&v)
}

/// RID de compte intégré (non supprimable) : Administrateur 500, Invité 501, DefaultAccount 503, WDAGUtility 504.
fn is_builtin_rid(rid: u32) -> bool { matches!(rid, 500 | 501 | 503 | 504) }

// ── Commandes Tauri ────────────────────────────────────────────────────────────

/// Liste les comptes avec statut RÉEL (bit ACB) : activé, verrouillé, admin, RID, MDP blanc.
/// Corrige le défaut de `list_offline_users` qui renvoyait toujours `enabled = true`.
#[tauri::command]
pub async fn sam_list_users(windows_dir: String) -> Result<Vec<SamUser>, NiTriTeError> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            // Lecture seule : garde fail-closed quand même (pas de sauvegarde ni d'écriture).
            garde_sam_ecriture(&windows_dir)?;
            let sam = sam_path(&windows_dir);
            if !std::path::Path::new(&sam).exists() {
                return Err(NiTriTeError::System(format!("SAM introuvable : {sam}")));
            }
            let r = execute_system_command("reg", &["load", MOUNT, &sam], 30)?;
            if !r.success {
                return Err(NiTriTeError::System(format!(
                    "Chargement de la ruche impossible (Windows en cours ou droits admin manquants) : {}",
                    r.stderr.trim()
                )));
            }
            let _guard = RegHiveGuard { hive_key: MOUNT.to_string() };

            let machine = machine_sid_monte();
            let mut users = Vec::new();
            for leaf in reg_subkeys(&users_key()) {
                if leaf.eq_ignore_ascii_case("Names") { continue; }
                if leaf.len() != 8 || !leaf.chars().all(|c| c.is_ascii_hexdigit()) { continue; }
                let Ok(rid) = u32::from_str_radix(&leaf, 16) else { continue };
                let key = user_key(rid);
                let f = reg_read_binary(&key, "F");
                let v = reg_read_binary(&key, "V");
                let acb = f.as_deref().and_then(read_acb).unwrap_or(0);
                let username = v.as_deref().map(extract_username).unwrap_or_default();
                let username = if username.is_empty() { format!("RID {rid}") } else { username };
                let is_admin = match (&machine, reg_read_binary(&admins_c_key(), "C")) {
                    (Some(m), Some(c)) => build_user_sid(m, rid).map(|s| group_contains_sid(&c, &s)).unwrap_or(false),
                    _ => false,
                };
                users.push(SamUser {
                    username,
                    rid,
                    enabled: acb & ACB_DISABLED == 0,
                    locked: acb & ACB_AUTOLOCK != 0,
                    is_admin,
                    password_blank: v.as_deref().map(|b| nt_len(b) == 0).unwrap_or(false),
                    is_builtin: is_builtin_rid(rid),
                });
            }
            users.sort_by_key(|u| u.rid);
            Ok(users)
        }
        #[cfg(not(target_os = "windows"))]
        { let _ = windows_dir; Ok(Vec::new()) }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

/// Applique une modification à la valeur F (ACB) d'un compte, avec sauvegarde + relecture.
#[cfg(target_os = "windows")]
fn modifier_f(windows_dir: &str, rid: u32, op: AcbOp, libelle: &str) -> Result<SamActionResult, NiTriTeError> {
    let (guard, backup) = monter_ruche(windows_dir)?;
    let key = user_key(rid);
    let mut f = reg_read_binary(&key, "F")
        .ok_or_else(|| NiTriTeError::System(format!("Valeur F introuvable pour le RID {rid}.")))?;
    if f.len() < F_MIN_LEN {
        return Err(NiTriTeError::System("Valeur F trop courte (structure SAM inattendue).".into()));
    }
    if !apply_acb(&mut f, op) {
        return Err(NiTriTeError::System("Modification des bits ACB impossible.".into()));
    }
    let attendu = read_acb(&f).unwrap_or(0);
    reg_write_binary(&key, "F", &f)?;
    let relu = reg_read_binary(&key, "F").and_then(|b| read_acb(&b));
    drop(guard);
    let ok = relu == Some(attendu);
    Ok(SamActionResult {
        success: ok,
        message: if ok { format!("{libelle} : OK (ACB = 0x{attendu:04X}).") }
                 else { format!("{libelle} : échec — relecture ACB = {relu:?}, attendu 0x{attendu:04X}.") },
        backup_path: backup,
    })
}

/// Blanchit (fait sauter) le mot de passe d'un compte.
#[tauri::command]
pub async fn sam_clear_password(windows_dir: String, rid: u32) -> Result<SamActionResult, NiTriTeError> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            let (guard, backup) = monter_ruche(&windows_dir)?;
            let key = user_key(rid);
            let mut v = reg_read_binary(&key, "V")
                .ok_or_else(|| NiTriTeError::System(format!("Valeur V introuvable pour le RID {rid}.")))?;
            if !blank_password(&mut v) {
                return Err(NiTriTeError::System("Valeur V trop courte (structure SAM inattendue).".into()));
            }
            reg_write_binary(&key, "V", &v)?;
            let relu = reg_read_binary(&key, "V");
            drop(guard);
            let ok = relu.as_deref().map(|b| nt_len(b) == 0 && u32_le(b, V_LMPW_LEN) == Some(0)).unwrap_or(false);
            Ok(SamActionResult {
                success: ok,
                message: if ok { "Mot de passe blanchi : le compte se connecte désormais sans mot de passe.".into() }
                         else { "Échec : la relecture ne confirme pas un hash NT/LM de longueur nulle.".into() },
                backup_path: backup,
            })
        }
        #[cfg(not(target_os = "windows"))]
        { let _ = (windows_dir, rid); Err(NiTriTeError::System("Disponible sous Windows uniquement.".into())) }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

/// Active ou désactive un compte (bit ACB DISABLE).
#[tauri::command]
pub async fn sam_set_enabled(windows_dir: String, rid: u32, enabled: bool) -> Result<SamActionResult, NiTriTeError> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            let op = if enabled { AcbOp::Enable } else { AcbOp::Disable };
            let libelle = if enabled { "Activation du compte" } else { "Désactivation du compte" };
            modifier_f(&windows_dir, rid, op, libelle)
        }
        #[cfg(not(target_os = "windows"))]
        { let _ = (windows_dir, rid, enabled); Err(NiTriTeError::System("Disponible sous Windows uniquement.".into())) }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

/// Déverrouille un compte (efface les bits DISABLE et AUTOLOCK).
#[tauri::command]
pub async fn sam_unlock(windows_dir: String, rid: u32) -> Result<SamActionResult, NiTriTeError> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        { modifier_f(&windows_dir, rid, AcbOp::Unlock, "Déverrouillage du compte") }
        #[cfg(not(target_os = "windows"))]
        { let _ = (windows_dir, rid); Err(NiTriTeError::System("Disponible sous Windows uniquement.".into())) }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

/// Promeut (admin=true) ou rétrograde (admin=false) un compte dans le groupe Administrateurs.
#[tauri::command]
pub async fn sam_set_admin(windows_dir: String, rid: u32, admin: bool) -> Result<SamActionResult, NiTriTeError> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            let (guard, backup) = monter_ruche(&windows_dir)?;
            let machine = machine_sid_monte()
                .ok_or_else(|| NiTriTeError::System("SID machine illisible (valeur Account\\V).".into()))?;
            let sid = build_user_sid(&machine, rid)
                .ok_or_else(|| NiTriTeError::System("Construction du SID du compte impossible.".into()))?;
            let ckey = admins_c_key();
            let c = reg_read_binary(&ckey, "C")
                .ok_or_else(|| NiTriTeError::System("Valeur C du groupe Administrateurs introuvable.".into()))?;
            let new_c = if admin {
                add_sid_to_group(&c, &sid)
            } else {
                remove_sid_from_group(&c, &sid)
            }.ok_or_else(|| NiTriTeError::System("Structure C du groupe inattendue (édition refusée).".into()))?;
            if new_c != c {
                reg_write_binary(&ckey, "C", &new_c)?;
            }
            let relu = reg_read_binary(&ckey, "C");
            drop(guard);
            let ok = relu.as_deref().map(|b| group_contains_sid(b, &sid) == admin).unwrap_or(false);
            Ok(SamActionResult {
                success: ok,
                message: if ok {
                    if admin { "Compte promu administrateur.".into() } else { "Compte rétrogradé (retiré des administrateurs).".into() }
                } else { "Échec : l'appartenance au groupe Administrateurs ne correspond pas après relecture.".into() },
                backup_path: backup,
            })
        }
        #[cfg(not(target_os = "windows"))]
        { let _ = (windows_dir, rid, admin); Err(NiTriTeError::System("Disponible sous Windows uniquement.".into())) }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

/// Supprime un compte : clé RID, entrée Names, et SID retiré de tous les groupes Builtin.
#[tauri::command]
pub async fn sam_delete_user(windows_dir: String, rid: u32) -> Result<SamActionResult, NiTriTeError> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        {
            if is_builtin_rid(rid) {
                return Err(NiTriTeError::CommandDenied(format!(
                    "Suppression refusée : le RID {rid} est un compte intégré Windows (Administrateur/Invité/DefaultAccount)."
                )));
            }
            let (guard, backup) = monter_ruche(&windows_dir)?;
            let key = user_key(rid);
            let v = reg_read_binary(&key, "V")
                .ok_or_else(|| NiTriTeError::System(format!("Compte RID {rid} introuvable.")))?;
            let name = extract_username(&v);

            // Retire le SID de tous les groupes Builtin (évite un SID fantôme « Compte inconnu »).
            if let Some(machine) = machine_sid_monte() {
                if let Some(sid) = build_user_sid(&machine, rid) {
                    let aliases = format!(r"{MOUNT}\SAM\Domains\Builtin\Aliases");
                    for gid in reg_subkeys(&aliases) {
                        if gid.eq_ignore_ascii_case("Members") { continue; }
                        let ck = format!(r"{aliases}\{gid}");
                        if let Some(c) = reg_read_binary(&ck, "C") {
                            if group_contains_sid(&c, &sid) {
                                if let Some(nc) = remove_sid_from_group(&c, &sid) {
                                    let _ = reg_write_binary(&ck, "C", &nc);
                                }
                            }
                        }
                    }
                }
            }

            // Entrée Names\<nom> puis clé RID.
            if !name.is_empty() {
                let _ = execute_system_command(
                    "reg", &["delete", &format!(r"{}\Names\{}", users_key(), name), "/f"], 20);
            }
            let del = execute_system_command("reg", &["delete", &key, "/f"], 20)?;
            let gone = reg_read_binary(&key, "F").is_none();
            drop(guard);
            let ok = del.success && gone;
            Ok(SamActionResult {
                success: ok,
                message: if ok { format!("Compte « {name} » (RID {rid}) supprimé.") }
                         else { format!("Échec de suppression (RID {rid}) : {}", del.stderr.trim()) },
                backup_path: backup,
            })
        }
        #[cfg(not(target_os = "windows"))]
        { let _ = (windows_dir, rid); Err(NiTriTeError::System("Disponible sous Windows uniquement.".into())) }
    })
    .await
    .map_err(|e| NiTriTeError::System(e.to_string()))?
}

/// Création de compte : NON DISPONIBLE par l'approche `reg`. Erreur explicite + raison.
#[tauri::command]
pub async fn sam_create_user(windows_dir: String, username: String) -> Result<SamActionResult, NiTriTeError> {
    let _ = (windows_dir, username);
    Err(NiTriTeError::System(
        "Création de compte non disponible hors ligne. Blocage structurel : l'entrée \
         SAM\\Domains\\Account\\Users\\Names\\<nom> exige une valeur par défaut dont le TYPE (champ de \
         type du registre) vaut le RID du compte — ce que `reg add /t` ne peut pas écrire (il n'accepte \
         que REG_SZ/REG_BINARY/…, pas un numéro de type arbitraire). Il faudrait aussi allouer un RID, \
         construire des structures V/F complètes et incrémenter le RID max dans Account\\F — non prouvable \
         sans un parseur de ruche maison. NTPWEdit et chntpw ne créent pas de compte non plus. \
         Utilisez un compte existant (blanchir le mot de passe, activer, promouvoir admin).".into(),
    ))
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Construit une valeur F minimale (0x50 o) avec un ACB donné.
    fn make_f(acb: u16) -> Vec<u8> {
        let mut f = vec![0u8; 0x50];
        f[F_ACB_OFS] = acb as u8;
        f[F_ACB_OFS + 1] = (acb >> 8) as u8;
        f
    }

    #[test]
    fn acb_desactive_pose_disable() {
        let mut f = make_f(0x0010); // NORMAL
        assert!(apply_acb(&mut f, AcbOp::Disable));
        assert_eq!(read_acb(&f), Some(0x0011));
    }

    #[test]
    fn acb_active_efface_disable() {
        let mut f = make_f(0x0011);
        assert!(apply_acb(&mut f, AcbOp::Enable));
        assert_eq!(read_acb(&f), Some(0x0010));
    }

    #[test]
    fn acb_deverrouille_efface_disable_et_autolock() {
        let mut f = make_f(0x0411); // NORMAL(0x10)? non: 0x0001|0x0400|0x0010 = 0x0411
        assert!(apply_acb(&mut f, AcbOp::Unlock));
        // Reste 0x0010 (NORMAL), DISABLE+AUTOLOCK effacés.
        assert_eq!(read_acb(&f), Some(0x0010));
    }

    #[test]
    fn acb_f_trop_courte_echoue() {
        let mut f = vec![0u8; 0x20];
        assert!(!apply_acb(&mut f, AcbOp::Enable));
    }

    // Construit une valeur V avec un nom d'utilisateur et des longueurs de hash données.
    fn make_v(name: &str, lm_len: u32, nt_len_: u32) -> Vec<u8> {
        let name_utf16: Vec<u8> = name.encode_utf16().flat_map(|c| c.to_le_bytes()).collect();
        let mut v = vec![0u8; V_DATA_BASE + name_utf16.len()];
        write_u32_le(&mut v, V_USERNAME_OFS, 0); // nom à 0xcc + 0
        write_u32_le(&mut v, V_USERNAME_LEN, name_utf16.len() as u32);
        write_u32_le(&mut v, V_LMPW_LEN, lm_len);
        write_u32_le(&mut v, V_NTPW_LEN, nt_len_);
        v[V_DATA_BASE..].copy_from_slice(&name_utf16);
        v
    }

    #[test]
    fn blanchir_met_longueurs_hash_a_zero() {
        let mut v = make_v("Momo", 0x14, 0x14);
        assert_eq!(nt_len(&v), 0x14);
        assert!(blank_password(&mut v));
        assert_eq!(nt_len(&v), 0);
        assert_eq!(u32_le(&v, V_LMPW_LEN), Some(0));
    }

    #[test]
    fn extraction_nom_utilisateur() {
        let v = make_v("Administrateur", 0, 0x14);
        assert_eq!(extract_username(&v), "Administrateur");
    }

    #[test]
    fn construction_sid_compte() {
        // SID machine factice : rev=1, sections=4, autorité NT, 21-X-Y-Z.
        let mut machine = vec![1u8, 4, 0, 0, 0, 0, 0, 5];
        for sub in [21u32, 0x1111, 0x2222, 0x3333] { machine.extend_from_slice(&sub.to_le_bytes()); }
        assert_eq!(machine.len(), 24);
        let sid = build_user_sid(&machine, 1000).unwrap();
        assert_eq!(sid.len(), 28);
        assert_eq!(sid[1], 5); // sections 4 → 5
        assert_eq!(u32_le(&sid, 24), Some(1000)); // RID en queue
    }

    // Construit une valeur C de groupe avec une liste de SIDs membres.
    fn make_c(members: &[&[u8]]) -> Vec<u8> {
        let mut block = Vec::new();
        for m in members { block.extend_from_slice(m); }
        let mut c = vec![0u8; C_DATA_BASE];
        write_u32_le(&mut c, C_MEMBERS_OFS, 0); // membres à 0x34 + 0
        write_u32_le(&mut c, C_MEMBERS_LEN, block.len() as u32);
        write_u32_le(&mut c, C_GRP_MEMBERS, members.len() as u32);
        c.extend_from_slice(&block);
        c
    }

    fn sid(rid: u32) -> Vec<u8> {
        let mut machine = vec![1u8, 4, 0, 0, 0, 0, 0, 5];
        for sub in [21u32, 1, 2, 3] { machine.extend_from_slice(&sub.to_le_bytes()); }
        build_user_sid(&machine, rid).unwrap()
    }

    #[test]
    fn groupe_ajout_retrait_et_appartenance() {
        let admin500 = sid(500);
        let user1000 = sid(1000);
        let c = make_c(&[&admin500]);
        assert!(group_contains_sid(&c, &admin500));
        assert!(!group_contains_sid(&c, &user1000));

        // Ajout
        let c2 = add_sid_to_group(&c, &user1000).unwrap();
        assert!(group_contains_sid(&c2, &user1000));
        assert!(group_contains_sid(&c2, &admin500));
        assert_eq!(u32_le(&c2, C_GRP_MEMBERS), Some(2));
        assert_eq!(u32_le(&c2, C_MEMBERS_LEN), Some(56)); // 2 × 28

        // Ajout idempotent
        let c2b = add_sid_to_group(&c2, &user1000).unwrap();
        assert_eq!(u32_le(&c2b, C_GRP_MEMBERS), Some(2));

        // Retrait
        let c3 = remove_sid_from_group(&c2, &user1000).unwrap();
        assert!(!group_contains_sid(&c3, &user1000));
        assert!(group_contains_sid(&c3, &admin500));
        assert_eq!(u32_le(&c3, C_GRP_MEMBERS), Some(1));
        assert_eq!(u32_le(&c3, C_MEMBERS_LEN), Some(28));
    }

    #[test]
    fn hex_aller_retour() {
        let data = vec![0x00u8, 0xbc, 0x0a, 0xff, 0x10, 0x55];
        let h = hex_encode(&data);
        assert_eq!(h, "00bc0aff1055");
        assert_eq!(hex_decode(&h), Some(data));
    }

    #[test]
    fn parse_sortie_reg_binary() {
        let out = "\r\nHKEY_LOCAL_MACHINE\\NITRITE_SAM\\...\\000001F4\r\n    F    REG_BINARY    0200010000bc0a00\r\n\r\n";
        assert_eq!(parse_reg_binary(out), Some(vec![0x02, 0x00, 0x01, 0x00, 0x00, 0xbc, 0x0a, 0x00]));
    }

    #[test]
    fn parse_reg_binary_absent() {
        assert_eq!(parse_reg_binary("Valeur introuvable"), None);
    }

    // Round-trip REG_BINARY réel via HKCU (pas de droits admin requis).
    // Ignoré par défaut : touche le registre (lancer avec `-- --ignored`).
    #[cfg(target_os = "windows")]
    #[test]
    #[ignore]
    fn reg_binary_round_trip_hkcu() {
        let key = r"HKCU\Software\NITRITE_SAM_TEST";
        let data = vec![0x02u8, 0x00, 0x01, 0x00, 0xde, 0xad, 0xbe, 0xef, 0x00, 0xff];
        reg_write_binary(key, "Probe", &data).expect("écriture HKCU");
        let relu = reg_read_binary(key, "Probe").expect("relecture HKCU");
        assert_eq!(relu, data);
        let _ = execute_system_command("reg", &["delete", key, "/f"], 20);
    }
}
