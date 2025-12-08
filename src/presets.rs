use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::config::APP_ID;

/// A single connection in a preset (stored by port names, not IDs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetConnection {
    pub output_node: String,
    pub output_port: String,
    pub input_node: String,
    pub input_port: String,
}

/// A named preset containing a list of connections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub connections: Vec<PresetConnection>,
}

/// Collection of all saved presets
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PresetStore {
    pub presets: HashMap<String, Preset>,
    /// Name of the currently active (auto-connecting) preset, if any
    #[serde(default)]
    pub active_preset: Option<String>,
}

impl PresetStore {
    /// Get the path to the presets file
    fn presets_path() -> Option<PathBuf> {
        let config_dir = dirs::config_dir()?;
        let app_dir = config_dir.join(APP_ID);
        Some(app_dir.join("presets.json"))
    }

    /// Load presets from disk
    pub fn load() -> Self {
        let path = match Self::presets_path() {
            Some(p) => p,
            None => return Self::default(),
        };

        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(e) => {
                log::warn!("Failed to load presets: {}", e);
                Self::default()
            }
        }
    }

    /// Save presets to disk
    pub fn save(&self) -> Result<(), String> {
        let path = Self::presets_path().ok_or("Could not determine config directory")?;

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {}", e))?;
        }

        let content =
            serde_json::to_string_pretty(self).map_err(|e| format!("Failed to serialize: {}", e))?;

        fs::write(&path, content).map_err(|e| format!("Failed to write presets: {}", e))?;

        Ok(())
    }

    /// Add or update a preset
    pub fn add_preset(&mut self, preset: Preset) {
        self.presets.insert(preset.name.clone(), preset);
    }

    /// Remove a preset by name
    pub fn remove_preset(&mut self, name: &str) {
        self.presets.remove(name);
    }

    /// Get a preset by name
    pub fn get_preset(&self, name: &str) -> Option<&Preset> {
        self.presets.get(name)
    }

    /// Get all preset names
    pub fn preset_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.presets.keys().cloned().collect();
        names.sort();
        names
    }

    /// Activate a preset for auto-connecting
    pub fn activate_preset(&mut self, name: &str) {
        if self.presets.contains_key(name) {
            self.active_preset = Some(name.to_string());
        }
    }

    /// Deactivate the current preset
    pub fn deactivate_preset(&mut self) {
        self.active_preset = None;
    }

    /// Get the currently active preset, if any
    pub fn get_active_preset(&self) -> Option<&Preset> {
        self.active_preset
            .as_ref()
            .and_then(|name| self.presets.get(name))
    }

    /// Check if a preset is currently active
    pub fn is_active(&self, name: &str) -> bool {
        self.active_preset.as_deref() == Some(name)
    }
}
