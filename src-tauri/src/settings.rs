use crate::error::{FlashError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub index_dirs: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub theme: String,
    pub max_results: usize,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            index_dirs: Vec::new(),
            exclude_patterns: vec![
                ".git/".to_string(),
                "node_modules/".to_string(),
                "target/".to_string(),
                "AppData/".to_string(),
            ],
            theme: "auto".to_string(),
            max_results: 50,
        }
    }
}

pub struct SettingsManager {
    path: PathBuf,
}

impl SettingsManager {
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            path: app_data_dir.join("settings.json"),
        }
    }

    pub fn load(&self) -> Result<AppSettings> {
        if !self.path.exists() {
            let defaults = AppSettings::default();
            self.save(&defaults)?;
            return Ok(defaults);
        }

        let content = fs::read_to_string(&self.path).map_err(|e| FlashError::Io(e))?;

        serde_json::from_str(&content).map_err(|e| FlashError::Config(e.to_string()))
    }

    pub fn save(&self, settings: &AppSettings) -> Result<()> {
        let content = serde_json::to_string_pretty(settings)
            .map_err(|e| FlashError::Config(e.to_string()))?;

        fs::write(&self.path, content).map_err(|e| FlashError::Io(e))
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        self.save(settings)
    }
}
