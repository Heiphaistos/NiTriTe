//! Manifeste Windows : l'agent demande les droits administrateur au
//! lancement, comme NiTriTe (DISM, SFC, installations, pilotes…).
fn main() {
    if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
        use embed_manifest::manifest::ExecutionLevel;
        use embed_manifest::{embed_manifest, new_manifest};
        embed_manifest(new_manifest("NiTriTe.Agent").requested_execution_level(ExecutionLevel::RequireAdministrator))
            .expect("manifeste Windows");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
