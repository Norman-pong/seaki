use std::collections::HashMap;
use std::path::Path;

use crate::plugin::manifest::{parse_manifest, ManifestError, PluginManifest, PluginRuntime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredPlugin {
    pub manifest: PluginManifest,
    pub plugin_dir: String,
    pub wasm_path: Option<String>,
    pub state: PluginState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    Discovered,
    Validated,
    Loaded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    Io(String),
    Manifest(ManifestError),
    WasmNotFound { plugin_id: String, path: String },
    DuplicateId(String),
    InvalidDirectory(String),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::Io(s) => write!(f, "io error: {s}"),
            RegistryError::Manifest(e) => write!(f, "manifest error: {e}"),
            RegistryError::WasmNotFound { plugin_id, path } => {
                write!(f, "wasm not found for plugin {plugin_id} at {path}")
            }
            RegistryError::DuplicateId(s) => write!(f, "duplicate plugin id: {s}"),
            RegistryError::InvalidDirectory(s) => write!(f, "invalid directory: {s}"),
        }
    }
}

impl std::error::Error for RegistryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RegistryError::Manifest(e) => Some(e),
            _ => None,
        }
    }
}

pub struct PluginRegistry {
    plugins: HashMap<String, RegisteredPlugin>,
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    pub fn scan_dir(&mut self, path: &Path) -> Result<Vec<String>, RegistryError> {
        if !path.is_dir() {
            return Err(RegistryError::InvalidDirectory(path.display().to_string()));
        }

        let mut registered = Vec::new();
        for entry in std::fs::read_dir(path).map_err(|e| RegistryError::Io(e.to_string()))? {
            let entry = entry.map_err(|e| RegistryError::Io(e.to_string()))?;
            let plugin_dir = entry.path();
            if !plugin_dir.is_dir() {
                continue;
            }

            let manifest_path = plugin_dir.join("plugin.toml");
            if !manifest_path.exists() {
                continue;
            }

            let registered_plugin = self.load(&plugin_dir)?;
            registered.push(registered_plugin.manifest.id.clone());
        }

        Ok(registered)
    }

    pub fn load(&mut self, plugin_dir: &Path) -> Result<RegisteredPlugin, RegistryError> {
        if !plugin_dir.is_dir() {
            return Err(RegistryError::InvalidDirectory(
                plugin_dir.display().to_string(),
            ));
        }

        let manifest_path = plugin_dir.join("plugin.toml");
        let content = std::fs::read_to_string(&manifest_path)
            .map_err(|e| RegistryError::Io(e.to_string()))?;
        let manifest = parse_manifest(&content).map_err(RegistryError::Manifest)?;

        if self.plugins.contains_key(&manifest.id) {
            return Err(RegistryError::DuplicateId(manifest.id.clone()));
        }

        let wasm_path = if manifest.runtime == PluginRuntime::Wasm {
            let entry_path = plugin_dir.join(&manifest.entry);
            if !entry_path.exists() {
                return Err(RegistryError::WasmNotFound {
                    plugin_id: manifest.id.clone(),
                    path: entry_path.display().to_string(),
                });
            }
            Some(entry_path.display().to_string())
        } else {
            None
        };

        let registered = RegisteredPlugin {
            manifest: manifest.clone(),
            plugin_dir: plugin_dir.display().to_string(),
            wasm_path,
            state: PluginState::Discovered,
        };

        self.plugins.insert(manifest.id.clone(), registered.clone());
        Ok(registered)
    }

    pub fn get(&self, plugin_id: &str) -> Option<&RegisteredPlugin> {
        self.plugins.get(plugin_id)
    }

    pub fn list(&self) -> Vec<&RegisteredPlugin> {
        self.plugins.values().collect()
    }

    pub fn list_by_capability(&self, cap: &str) -> Vec<&RegisteredPlugin> {
        self.plugins
            .values()
            .filter(|p| match cap {
                "receive_message" => p.manifest.capabilities.receive_message,
                "send_message" => p.manifest.capabilities.send_message,
                "send_file" => p.manifest.capabilities.send_file,
                "thread_reply" => p.manifest.capabilities.thread_reply,
                "drive_comment" => p.manifest.capabilities.drive_comment,
                "interactive_card" => p.manifest.capabilities.interactive_card,
                _ => false,
            })
            .collect()
    }

    pub fn transition_state(&mut self, plugin_id: &str, state: PluginState) -> bool {
        match self.plugins.get_mut(plugin_id) {
            Some(plugin) => {
                plugin.state = state;
                true
            }
            None => false,
        }
    }
}
