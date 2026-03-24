use crate::error::{FlashError, Result};
use config::{Config, Environment, File as ConfigFile};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use strum::{Display, EnumIter, EnumString};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHistoryItem {
    pub query: String,
    pub frequency: u32,
    pub last_used: u64,
}

pub const COMMON_EXTENSIONS: &[&str] = &[
    "pdf", "docx", "doc", "xlsx", "xls", "pptx", "ppt", "odt", "rtf", "jpeg", "jpg", "png", "tiff",
    "heic", "heif", "zip", "7z", "rar", "tar", "gz", "eml", "msg", "pst", "epub", "mobi", "azw3",
    "md", "json", "xml", "txt", "csv", "tsv", "rs", "py", "js", "ts", "go", "java", "c", "cpp",
    "h", "hpp", "cs", "html", "css",
];

#[derive(Debug, Default)]
pub struct AllowedExtensionsCache(pub std::sync::OnceLock<std::collections::HashSet<String>>);

impl Clone for AllowedExtensionsCache {
    fn clone(&self) -> Self {
        AllowedExtensionsCache(std::sync::OnceLock::new())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_settings_version")]
    pub version: u32,

    // Indexing
    pub index_dirs: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub exclude_folders: Vec<String>, // Explicit folder paths to exclude
    pub auto_index_on_startup: bool,
    pub index_file_size_limit_mb: u32,
    #[serde(default)]
    pub custom_extensions: String,

    // Search
    pub max_results: usize,
    pub search_history_enabled: bool,
    pub fuzzy_matching: bool,
    pub case_sensitive: bool,
    #[serde(default)]
    pub whole_word: bool,
    pub default_filters: DefaultFilters,
    #[serde(default)]
    pub recent_searches: Vec<String>,
    #[serde(default)]
    pub search_history: Vec<SearchHistoryItem>,
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
    pub context_menu_enabled: bool,

    #[serde(default = "default_global_hotkey")]
    pub global_hotkey: String,

    // Performance
    pub indexing_threads: u8,
    pub memory_limit_mb: u32,

    // Pinned files for quick access
    pub pinned_files: Vec<String>,

    #[serde(skip)]
    pub allowed_extensions_cache: AllowedExtensionsCache,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DefaultFilters {
    pub file_types: Vec<String>,
    pub min_size_mb: Option<u32>,
    pub max_size_mb: Option<u32>,
    pub modified_within_days: Option<u32>,
}

fn default_global_hotkey() -> String {
    "Alt+Space".to_string()
}

fn default_settings_version() -> u32 {
    1
}

#[derive(
    Debug, Clone, Serialize, Deserialize, Default, Display, EnumString, EnumIter, PartialEq,
)]
#[strum(serialize_all = "lowercase")]
pub enum Theme {
    #[default]
    Auto,
    Light,
    Dark,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, Default, Display, EnumString, EnumIter, PartialEq,
)]
#[strum(serialize_all = "lowercase")]
pub enum FontSize {
    Small,
    #[default]
    Medium,
    Large,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, Default, Display, EnumString, EnumIter, PartialEq,
)]
#[strum(serialize_all = "snake_case")]
pub enum DoubleClickAction {
    #[default]
    OpenFile,
    ShowInFolder,
    Preview,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: default_settings_version(),
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
            exclude_folders: vec![
                "$RECYCLE.BIN".to_string(),
                "System Volume Information".to_string(),
            ],
            auto_index_on_startup: true,
            index_file_size_limit_mb: 100,
            custom_extensions: String::new(),

            // Search
            max_results: 50,
            search_history_enabled: true,
            fuzzy_matching: true,
            case_sensitive: false,
            whole_word: false,
            default_filters: DefaultFilters::default(),
            recent_searches: vec![],
            search_history: vec![],
            filename_index_enabled: true,

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
            context_menu_enabled: false,
            global_hotkey: default_global_hotkey(),

            // Performance
            indexing_threads: 4,
            memory_limit_mb: 512,

            // Pinned files
            pinned_files: vec![],
            allowed_extensions_cache: AllowedExtensionsCache::default(),
        }
    }
}

pub struct SettingsManager {
    path: PathBuf,
}

impl AppSettings {
    pub fn get_allowed_extensions(&self) -> &std::collections::HashSet<String> {
        self.allowed_extensions_cache.0.get_or_init(|| {
            let mut exts = std::collections::HashSet::new();
            for ext in COMMON_EXTENSIONS {
                exts.insert(ext.to_string());
            }
            for custom in self.custom_extensions.split(',') {
                let trimmed = custom.trim().trim_start_matches('.').to_lowercase();
                if !trimmed.is_empty() {
                    exts.insert(trimmed);
                }
            }
            exts
        })
    }
}

impl SettingsManager {
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            path: app_data_dir.join("settings.json"),
        }
    }

    pub fn load(&self) -> Result<AppSettings> {
        let builder = Config::builder()
            // Start with default settings
            .add_source(ConfigFile::from_str(
                &serde_json::to_string(&AppSettings::default()).unwrap(),
                config::FileFormat::Json,
            ))
            // Add settings from file if it exists
            .add_source(ConfigFile::from(self.path.clone()).required(false))
            // Override with environment variables (e.g., FLASH_SEARCH_THEME=dark)
            .add_source(Environment::with_prefix("FLASH_SEARCH").separator("__"));

        match builder.build() {
            Ok(config) => config
                .try_deserialize()
                .map_err(|e| FlashError::config("parse_settings", e.to_string())),
            Err(e) => Err(FlashError::config("build_config", e.to_string())),
        }
    }

    pub fn save(&self, settings: &AppSettings) -> Result<()> {
        let content = serde_json::to_string_pretty(settings)
            .map_err(|e| FlashError::config("serialize_settings", e.to_string()))?;

        let tmp_path = self.path.with_extension("tmp");
        fs::write(&tmp_path, content).map_err(|e| FlashError::Io(std::sync::Arc::new(e)))?;
        fs::rename(&tmp_path, &self.path).map_err(|e| FlashError::Io(std::sync::Arc::new(e)))
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

        let settings = AppSettings {
            max_results: 100,
            theme: Theme::Dark,
            ..Default::default()
        };

        manager.save(&settings).unwrap();
        let loaded = manager.load().unwrap();
        assert_eq!(loaded.max_results, 100);
        assert_eq!(loaded.theme, Theme::Dark);
    }
}
