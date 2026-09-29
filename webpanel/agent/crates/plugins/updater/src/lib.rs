//! Cale de `tauri-plugin-updater` : l'agent se met a jour par sa propre
//! release (voir README), pas par le plugin de l'application native.
pub struct Plugin;

#[derive(Default)]
pub struct Builder;

impl Builder {
    pub fn new() -> Self {
        Builder
    }

    pub fn build(self) -> Plugin {
        Plugin
    }
}
