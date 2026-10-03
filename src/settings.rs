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

/// Current Unix timestamp in seconds.
///
/// Returns 0 rather than panicking when the system clock is set before the
/// epoch; a bogus timestamp only affects history ordering.
#[must_use]
pub fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Result cap applied when the user clears the "Maximum Search Results" box.
///
/// Shared by [`Default`] and the UI's commit path so clearing the field and
/// resetting to defaults converge on the same value.
pub const DEFAULT_MAX_RESULTS: usize = 50;

/// Bounds the user-supplied result cap.
///
/// Zero would make every query return nothing; an unbounded cap lets one query
/// allocate without limit.
pub const MAX_RESULTS_LIMIT: usize = 10_000;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)]
pub struct AppSettings {
    #[serde(default = "default_settings_version")]
    pub version: u32,

    // Indexing
    pub index_dirs: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub exclude_folders: Vec<String>, // Explicit folder paths to exclude
    pub auto_index_on_startup: bool,
    #[serde(default = "default_true")]
    pub use_gitignore: bool,
    pub index_file_size_limit_mb: u32,
    #[serde(default)]
    pub custom_extensions: String,

    // Search
    pub max_results: usize,
    pub search_history_enabled: bool,
    pub case_sensitive: bool,
    #[serde(default)]
    pub whole_word: bool,
    pub default_filters: DefaultFilters,
    /// Recorded on each submitted search, ranked by frequency.
    #[serde(default)]
    pub search_history: Vec<SearchHistoryItem>,
    pub filename_index_enabled: bool,

    // Appearance
    pub theme: Theme,
    pub font_size: FontSize,
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
    pub enable_ocr: bool,

    // Pinned files for quick access
    pub pinned_files: Vec<String>,

    #[serde(skip)]
    pub allowed_extensions_cache: AllowedExtensionsCache,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: default_settings_version(),
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
            use_gitignore: true,
            index_file_size_limit_mb: 100,
            custom_extensions: String::new(),
            max_results: DEFAULT_MAX_RESULTS,
            search_history_enabled: true,
            case_sensitive: false,
            whole_word: false,
            default_filters: DefaultFilters::default(),
            search_history: Vec::new(),
            filename_index_enabled: true,
            theme: Theme::default(),
            font_size: FontSize::default(),
            results_per_page: 50,
            minimize_to_tray: true,
            auto_start_on_boot: false,
            double_click_action: DoubleClickAction::default(),
            show_preview_panel: true,
            context_menu_enabled: false,
            global_hotkey: default_global_hotkey(),
            indexing_threads: 4,
            memory_limit_mb: 512,
            enable_ocr: false,
            pinned_files: Vec::new(),
            allowed_extensions_cache: AllowedExtensionsCache::default(),
        }
    }
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Auto,
    Light,
    Dark,
}

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Light => write!(f, "light"),
            Self::Dark => write!(f, "dark"),
        }
    }
}

impl std::str::FromStr for Theme {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            other => Err(format!("Unknown theme: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FontSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl FontSize {
    /// Multiplier applied to every literal text size in the views, in percent.
    ///
    /// The steps are deliberately close together: 125% and 175% reflow enough
    /// text that fixed-width panels start clipping.
    #[must_use]
    pub const fn scale_percent(self) -> u16 {
        match self {
            Self::Small => 90,
            Self::Medium => 100,
            Self::Large => 115,
        }
    }
}

impl std::fmt::Display for FontSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Small => write!(f, "small"),
            Self::Medium => write!(f, "medium"),
            Self::Large => write!(f, "large"),
        }
    }
}

impl std::str::FromStr for FontSize {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "small" => Ok(Self::Small),
            "medium" => Ok(Self::Medium),
            "large" => Ok(Self::Large),
            other => Err(format!("Unknown font size: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DoubleClickAction {
    #[default]
    OpenFile,
    ShowInFolder,
    Preview,
}

impl std::fmt::Display for DoubleClickAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpenFile => write!(f, "open_file"),
            Self::ShowInFolder => write!(f, "show_in_folder"),
            Self::Preview => write!(f, "preview"),
        }
    }
}

