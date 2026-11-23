use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::paths;

fn default_true() -> bool { true }
fn default_10() -> u64 { 10 }
fn default_normal() -> String { "normal".to_string() }
fn default_json() -> String { "json".to_string() }
fn default_temperature() -> f64 { 0.7 }
fn default_ollama_url() -> String { "http://localhost:11434".to_string() }
fn default_ollama_model() -> String { "llama3:8b".to_string() }
fn default_ai_backend() -> String { "llamacpp".to_string() }
fn default_llamacpp_port() -> u16 { 8080 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub theme: String,
    pub language: String,
    pub sidebar_collapsed: bool,
    // Ollama (installation requise)
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,
    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,
    #[serde(default = "default_temperature")]
    pub ollama_temperature: f64,
    // Backend IA actif: "ollama" | "llamacpp"
    #[serde(default = "default_ai_backend")]
    pub ai_backend: String,
    // llama.cpp portable (sans installation)
    #[serde(default)]
    pub llamacpp_server_path: String,
    #[serde(default)]
    pub llamacpp_model_path: String,
    #[serde(default = "default_llamacpp_port")]
    pub llamacpp_port: u16,
    pub monitor_interval_ms: u64,
    #[serde(default = "default_true")]
    pub show_animations: bool,
    #[serde(default)]
    pub compact_mode: bool,
    #[serde(default = "default_true")]
    pub notifications_enabled: bool,
    #[serde(default = "default_10")]
    pub process_count: u64,
    #[serde(default = "default_normal")]
    pub font_size: String,
    #[serde(default = "default_json")]
    pub export_format: String,
    /// Reglages purement interface (onglets Performance / Notifications de la
    /// page Parametres : `notif_*`, `auto_save`, `gpu_acceleration`...). Sans ce
    /// champ, serde les jetait en silence et la page annoncait « sauvegardes »
    /// des valeurs revenues a leur defaut au chargement suivant.
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "nitrite-dark".to_string(),
            language: "fr".to_string(),
            sidebar_collapsed: false,
            ollama_url: "http://localhost:11434".to_string(),
            ollama_model: "llama3:8b".to_string(),
            ollama_temperature: 0.7,
            ai_backend: "llamacpp".to_string(),
            llamacpp_server_path: String::new(),
            llamacpp_model_path: String::new(),
            llamacpp_port: 8080,
            monitor_interval_ms: 2000,
            show_animations: true,
            compact_mode: false,
            notifications_enabled: true,
            process_count: 10,
            font_size: "normal".to_string(),
            export_format: "json".to_string(),
            extra: serde_json::Map::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values_are_correct() {
        assert!(default_true());
        assert_eq!(default_10(), 10);
        assert_eq!(default_normal(), "normal");
        assert_eq!(default_json(), "json");
        assert!((default_temperature() - 0.7).abs() < f64::EPSILON);
        assert_eq!(default_ollama_url(), "http://localhost:11434");
        assert_eq!(default_ollama_model(), "llama3:8b");
        assert_eq!(default_ai_backend(), "llamacpp");
        assert_eq!(default_llamacpp_port(), 8080);
    }

    #[test]
    fn app_config_default_matches_expected() {
        let c = AppConfig::default();
        assert_eq!(c.theme, "nitrite-dark");
        assert_eq!(c.language, "fr");
        assert!(!c.sidebar_collapsed);
        assert_eq!(c.ollama_url, "http://localhost:11434");
        assert_eq!(c.ollama_model, "llama3:8b");
        assert_eq!(c.ai_backend, "llamacpp");
        assert_eq!(c.llamacpp_port, 8080);
        assert!(c.show_animations);
        assert!(!c.compact_mode);
        assert!(c.notifications_enabled);
        assert_eq!(c.process_count, 10);
        assert_eq!(c.font_size, "normal");
        assert_eq!(c.export_format, "json");
    }

    #[test]
    fn app_config_serializes_and_deserializes() {
        let original = AppConfig::default();
        let json = serde_json::to_string(&original).expect("serialization failed");
        let restored: AppConfig = serde_json::from_str(&json).expect("deserialization failed");
        assert_eq!(restored.theme, original.theme);
        assert_eq!(restored.llamacpp_port, original.llamacpp_port);
        assert_eq!(restored.process_count, original.process_count);
    }

    #[test]
    fn ui_only_settings_survive_a_round_trip() {
        let json = r#"{"theme":"t","language":"fr","sidebar_collapsed":false,"monitor_interval_ms":2000,"notif_sounds":true,"dashboard_refresh_ms":5000}"#;
        let c: AppConfig = serde_json::from_str(json).expect("parse");
        let back: serde_json::Value = serde_json::to_value(&c).expect("ser");
        assert_eq!(back["notif_sounds"], true);
        assert_eq!(back["dashboard_refresh_ms"], 5000);
        assert_eq!(back["llamacpp_port"], 8080);
    }

    #[test]
    fn app_config_partial_json_uses_serde_defaults() {
        // If fields are missing from JSON, serde defaults kick in
        let json = r#"{"theme":"light","language":"en","sidebar_collapsed":false,"monitor_interval_ms":1000}"#;
        let c: AppConfig = serde_json::from_str(json).expect("partial json failed");
        assert_eq!(c.theme, "light");
        // serde defaults applied
        assert_eq!(c.llamacpp_port, 8080);
        assert_eq!(c.process_count, 10);
        assert!(c.show_animations);
        assert!(c.notifications_enabled);
    }
}

impl AppConfig {
    fn config_file() -> PathBuf {
        paths::config_dir().join("nitrite_config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_file();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => Self::default(),
            }
        } else {
            let config = Self::default();
            let _ = config.save();
            config
        }
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::config_file();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)
    }
}
