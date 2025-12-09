use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::config::APP_ID;

/// Application settings that persist across restarts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Whether to start minimized to the system tray
    #[serde(default)]
    pub start_minimized: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            start_minimized: false,
        }
    }
}

impl Settings {
    /// Get the path to the settings file
    fn settings_path() -> Option<PathBuf> {
        let config_dir = dirs::config_dir()?;
        let app_dir = config_dir.join(APP_ID);
        Some(app_dir.join("settings.json"))
    }

    /// Load settings from disk
    pub fn load() -> Self {
        let path = match Self::settings_path() {
            Some(p) => p,
            None => return Self::default(),
        };

        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(e) => {
                log::warn!("Failed to load settings: {}", e);
                Self::default()
            }
        }
    }

    /// Save settings to disk
    pub fn save(&self) -> Result<(), String> {
        let path = Self::settings_path().ok_or("Could not determine config directory")?;

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {}", e))?;
        }

        let content =
            serde_json::to_string_pretty(self).map_err(|e| format!("Failed to serialize: {}", e))?;

        fs::write(&path, content).map_err(|e| format!("Failed to write settings: {}", e))?;

        Ok(())
    }
}
