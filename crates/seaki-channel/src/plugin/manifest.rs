use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    pub runtime: PluginRuntime,
    #[serde(default)]
    pub entry: String,
    #[serde(default)]
    pub capabilities: PluginCapabilities,
    #[serde(default)]
    pub permissions: PluginPermissions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginRuntime {
    Wasm,
    Native,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PluginCapabilities {
    #[serde(default)]
    pub receive_message: bool,
    #[serde(default)]
    pub send_message: bool,
    #[serde(default)]
    pub send_file: bool,
    #[serde(default)]
    pub thread_reply: bool,
    #[serde(default)]
    pub drive_comment: bool,
    #[serde(default)]
    pub interactive_card: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PluginPermissions {
    #[serde(default)]
    pub network: Vec<String>,
    #[serde(default)]
    pub local_files: Vec<String>,
    #[serde(default)]
    pub brokered_secret_scopes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    ParseFailed(String),
    MissingField(String),
    InvalidRuntime(String),
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestError::ParseFailed(s) => write!(f, "manifest parse failed: {s}"),
            ManifestError::MissingField(s) => write!(f, "missing required field: {s}"),
            ManifestError::InvalidRuntime(s) => write!(f, "invalid runtime: {s}"),
        }
    }
}

impl std::error::Error for ManifestError {}

pub fn parse_manifest(content: &str) -> Result<PluginManifest, ManifestError> {
    let manifest: PluginManifest =
        toml::from_str(content).map_err(|e| ManifestError::ParseFailed(e.to_string()))?;
    if manifest.id.is_empty() {
        return Err(ManifestError::MissingField("id".to_string()));
    }
    if manifest.name.is_empty() {
        return Err(ManifestError::MissingField("name".to_string()));
    }
    if manifest.version.is_empty() {
        return Err(ManifestError::MissingField("version".to_string()));
    }
    if manifest.entry.is_empty() {
        return Err(ManifestError::MissingField("entry".to_string()));
    }
    Ok(manifest)
}
