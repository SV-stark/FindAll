use crate::error::{FlashError, Result};
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
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
    /// NOTE: Clone intentionally resets the once-lock to empty.
    /// This is common for types that wrap a cache/lazy value where
    /// each clone should maintain its own independent cache lifecycle.
    fn clone(&self) -> Self {
        Self(std::sync::OnceLock::new())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, SmartDefault)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)]
pub struct AppSettings {
    #[serde(default = "default_settings_version")]
    #[default(default_settings_version())]
    pub version: u32,

    // Indexing
    pub index_dirs: Vec<String>,
    #[default(vec![
        ".git/".to_string(),
        "node_modules/".to_string(),
        "target/".to_string(),
        "AppData/".to_string(),
        "*.tmp".to_string(),
        "*.temp".to_string(),
        "Thumbs.db".to_string(),
        ".DS_Store".to_string(),
    ])]
    pub exclude_patterns: Vec<String>,
    #[default(vec![
        "$RECYCLE.BIN".to_string(),
        "System Volume Information".to_string(),
    ])]
    pub exclude_folders: Vec<String>, // Explicit folder paths to exclude
    #[default(true)]
    pub auto_index_on_startup: bool,
    #[serde(default = "default_true")]
    #[default(true)]
    pub use_gitignore: bool,
    #[default(100)]
    pub index_file_size_limit_mb: u32,
    #[serde(default)]
    pub custom_extensions: String,

    // Search
    #[default(50)]
    pub max_results: usize,
    #[default(true)]
    pub search_history_enabled: bool,
    #[default(true)]
    pub fuzzy_matching: bool,
    pub case_sensitive: bool,
    #[serde(default)]
    pub whole_word: bool,
    pub default_filters: DefaultFilters,
    #[serde(default)]
    pub recent_searches: Vec<String>,
    #[serde(default)]
    pub search_history: Vec<SearchHistoryItem>,
    #[default(true)]
    pub filename_index_enabled: bool,

    // Appearance
    pub theme: Theme,
    pub font_size: FontSize,
    #[default(true)]
    pub show_file_extensions: bool,
    #[default(50)]
    pub results_per_page: usize,

    // Behavior
    #[default(true)]
    pub minimize_to_tray: bool,
    pub auto_start_on_boot: bool,
    pub double_click_action: DoubleClickAction,
    #[default(true)]
    pub show_preview_panel: bool,
    pub context_menu_enabled: bool,

    #[serde(default = "default_global_hotkey")]
    #[default(default_global_hotkey())]
    pub global_hotkey: String,

    // Performance
    #[default(4)]
    pub indexing_threads: u8,
    #[default(512)]
    pub memory_limit_mb: u32,
    #[default(false)]
    pub enable_ocr: bool,

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

const fn default_true() -> bool {
    true
}

const fn default_settings_version() -> u32 {
    1
}

#[derive(
    Debug, Clone, Serialize, Deserialize, Default, Display, EnumString, EnumIter, PartialEq, Eq,
)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Auto,
    Light,
    Dark,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, Default, Display, EnumString, EnumIter, PartialEq, Eq,
)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum FontSize {
    Small,
    #[default]
    Medium,
    Large,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, Default, Display, EnumString, EnumIter, PartialEq, Eq,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum DoubleClickAction {
    #[default]
    OpenFile,
    ShowInFolder,
    Preview,
}

pub struct SettingsManager {
    path: PathBuf,
}

impl AppSettings {
    pub fn get_allowed_extensions(&self) -> &std::collections::HashSet<String> {
        self.allowed_extensions_cache.0.get_or_init(|| {
            let mut exts = std::collections::HashSet::new();
            for ext in COMMON_EXTENSIONS {
                exts.insert((*ext).to_string());
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
    #[must_use]
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            path: app_data_dir.join("settings.json"),
        }
    }

    pub fn load(&self) -> Result<AppSettings> {
        let mut settings = if self.path.exists() {
            let content = fs::read_to_string(&self.path)
                .map_err(|e| FlashError::config("read_settings", e.to_string()))?;
            serde_json::from_str(&content)
                .map_err(|e| FlashError::config("parse_settings", e.to_string()))?
        } else {
            AppSettings::default()
        };

        // Override with environment variables (e.g., FLASH_SEARCH__THEME=dark)
        if let Ok(val) = std::env::var("FLASH_SEARCH__THEME")
            && let Ok(theme) = val.parse::<Theme>() {
                settings.theme = theme;
            }
        if let Ok(val) = std::env::var("FLASH_SEARCH__FONT_SIZE")
            && let Ok(font_size) = val.parse::<FontSize>() {
                settings.font_size = font_size;
            }
        if let Ok(val) = std::env::var("FLASH_SEARCH__DOUBLE_CLICK_ACTION")
            && let Ok(action) = val.parse::<DoubleClickAction>() {
                settings.double_click_action = action;
            }
        if let Ok(val) = std::env::var("FLASH_SEARCH__INDEXING_THREADS")
            && let Ok(threads) = val.parse::<u8>() {
                settings.indexing_threads = threads;
            }
        if let Ok(val) = std::env::var("FLASH_SEARCH__MEMORY_LIMIT_MB")
            && let Ok(limit) = val.parse::<u32>() {
                settings.memory_limit_mb = limit;
            }
        if let Ok(val) = std::env::var("FLASH_SEARCH__ENABLE_OCR")
            && let Ok(b) = val.parse::<bool>() {
                settings.enable_ocr = b;
            }
        if let Ok(val) = std::env::var("FLASH_SEARCH__AUTO_INDEX_ON_STARTUP")
            && let Ok(b) = val.parse::<bool>() {
                settings.auto_index_on_startup = b;
            }
        if let Ok(val) = std::env::var("FLASH_SEARCH__USE_GITIGNORE")
            && let Ok(b) = val.parse::<bool>() {
                settings.use_gitignore = b;
            }
        if let Ok(val) = std::env::var("FLASH_SEARCH__MAX_RESULTS")
            && let Ok(limit) = val.parse::<usize>() {
                settings.max_results = limit;
            }
        if let Ok(val) = std::env::var("FLASH_SEARCH__FUZZY_MATCHING")
            && let Ok(b) = val.parse::<bool>() {
                settings.fuzzy_matching = b;
            }
        if let Ok(val) = std::env::var("FLASH_SEARCH__CASE_SENSITIVE")
            && let Ok(b) = val.parse::<bool>() {
                settings.case_sensitive = b;
            }
        if let Ok(val) = std::env::var("FLASH_SEARCH__FILENAME_INDEX_ENABLED")
            && let Ok(b) = val.parse::<bool>() {
                settings.filename_index_enabled = b;
            }
        if let Ok(val) = std::env::var("FLASH_SEARCH__MINIMIZE_TO_TRAY")
            && let Ok(b) = val.parse::<bool>() {
                settings.minimize_to_tray = b;
            }
        if let Ok(val) = std::env::var("FLASH_SEARCH__AUTO_START_ON_BOOT")
            && let Ok(b) = val.parse::<bool>() {
                settings.auto_start_on_boot = b;
            }

        Ok(settings)
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
