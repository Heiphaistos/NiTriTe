//! Garde-fou de `disk_wipe` (winpe.rs) : quels disques physiques ne jamais effacer.

/// Numeros des disques physiques qui portent le volume `lettre` (IOCTL
/// VOLUME_GET_VOLUME_DISK_EXTENTS : pas de wmic, pas de PowerShell, marche en
/// WinPE et quelle que soit la langue). `None` si le volume n'est pas lisible.
#[cfg(target_os = "windows")]
pub fn disques_du_volume(lettre: char) -> Option<Vec<u32>> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
        IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS, OPEN_EXISTING,
    };
    use windows::Win32::System::Ioctl::VOLUME_DISK_EXTENTS;
    use windows::Win32::System::IO::DeviceIoControl;

    let chemin: Vec<u16> = format!("\\\\.\\{}:", lettre).encode_utf16().chain(Some(0)).collect();
    // u64 : alignement de VOLUME_DISK_EXTENTS (champs i64) ; 32 extents max.
    let mut buf = [0u64; 64];
    let mut lus = 0u32;
    // SAFETY : chemin termine par 0 ; buf vit pendant l'appel et sa taille est
    // passee ; la poignee est fermee juste apres.
    unsafe {
        let h = CreateFileW(PCWSTR(chemin.as_ptr()), 0, FILE_SHARE_READ | FILE_SHARE_WRITE,
            None, OPEN_EXISTING, FILE_FLAGS_AND_ATTRIBUTES(0), None).ok()?;
        let r = DeviceIoControl(h, IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS, None, 0,
            Some(buf.as_mut_ptr() as *mut _), std::mem::size_of_val(&buf) as u32, Some(&mut lus), None);
        let _ = CloseHandle(h);
        r.ok()?;
        let ext = &*(buf.as_ptr() as *const VOLUME_DISK_EXTENTS);
        let n = (ext.NumberOfDiskExtents as usize).min(32);
        let premier = ext.Extents.as_ptr();
        Some((0..n).map(|i| (*premier.add(i)).DiskNumber).collect())
    }
}

/// Disques a ne jamais effacer : celui du Windows en cours d'execution (hors
/// WinPE) et celui d'ou NiTriTe a ete lance (la cle USB en WinPE). Fail-closed :
/// si le disque du Windows en cours ne peut pas etre determine, refus.
#[cfg(target_os = "windows")]
pub fn disques_proteges(en_winpe: bool) -> Result<Vec<u32>, String> {
    let lettre_de = |s: &str| s.chars().next()
        .filter(|c| c.is_ascii_alphabetic() && s.get(1..).is_some_and(|r| r.starts_with(':')));
    let mut proteges = Vec::new();
    if !en_winpe {
        let sd = std::env::var("SYSTEMDRIVE").unwrap_or_else(|_| "C:".into());
        let l = lettre_de(&sd).ok_or("lecteur systeme illisible")?;
        proteges.extend(disques_du_volume(l).ok_or("disque du Windows en cours introuvable")?);
    }
    if let Some(l) = std::env::current_exe().ok().and_then(|e| lettre_de(&e.to_string_lossy())) {
        proteges.extend(disques_du_volume(l).unwrap_or_default());
    }
    Ok(proteges)
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    // Lecture seule (IOCTL de requete) : le volume systeme de ce PC est porte
    // par au moins un disque, et il est protege hors WinPE.
    #[test]
    fn le_disque_systeme_est_trouve_et_protege() {
        let sd = std::env::var("SYSTEMDRIVE").unwrap_or_else(|_| "C:".into());
        let l = sd.chars().next().unwrap_or('C');
        let disques = disques_du_volume(l).expect("IOCTL sur le volume systeme");
        assert!(!disques.is_empty());
        let proteges = disques_proteges(false).expect("garde");
        assert!(disques.iter().all(|d| proteges.contains(d)));
        assert!(disques_du_volume('?').is_none());
    }
}
