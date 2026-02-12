use crate::error::{FlashError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHistoryItem {
    pub query: String,
    pub frequency: u32,
    pub last_used: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    // Indexing
    pub index_dirs: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub auto_index_on_startup: bool,
    pub index_file_size_limit_mb: u32,

    // Search
    pub max_results: usize,
    pub search_history_enabled: bool,
    pub fuzzy_matching: bool,
    pub case_sensitive: bool,
    pub default_filters: DefaultFilters,
    pub recent_searches: Option<Vec<String>>,
    pub search_history: Option<Vec<SearchHistoryItem>>,
    pub filename_index_enabled: bool,

    // Appearance
    pub theme: Theme,
    pub font_size: FontSize,
    pub show_file_extensions: bool,
    pub results_per_page: usize,

    // Behavior
    pub minimize_to_tray: bool,
    pub auto_start_on_boot: bool,
    pub double_click_action: DoubleClickAction,
    pub show_preview_panel: bool,

    // Performance
    pub indexing_threads: u8,
    pub memory_limit_mb: u32,

    // Pinned files for quick access
    pub pinned_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultFilters {
    pub file_types: Vec<String>,
    pub min_size_mb: Option<u32>,
    pub max_size_mb: Option<u32>,
    pub modified_within_days: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Theme {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "dark")]
    Dark,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::Auto
    }
}

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Theme::Auto => write!(f, "auto"),
            Theme::Light => write!(f, "light"),
            Theme::Dark => write!(f, "dark"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FontSize {
    #[serde(rename = "small")]
    Small,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "large")]
    Large,
}

impl Default for FontSize {
    fn default() -> Self {
        FontSize::Medium
    }
}

impl std::fmt::Display for FontSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FontSize::Small => write!(f, "small"),
            FontSize::Medium => write!(f, "medium"),
            FontSize::Large => write!(f, "large"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DoubleClickAction {
    #[serde(rename = "open_file")]
    OpenFile,
    #[serde(rename = "show_in_folder")]
    ShowInFolder,
    #[serde(rename = "preview")]
    Preview,
}

impl Default for DoubleClickAction {
    fn default() -> Self {
        DoubleClickAction::OpenFile
    }
}

impl Default for DefaultFilters {
    fn default() -> Self {
        Self {
            file_types: vec![],
            min_size_mb: None,
            max_size_mb: None,
            modified_within_days: None,
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            // Indexing
            index_dirs: Vec::new(),
            exclude_patterns: vec![
                ".git/".to_string(),
                "node_modules/".to_string(),
                "target/".to_string(),
                "AppData/".to_string(),
                "*.tmp".to_string(),
                "*.temp".to_string(),
                "Thumbs.db".to_string(),
                ".DS_Store".to_string(),
            ],
            auto_index_on_startup: true,
            index_file_size_limit_mb: 100,

            // Search
            max_results: 50,
            search_history_enabled: true,
            fuzzy_matching: true,
            case_sensitive: false,
            default_filters: DefaultFilters::default(),
            recent_searches: Some(vec![]),
            search_history: Some(vec![]),
            filename_index_enabled: false,

            // Appearance
            theme: Theme::default(),
            font_size: FontSize::default(),
            show_file_extensions: true,
            results_per_page: 50,

            // Behavior
            minimize_to_tray: true,
            auto_start_on_boot: false,
            double_click_action: DoubleClickAction::default(),
            show_preview_panel: true,

            // Performance
            indexing_threads: 4,
            memory_limit_mb: 512,

            // Pinned files
            pinned_files: vec![],
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_settings_save_load() {
        let temp_dir = tempdir().unwrap();
        let manager = SettingsManager::new(temp_dir.path());

        let mut settings = AppSettings::default();
        settings.max_results = 100;
        settings.theme = Theme::Dark;

        manager.save(&settings).unwrap();
        let loaded = manager.load().unwrap();

        assert_eq!(loaded.max_results, 100);
        assert!(matches!(loaded.theme, Theme::Dark));
    }
}
