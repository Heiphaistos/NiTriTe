use thiserror::Error;

#[derive(Error, Debug)]
pub enum NiTriTeError {
    #[error("Erreur systeme: {0}")]
    System(String),

    #[error("Erreur WMI: {0}")]
    Wmi(String),

    #[error("Erreur reseau: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Erreur IO: {0}")]
    Io(#[from] std::io::Error),

    #[error("Erreur JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Erreur registre: {0}")]
    Registry(String),

    #[error("Commande refusee: {0}")]
    CommandDenied(String),

    #[error("Ollama non disponible: {0}")]
    OllamaUnavailable(String),

    #[error("Droits admin requis: {0}")]
    ElevationRequired(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Tauri: {0}")]
    Tauri(String),
}

#[cfg(windows)]
impl From<wmi::WMIError> for NiTriTeError {
    fn from(e: wmi::WMIError) -> Self {
        NiTriTeError::Wmi(e.to_string())
    }
}

impl From<tauri::Error> for NiTriTeError {
    fn from(e: tauri::Error) -> Self {
        NiTriTeError::Tauri(e.to_string())
    }
}

impl serde::Serialize for NiTriTeError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
