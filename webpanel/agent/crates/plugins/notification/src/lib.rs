//! Cale de `tauri-plugin-notification` : le plugin natif n'a pas d'objet dans l'agent
//! (le navigateur et les points d'entree /api/host/* le remplacent).
pub struct Plugin;

pub fn init() -> Plugin {
    Plugin
}