impl std::str::FromStr for DoubleClickAction {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "open_file" | "openfile" => Ok(Self::OpenFile),
            "show_in_folder" | "showinfolder" => Ok(Self::ShowInFolder),
            "preview" => Ok(Self::Preview),
            other => Err(format!("Unknown double click action: {other}")),
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
            && let Ok(theme) = val.parse::<Theme>()
        {
            settings.theme = theme;
        }
        if let Ok(val) = std::env::var("FLASH_SEARCH__FONT_SIZE")
            && let Ok(font_size) = val.parse::<FontSize>()
        {
            settings.font_size = font_size;
        }
        if let Ok(val) = std::env::var("FLASH_SEARCH__DOUBLE_CLICK_ACTION")
            && let Ok(action) = val.parse::<DoubleClickAction>()
        {
            settings.double_click_action = action;
        }
        if let Ok(val) = std::env::var("FLASH_SEARCH__INDEXING_THREADS")
            && let Ok(threads) = val.parse::<u8>()
        {
            settings.indexing_threads = threads;
        }
        if let Ok(val) = std::env::var("FLASH_SEARCH__MEMORY_LIMIT_MB")
            && let Ok(limit) = val.parse::<u32>()
        {
            settings.memory_limit_mb = limit;
        }
        if let Ok(val) = std::env::var("FLASH_SEARCH__ENABLE_OCR")
            && let Ok(b) = val.parse::<bool>()
        {
            settings.enable_ocr = b;
        }
        if let Ok(val) = std::env::var("FLASH_SEARCH__AUTO_INDEX_ON_STARTUP")
            && let Ok(b) = val.parse::<bool>()
        {
            settings.auto_index_on_startup = b;
        }
        if let Ok(val) = std::env::var("FLASH_SEARCH__USE_GITIGNORE")
            && let Ok(b) = val.parse::<bool>()
        {
            settings.use_gitignore = b;
        }
        if let Ok(val) = std::env::var("FLASH_SEARCH__MAX_RESULTS")
            && let Ok(limit) = val.parse::<usize>()
        {
            settings.max_results = limit;
        }
        if let Ok(val) = std::env::var("FLASH_SEARCH__CASE_SENSITIVE")
            && let Ok(b) = val.parse::<bool>()
        {
            settings.case_sensitive = b;
        }
        if let Ok(val) = std::env::var("FLASH_SEARCH__FILENAME_INDEX_ENABLED")
            && let Ok(b) = val.parse::<bool>()
        {
            settings.filename_index_enabled = b;
        }
        if let Ok(val) = std::env::var("FLASH_SEARCH__MINIMIZE_TO_TRAY")
            && let Ok(b) = val.parse::<bool>()
        {
            settings.minimize_to_tray = b;
        }
        if let Ok(val) = std::env::var("FLASH_SEARCH__AUTO_START_ON_BOOT")
            && let Ok(b) = val.parse::<bool>()
        {
            settings.auto_start_on_boot = b;
        }

