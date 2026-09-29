//! Correspondance nom du catalogue <-> nom affiche dans le registre Uninstall.
//!
//! L'ancienne comparaison (`verify_installed`) acceptait le simple premier mot
//! du nom : « Microsoft Teams » etait « verifie installe » des qu'une app
//! Microsoft quelconque etait presente, « Google Chrome » des que Google Drive
//! l'etait. Resultat : une installation ratee passait pour reussie, et une
//! desinstallation reussie pour ratee (l'app « restait » presente).
//!
//! Ici on compare des suites de mots entiers, sans les jetons de version ni
//! d'architecture que le registre ajoute (« 7-Zip 26.02 (x64) »).

/// Jetons ajoutes par les installeurs qui ne font pas partie du nom.
const NOISE: &[&str] = &["x64", "x86", "amd64", "arm64", "64", "32", "bit", "win64", "win32"];

/// Mots trop generiques pour identifier une app a eux seuls (editeurs,
/// termes courants). Un nom de catalogue d'un seul de ces mots ne matche
/// qu'en tete du nom installe, jamais au milieu.
const GENERIC: &[&str] = &[
    "microsoft", "google", "adobe", "mozilla", "apple", "update", "tools", "tool",
    "driver", "drivers", "runtime", "service", "client", "desktop", "app", "free",
    "the", "for", "and", "pro", "studio", "player", "office", "security",
];

pub fn tokens(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty() && !NOISE.contains(t))
        .map(str::to_string)
        .collect()
}

fn is_version_like(t: &str) -> bool {
    t.chars().all(|c| c.is_ascii_digit())
}

/// `true` si `installed` (nom du registre) designe l'app `catalog`.
pub fn names_match(catalog: &str, installed: &str) -> bool {
    let c = tokens(catalog);
    let r = tokens(installed);
    if c.is_empty() || r.is_empty() {
        return false;
    }
    // 1. Le nom installe commence par le nom du catalogue (cas le plus courant :
    //    « VLC » / « VLC media player », « Python 3.12 » / « Python 3.12.4 »).
    if r.len() >= c.len() && r[..c.len()] == c[..] {
        return true;
    }
    // 2. Meme nom a l'espacement pres (« Libre Office » / « LibreOffice »).
    let joined_c: String = c.concat();
    let mut acc = String::new();
    for t in &r {
        acc.push_str(t);
        if acc == joined_c {
            return true;
        }
        if acc.len() >= joined_c.len() {
            break;
        }
    }
    // 3. Nom du catalogue present au milieu (« Firefox » / « Mozilla Firefox »),
    //    seulement s'il est assez distinctif.
    let distinctive = c.len() >= 2
        || (c[0].len() >= 4 && !GENERIC.contains(&c[0].as_str()) && !is_version_like(&c[0]));
    if distinctive && r.len() > c.len() {
        return r.windows(c.len()).any(|w| w == c.as_slice());
    }
    false
}

/// `true` si l'un des noms installes designe l'app `catalog`.
pub fn any_installed(catalog: &str, installed_names: &[String]) -> bool {
    installed_names.iter().any(|n| names_match(catalog, n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_versioned_registry_names() {
        assert!(names_match("7-Zip", "7-Zip 26.02 (x64)"));
        assert!(names_match("VLC", "VLC media player"));
        assert!(names_match("Python 3.12", "Python 3.12.4 (64-bit)"));
        assert!(names_match("Notepad++", "Notepad++ (64-bit x64)"));
        assert!(names_match("Mozilla Firefox", "Mozilla Firefox (x64 fr)"));
    }

    #[test]
    fn matches_inner_distinctive_names() {
        assert!(names_match("Firefox", "Mozilla Firefox (x64 fr)"));
        assert!(names_match("Visual Studio Code", "Microsoft Visual Studio Code (User)"));
        assert!(names_match("CPU-Z", "CPUID CPU-Z 2.10"));
        assert!(names_match("Libre Office", "LibreOffice 24.8.1.2"));
    }

    #[test]
    fn rejects_same_vendor_other_product() {
        assert!(!names_match("Microsoft Teams", "Microsoft Edge"));
        assert!(!names_match("Google Chrome", "Google Drive"));
        assert!(!names_match("Adobe Acrobat Reader", "Adobe Photoshop 2024"));
        assert!(!names_match("Git", "GitHub Desktop"));
        assert!(!names_match("Office", "LibreOffice Help Pack"));
    }

    #[test]
    fn rejects_generic_single_word_in_the_middle() {
        assert!(!names_match("Update", "Microsoft Update Health Tools"));
        assert!(!names_match("Studio", "Microsoft Visual Studio Code"));
    }

    #[test]
    fn empty_names_never_match() {
        assert!(!names_match("", "Anything"));
        assert!(!names_match("App", ""));
        assert!(!names_match("(x64)", "7-Zip (x64)"));
    }
}