        Ok(settings)
    }

    /// Persists settings atomically.
    ///
    /// Writes a sibling temp file, flushes it to disk, then renames over the
    /// target. The rename is atomic on both NTFS and POSIX, so a crash or a
    /// full disk mid-write leaves the previous settings intact rather than a
    /// half-written file that would fail to parse on the next launch.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or any filesystem step fails.
    pub fn save(&self, settings: &AppSettings) -> Result<()> {
        let content = serde_json::to_string_pretty(settings)
            .map_err(|e| FlashError::config("serialize_settings", e.to_string()))?;

        let tmp_path = self.path.with_extension("tmp");

        {
            use std::io::Write as _;
            let mut file =
                fs::File::create(&tmp_path).map_err(|e| FlashError::Io(std::sync::Arc::new(e)))?;
            file.write_all(content.as_bytes())
                .map_err(|e| FlashError::Io(std::sync::Arc::new(e)))?;
            // Without the flush the rename can land before the bytes reach the
            // platter, leaving an empty settings file after a power loss.
            file.sync_all()
                .map_err(|e| FlashError::Io(std::sync::Arc::new(e)))?;
        }

        if let Err(e) = fs::rename(&tmp_path, &self.path) {
            // Do not leave the temp file behind to accumulate on every failure.
            let _ = fs::remove_file(&tmp_path);
            return Err(FlashError::Io(std::sync::Arc::new(e)));
        }
        Ok(())
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

    /// Settings written by an older build carry fields that have since been
    /// removed (`fuzzy_matching`, `show_file_extensions`, `recent_searches`).
    /// An upgrade must not fail to load such a file -- that would silently
    /// reset every user back to defaults on first launch.
    #[test]
    fn settings_with_removed_fields_still_load() {
        let temp_dir = tempdir().unwrap();
        let manager = SettingsManager::new(temp_dir.path());

        let legacy = serde_json::json!({
            "version": 1,
            "index_dirs": ["C:/Users/test/Documents"],
            "exclude_patterns": ["target"],
            "exclude_folders": [],
            "auto_index_on_startup": true,
            "use_gitignore": true,
            "index_file_size_limit_mb": 100,
            "custom_extensions": "log",
            "max_results": 250,
            "search_history_enabled": true,
            "fuzzy_matching": true,
            "case_sensitive": true,
            "whole_word": false,
            "default_filters": {
                "file_types": ["txt"],
                "min_size": "",
                "max_size": "",
                "date_range": "anytime"
            },
            "recent_searches": ["old query"],
            "search_history": [],
            "filename_index_enabled": true,
            "theme": "dark",
            "font_size": "large",
            "show_file_extensions": true,
            "results_per_page": 50,
            "minimize_to_tray": true,
            "auto_start_on_boot": false,
            "double_click_action": "show_in_folder",
            "show_preview_panel": true,
            "context_menu_enabled": false,
            "indexing_threads": 4,
            "memory_limit_mb": 2048,
            "enable_ocr": false,
            "pinned_files": ["C:/Users/test/notes.md"]
        });

        fs::write(&manager.path, serde_json::to_vec_pretty(&legacy).unwrap()).unwrap();

        let loaded = manager.load().expect("legacy settings must still load");
        assert_eq!(loaded.max_results, 250);
        assert_eq!(loaded.font_size, FontSize::Large);
        assert_eq!(loaded.double_click_action, DoubleClickAction::ShowInFolder);
        assert_eq!(
            loaded.pinned_files,
            vec!["C:/Users/test/notes.md".to_string()]
        );
        assert_eq!(
            loaded.index_dirs,
            vec!["C:/Users/test/Documents".to_string()]
        );
    }

    /// A truncated settings file must surface as an error the caller can fall
    /// back from -- not as silently-default settings that overwrite the user's
    /// real configuration on the next save.
    #[test]
    fn corrupt_settings_file_reports_an_error() {
        let temp_dir = tempdir().unwrap();
        let manager = SettingsManager::new(temp_dir.path());
        fs::write(&manager.path, b"{ this is not json").unwrap();
        assert!(
            manager.load().is_err(),
            "a corrupt file must be reported, not silently defaulted"
        );
    }

    /// A failed save must not leave its temp file behind, or every failure would
    /// add another stray `settings.tmp` to the app data directory.
    #[test]
    fn save_does_not_leave_a_temp_file_behind() {
        let temp_dir = tempdir().unwrap();
        let manager = SettingsManager::new(temp_dir.path());
        manager.save(&AppSettings::default()).unwrap();
        assert!(
            !manager.path.with_extension("tmp").exists(),
            "successful save must clean up its temp file"
        );
        assert!(manager.path.exists());
    }
}
