use crate::commands::AppState;
use crate::commands::{
    get_file_preview_highlighted_internal, search_filenames_internal, search_query_internal,
};
use crate::error::FlashError;
use crate::indexer::searcher::{SearchParams, SearchResult};
use crate::scanner::ProgressEvent;
use crate::settings::AppSettings;
use compact_str::CompactString;
use iced::futures::SinkExt;
use iced::widget::Id;
use iced::{Element, Subscription, Task};
use parking_lot::Mutex;
use std::sync::Arc;

pub mod icons;
pub mod search;
pub mod settings;
pub mod theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tab {
    Search,
    Settings,
}

#[derive(Debug, Clone)]
pub struct FileItem {
    pub score: f32,
    pub path: String,
    pub title: String,
    pub extension: Option<CompactString>,
    pub size: Option<u64>,
    pub modified: Option<u64>,
    pub snippets: Vec<String>,
}

impl From<SearchResult> for FileItem {
    fn from(r: SearchResult) -> Self {
        let path_clone = r.file_path.clone();
        Self {
            score: r.score,
            path: r.file_path,
            title: r.title.as_ref().map_or_else(
                || {
                    std::path::Path::new(&path_clone)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&path_clone)
                        .to_string()
                },
                std::string::ToString::to_string,
            ),
            extension: r.extension,
            size: r.size,
            modified: r.modified,
            snippets: r.snippets,
        }
    }
}

impl From<crate::models::FilenameSearchResult> for FileItem {
    fn from(r: crate::models::FilenameSearchResult) -> Self {
        let path_clone = r.file_path.clone();
        Self {
            score: 1.0,
            path: r.file_path,
            title: r.file_name.to_string(),
            extension: std::path::Path::new(&path_clone)
                .extension()
                .and_then(|e| e.to_str())
                .map(CompactString::from),
            size: None,
            modified: None,
            snippets: Vec::new(),
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    strum::Display,
    strum::EnumIter,
    strum::EnumString,
)]
pub enum DateFilter {
    #[default]
    #[strum(serialize = "Anytime")]
    Anytime,
    #[strum(serialize = "Today")]
    Today,
    #[strum(serialize = "Last 7 Days")]
    Last7Days,
    #[strum(serialize = "Last 30 Days")]
    Last30Days,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    strum::Display,
    strum::EnumIter,
    strum::EnumString,
)]
pub enum SearchMode {
    #[default]
    #[strum(serialize = "Full Text")]
    FullText,
    #[strum(serialize = "Filename")]
    Filename,
}

pub fn get_search_input_id() -> Id {
    static ID: std::sync::OnceLock<Id> = std::sync::OnceLock::new();
    ID.get_or_init(Id::unique).clone()
}

pub fn get_progress_subscription_id() -> Id {
    static ID: std::sync::OnceLock<Id> = std::sync::OnceLock::new();
    ID.get_or_init(Id::unique).clone()
}

pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// # Panics
///
/// Panics if the timestamp is out of range for the system's local time.
pub fn format_date(timestamp: u64) -> String {
    jiff::Timestamp::from_second(i64::try_from(timestamp).unwrap_or(i64::MAX))
        .unwrap_or_else(|_| jiff::Timestamp::from_second(0).unwrap())
        .to_zoned(jiff::tz::TimeZone::system())
        .strftime("%Y-%m-%d %H:%M")
        .to_string()
}

#[derive(Debug, Clone)]
pub enum Message {
    TabChanged(Tab),
    SearchQueryChanged(String),
    SearchSubmitted,
    SearchResultsReceived(usize, Vec<FileItem>),
    SearchError(FlashError),
    ResultSelected(usize),
    ItemHovered(Option<usize>),
    OpenFile(String),
    OpenFolder(String),
    CopyPath(String),
    ShowContextMenu(usize),
    // Filters
    FilterExtensionChanged(String),
    ToggleFilterExtension(String),
    MinSizeChanged(String),
    MaxSizeChanged(String),
    SizeUnitChanged(String),
    DateFilterChanged(DateFilter),
    SearchModeChanged(SearchMode),
    ToggleCaseSensitive(bool),
    ToggleWholeWord(bool),
    ClearFilters,
    // Settings
    MaxResultsChanged(String),
    ExcludePatternsChanged(String),
    CustomExtensionsChanged(String),
    GlobalHotkeyChanged(String),
    AddFolder,
    RemoveFolder(usize),
    ToggleMinimizeToTray(bool),
    ToggleAutoStart(bool),
    ToggleContextMenu(bool),
    ToggleGitignore(bool),
    ToggleTheme,
    RebuildIndex,
    IndexDirAdded(String),
    RemoveIndexDir(usize),
    ExcludePatternAdded(String),
    RemoveExcludePattern(usize),
    SaveSettings,
    ResetSettings,
    ThemeChanged(crate::settings::Theme),
    FontSizeChanged(crate::settings::FontSize),
    // Lifecycle
    PollProgress,
    PollProgressResult(Option<ProgressEvent>),
    PreviewLoaded(crate::models::PreviewResult),
    IndexRebuilt,
    RebuildProgress(f32),
    StatusUpdate(String),
    // Pinned
    PinFile(String),
    UnpinFile(String),
    // System
    PickFolder,
    FolderPicked(Option<String>),
    ExportResults(String), // format: "csv" or "json"
    WindowIdCaptured(iced::window::Id),
    WindowUnfocused(iced::window::Id),
    DismissError,
    Quit,
    NoOp,
    ToggleSidebar,
}

#[allow(clippy::struct_excessive_bools)]
pub struct App {
    pub(crate) state: Option<Arc<AppState>>,
    pub(crate) error: Option<String>,
    pub(crate) search_error: Option<String>,
    pub(crate) db_corrupted_dismissed: bool,
    pub(crate) active_tab: Tab,
    pub(crate) search_query: String,
    pub(crate) results: Vec<FileItem>,
    pub(crate) selected_index: Option<usize>,
    pub(crate) hovered_item_index: Option<usize>,
    pub(crate) is_searching: bool,
    pub(crate) search_id: usize,
    pub(crate) filter_extension: String,
    pub(crate) filter_extensions: std::collections::HashSet<String>,
    pub(crate) min_size: String,
    pub(crate) max_size: String,
    pub(crate) size_unit: String,
    pub(crate) date_filter: DateFilter,
    pub(crate) search_mode: SearchMode,
    pub(crate) filter_size: String,
    pub(crate) files_indexed: i32,
    pub(crate) index_size: String,
    pub(crate) rebuild_status: Option<String>,
    pub(crate) rebuild_progress: Option<f32>,
    pub(crate) rebuild_eta: Option<u64>,
    pub(crate) is_dark: bool,
    pub(crate) sidebar_collapsed: bool,
    pub(crate) settings: AppSettings,
    pub(crate) new_index_dir: String,
    pub(crate) new_exclude_pattern: String,
    pub(crate) preview_result: Option<crate::models::PreviewResult>,
    pub(crate) is_loading_preview: bool,
    #[allow(dead_code)]
    pub(crate) tray_icon: Option<tray_icon::TrayIcon>,
    pub(crate) window_id: Option<iced::window::Id>,
    pub(crate) progress_rx: Option<flume::Receiver<ProgressEvent>>,
}

#[derive(Debug, Clone)]
struct SubscriptionData {
    rx: flume::Receiver<ProgressEvent>,
}

impl std::hash::Hash for SubscriptionData {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // same_channel defines equality; use a constant so hash is consistent.
        // Iced uses this only for subscription deduplication within a single run.
        0u8.hash(state);
    }
}

impl PartialEq for SubscriptionData {
    fn eq(&self, other: &Self) -> bool {
        self.rx.same_channel(&other.rx)
    }
}

impl Eq for SubscriptionData {}

impl Default for App {
    fn default() -> Self {
        Self {
            state: None,
            error: None,
            search_error: None,
            db_corrupted_dismissed: false,
            active_tab: Tab::Search,
            search_query: String::new(),
            results: Vec::new(),
            selected_index: None,
            hovered_item_index: None,
            is_searching: false,
            search_id: 0,
            filter_extension: String::new(),
            filter_extensions: std::collections::HashSet::new(),
            min_size: String::new(),
            max_size: String::new(),
            size_unit: "MB".to_string(),
            date_filter: DateFilter::Anytime,
            search_mode: SearchMode::FullText,
            filter_size: String::new(),
            files_indexed: 0,
            index_size: "0 MB".to_string(),
            rebuild_status: None,
            rebuild_progress: None,
            rebuild_eta: None,
            is_dark: false,
            sidebar_collapsed: false,
            settings: AppSettings::default(),
            new_index_dir: String::new(),
            new_exclude_pattern: String::new(),
            preview_result: None,
            is_loading_preview: false,
            tray_icon: None,
            window_id: None,
            progress_rx: None,
        }
    }
}

impl App {
    fn new(
        state: Result<Arc<AppState>, String>,
        progress_rx: Option<flume::Receiver<ProgressEvent>>,
        initial_dir: Option<String>,
    ) -> Self {
        match state {
            Ok(state) => {
                let settings = state.settings_manager.load().unwrap_or_default();
                let index_stats = state.indexer.get_statistics().unwrap_or_default();
                let index_size = format!(
                    "{:.1} MB",
                    (index_stats.total_size_bytes as f64) / 1_048_576.0
                );
                let is_dark = matches!(settings.theme, crate::settings::Theme::Dark);

                let mut app = Self {
                    state: Some(state),
                    settings: settings.clone(),
                    files_indexed: i32::try_from(index_stats.total_documents).unwrap_or(i32::MAX),
                    index_size,
                    is_dark,
                    progress_rx,
                    ..Default::default()
                };

                for ext in &settings.default_filters.file_types {
                    app.filter_extensions.insert(ext.clone());
                }

                if let Some(dir) = initial_dir {
                    app.search_query = format!("path:\"{dir}\" ");
                }

                app
            }
            Err(e) => Self {
                error: Some(e),
                progress_rx,
                ..Default::default()
            },
        }
    }

    fn parse_size_filter(size_str: &str) -> (Option<u64>, Option<u64>) {
        if size_str.is_empty() {
            return (None, None);
        }

        let size_str = size_str.trim();
        let (op, num_str) = size_str.strip_prefix(">=").map_or_else(
            || {
                size_str.strip_prefix("<=").map_or_else(
                    || {
                        size_str.strip_prefix(">").map_or_else(
                            || {
                                size_str
                                    .strip_prefix("<")
                                    .map_or_else(|| (">=", size_str), |stripped| ("<", stripped))
                            },
                            |stripped| (">", stripped),
                        )
                    },
                    |stripped| ("<=", stripped),
                )
            },
            |stripped| (">=", stripped),
        );

        let num_str = num_str.trim();
        let mut multiplier: u64 = 1;
        let mut clean_num = num_str;

        if num_str.to_uppercase().ends_with("GB") {
            multiplier = 1024 * 1024 * 1024;
            clean_num = num_str[..num_str.len() - 2].trim();
        } else if num_str.to_uppercase().ends_with("MB") {
            multiplier = 1024 * 1024;
            clean_num = num_str[..num_str.len() - 2].trim();
        } else if num_str.to_uppercase().ends_with("KB") {
            multiplier = 1024;
            clean_num = num_str[..num_str.len() - 2].trim();
        } else if num_str.to_uppercase().ends_with('B') {
            multiplier = 1;
            clean_num = num_str[..num_str.len() - 1].trim();
        }

        let Ok(val) = clean_num.parse::<f64>() else {
            return (None, None);
        };

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let bytes = (val * multiplier as f64) as u64;

        match op {
            ">" => (Some(bytes + 1), None),
            "<" => (None, Some(bytes.saturating_sub(1))),
            "<=" => (None, Some(bytes)),
            _ => (Some(bytes), None),
        }
    }

    fn get_min_modified(&self) -> Option<u64> {
        match self.date_filter {
            DateFilter::Anytime => None,
            DateFilter::Today => Some(
                #[allow(clippy::cast_sign_loss)]
                {
                    jiff::Zoned::now()
                        .with()
                        .hour(0)
                        .minute(0)
                        .second(0)
                        .build()
                        .unwrap()
                        .timestamp()
                        .as_second() as u64
                },
            ),
            DateFilter::Last7Days => Some(
                #[allow(clippy::cast_sign_loss)]
                {
                    jiff::Zoned::now()
                        .checked_sub(jiff::SignedDuration::from_secs(7 * 24 * 3600))
                        .unwrap()
                        .timestamp()
                        .as_second() as u64
                },
            ),
            DateFilter::Last30Days => Some(
                #[allow(clippy::cast_sign_loss)]
                {
                    jiff::Zoned::now()
                        .checked_sub(jiff::SignedDuration::from_secs(30 * 24 * 3600))
                        .unwrap()
                        .timestamp()
                        .as_second() as u64
                },
            ),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn perform_search(&mut self) -> Task<Message> {
        let state = match &self.state {
            Some(s) => s.clone(),
            None => return Task::none(),
        };

        let mut query = self.search_query.clone();

        if self.settings.whole_word
            && !query.starts_with('"')
            && !query.ends_with('"')
            && !query.contains(':')
        {
            query = format!("\"{query}\"");
        }

        let max_results = self.settings.max_results;
        let mode = self.search_mode;

        let mut extensions: ahash::AHashSet<String> = self
            .filter_extension
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        for ext in &self.filter_extensions {
            extensions.insert(ext.clone());
        }

        let multiplier: u64 = match self.size_unit.as_str() {
            "KB" => 1024,
            "GB" => 1024 * 1024 * 1024,
            _ => 1024 * 1024,
        };

        let min_size = self
            .min_size
            .trim()
            .parse::<u64>()
            .ok()
            .map(|n| n * multiplier);
        let max_size = self
            .max_size
            .trim()
            .parse::<u64>()
            .ok()
            .map(|n| n * multiplier);

        let (mut min_size, mut max_size) = if min_size.is_none() && max_size.is_none() {
            Self::parse_size_filter(&self.filter_size)
        } else {
            (min_size, max_size)
        };

        let mut min_modified = self.get_min_modified();

        query = parse_inline_query_filters(
            &query,
            &mut min_size,
            &mut max_size,
            &mut min_modified,
            &mut extensions,
        );

        let extension: Option<Vec<String>> = if extensions.is_empty() {
            None
        } else {
            Some(extensions.into_iter().collect())
        };

        self.is_searching = true;
        self.results.clear();
        self.preview_result = None;
        self.search_id += 1;
        let current_search_id = self.search_id;
        let case_sensitive = self.settings.case_sensitive;

        Task::future(async move {
            match mode {
                SearchMode::Filename => {
                    match search_filenames_internal(query.clone(), max_results, &state).await {
                        Ok(results) => {
                            let items: Vec<FileItem> =
                                results.into_iter().map(FileItem::from).collect();
                            Message::SearchResultsReceived(current_search_id, items)
                        }
                        Err(e) => Message::SearchError(FlashError::search(&query, e)),
                    }
                }
                SearchMode::FullText => {
                    match search_query_internal(
                        SearchParams::builder()
                            .query(&query)
                            .limit(max_results)
                            .maybe_min_size(min_size)
                            .maybe_max_size(max_size)
                            .maybe_min_modified(min_modified)
                            .maybe_file_extensions(extension.as_deref())
                            .case_sensitive(case_sensitive)
                            .build(),
                        &state,
                    )
                    .await
                    {
                        Ok(results) => {
                            let items: Vec<FileItem> =
                                results.into_iter().map(FileItem::from).collect();
                            Message::SearchResultsReceived(current_search_id, items)
                        }
                        Err(e) => Message::SearchError(FlashError::search(&query, e)),
                    }
                }
            }
        })
    }

    fn save_settings(&self) -> Task<Message> {
        if let Some(state) = &self.state {
            let settings = self.settings.clone();
            let state = state.clone();
            return Task::perform(
                async move {
                    let _ = state.settings_manager.save(&settings);
                    let mut watcher = state.watcher.lock();
                    let _ = watcher.update_watch_list(&settings.index_dirs);
                },
                |()| Message::NoOp,
            );
        }
        Task::none()
    }
}

#[allow(clippy::too_many_lines)]
pub fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::TabChanged(tab) => {
            app.active_tab = tab;
            Task::none()
        }
        Message::SearchQueryChanged(q) => {
            app.search_query = q;
            Task::none()
        }
        Message::SearchSubmitted => app.perform_search(),
        Message::SearchResultsReceived(id, results) => {
            if id == app.search_id {
                app.results = results;
                app.is_searching = false;
                app.selected_index = None;
            }
            Task::none()
        }
        Message::SearchError(e) => {
            app.is_searching = false;
            app.search_error = Some(e.to_string());
            Task::none()
        }
        Message::ResultSelected(idx) => {
            app.selected_index = Some(idx);
            if app.settings.show_preview_panel {
                let item = app.results[idx].clone();
                let query = app.search_query.clone();
                if let Some(state) = &app.state {
                    let state = state.clone();
                    app.is_loading_preview = true;
                    return Task::future(async move {
                        match get_file_preview_highlighted_internal(item.path, query, &state).await
                        {
                            Ok(preview) => Message::PreviewLoaded(preview),
                            Err(e) => Message::StatusUpdate(format!("Preview error: {e}")),
                        }
                    });
                }
            }
            Task::none()
        }
        Message::PreviewLoaded(preview) => {
            app.preview_result = Some(preview);
            app.is_loading_preview = false;
            Task::none()
        }
        Message::ItemHovered(idx) => {
            app.hovered_item_index = idx;
            Task::none()
        }
        Message::OpenFile(path) => {
            let _ = opener::open(std::path::Path::new(&path));
            Task::none()
        }
        Message::OpenFolder(path) => {
            let _ = crate::commands::open_folder_internal(&path);
            Task::none()
        }
        Message::CopyPath(path) => {
            let _ = crate::commands::copy_to_clipboard_internal(&path);
            Task::none()
        }
        Message::FilterExtensionChanged(ext) => {
            app.filter_extension = ext;
            Task::none()
        }
        Message::ToggleFilterExtension(ext) => {
            if app.filter_extensions.contains(&ext) {
                app.filter_extensions.remove(&ext);
            } else {
                app.filter_extensions.insert(ext);
            }
            Task::none()
        }
        Message::MinSizeChanged(s) => {
            app.min_size = s;
            Task::none()
        }
        Message::MaxSizeChanged(s) => {
            app.max_size = s;
            Task::none()
        }
        Message::SizeUnitChanged(u) => {
            app.size_unit = u;
            Task::none()
        }
        Message::DateFilterChanged(d) => {
            app.date_filter = d;
            Task::none()
        }
        Message::SearchModeChanged(m) => {
            app.search_mode = m;
            Task::none()
        }
        Message::ToggleCaseSensitive(b) => {
            app.settings.case_sensitive = b;
            Task::none()
        }
        Message::ToggleWholeWord(b) => {
            app.settings.whole_word = b;
            Task::none()
        }
        Message::ClearFilters => {
            app.filter_extension.clear();
            app.filter_extensions.clear();
            app.min_size.clear();
            app.max_size.clear();
            app.date_filter = DateFilter::Anytime;
            Task::none()
        }
        Message::MaxResultsChanged(s) => {
            if let Ok(n) = s.parse::<usize>() {
                app.settings.max_results = n;
            }
            Task::none()
        }
        Message::ExcludePatternsChanged(s) => {
            app.settings.exclude_patterns = s
                .split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();
            Task::none()
        }
        Message::CustomExtensionsChanged(s) => {
            app.settings.custom_extensions = s;
            Task::none()
        }
        Message::GlobalHotkeyChanged(s) => {
            app.settings.global_hotkey = s;
            Task::none()
        }
        Message::AddFolder => Task::done(Message::PickFolder),
        Message::ToggleMinimizeToTray(b) => {
            app.settings.minimize_to_tray = b;
            Task::none()
        }
        Message::ToggleAutoStart(b) => {
            app.settings.auto_start_on_boot = b;
            Task::none()
        }
        Message::ToggleContextMenu(b) => {
            app.settings.context_menu_enabled = b;
            Task::none()
        }
        Message::ToggleGitignore(b) => {
            app.settings.use_gitignore = b;
            Task::none()
        }
        Message::ToggleTheme => {
            app.is_dark = !app.is_dark;
            app.settings.theme = if app.is_dark {
                crate::settings::Theme::Dark
            } else {
                crate::settings::Theme::Light
            };
            Task::none()
        }
        Message::RebuildIndex => {
            if let Some(state) = &app.state {
                let state = state.clone();
                let index_dirs = app.settings.index_dirs.clone();
                app.rebuild_progress = Some(0.0);
                app.rebuild_status = Some("Rebuilding index...".to_string());
                return Task::future(async move {
                    if let Err(e) = state.indexer.clear() {
                        tracing::error!("Failed to clear search index: {e}");
                    }
                    let _ = state.indexer.commit();
                    if let Err(e) = state.metadata_db.clear() {
                        tracing::error!("Failed to clear metadata DB: {e}");
                    }
                    if let Some(ref filename_index) = state.filename_index {
                        let clear_res = filename_index.clear();
                        if let Err(e) = clear_res {
                            tracing::error!("Failed to clear filename index: {e}");
                        }
                    }

                    let dirs_to_scan = if index_dirs.is_empty() {
                        crate::commands::get_home_dir_internal()
                            .ok()
                            .into_iter()
                            .collect::<Vec<String>>()
                    } else {
                        index_dirs
                    };

                    for dir in dirs_to_scan {
                        let _ = state
                            .scanner
                            .scan_directory(
                                std::path::PathBuf::from(dir),
                                vec![],
                                state.indexing_cancel.clone(),
                            )
                            .await;
                    }
                    Message::IndexRebuilt
                });
            }
            Task::none()
        }
        Message::IndexDirAdded(dir) => {
            if !dir.is_empty() && !app.settings.index_dirs.contains(&dir) {
                app.settings.index_dirs.push(dir.clone());
                app.new_index_dir.clear();
                if let Some(state) = &app.state {
                    let state = state.clone();
                    let path_clone = dir;
                    let save_task = app.save_settings();
                    let scan_task = Task::future(async move {
                        let _ = state
                            .scanner
                            .scan_directory(
                                std::path::PathBuf::from(path_clone),
                                vec![],
                                state.indexing_cancel.clone(),
                            )
                            .await;
                        Message::IndexRebuilt
                    });
                    return Task::batch(vec![save_task, scan_task]);
                }
            }
            Task::none()
        }
        Message::ExcludePatternAdded(p) => {
            if !p.is_empty() && !app.settings.exclude_patterns.contains(&p) {
                app.settings.exclude_patterns.push(p);
                app.new_exclude_pattern.clear();
            }
            Task::none()
        }
        Message::SaveSettings => app.save_settings(),
        Message::ResetSettings => {
            app.settings = AppSettings::default();
            Task::none()
        }
        Message::ThemeChanged(t) => {
            app.settings.theme = t;
            Task::none()
        }
        Message::FontSizeChanged(f) => {
            app.settings.font_size = f;
            Task::none()
        }
        Message::PollProgressResult(Some(event)) => {
            match event.ptype {
                crate::scanner::ProgressType::Content => {
                    app.files_indexed = i32::try_from(event.processed).unwrap_or(i32::MAX);
                    app.rebuild_progress = if event.total > 0 {
                        Some(event.processed as f32 / event.total as f32)
                    } else {
                        None
                    };
                    app.rebuild_status = Some(event.status);
                    app.rebuild_eta = if event.eta_seconds > 0 {
                        Some(event.eta_seconds)
                    } else {
                        None
                    };
                }
                crate::scanner::ProgressType::Filename => {
                    app.rebuild_status = Some(event.status);
                }
            }
            Task::none()
        }
        Message::IndexRebuilt => {
            let stats = app
                .state
                .as_ref()
                .map(|s| s.indexer.get_statistics().unwrap_or_default())
                .unwrap_or_default();
            app.files_indexed = i32::try_from(stats.total_documents).unwrap_or(i32::MAX);
            app.index_size = format!("{:.1} MB", (stats.total_size_bytes as f64) / 1_048_576.0);
            app.rebuild_progress = None;
            app.rebuild_status = None;
            app.rebuild_eta = None;
            Task::none()
        }
        Message::StatusUpdate(s) => {
            app.rebuild_status = Some(s);
            Task::none()
        }
        Message::WindowIdCaptured(id) => {
            if app.window_id.is_none() {
                app.window_id = Some(id);
            }
            Task::none()
        }
        Message::WindowUnfocused(id) => iced::window::minimize(id, true),
        Message::DismissError => {
            app.error = None;
            app.search_error = None;
            app.db_corrupted_dismissed = true;
            Task::none()
        }
        Message::Quit => app.window_id.map_or_else(Task::none, iced::window::close),
        Message::PickFolder => Task::future(async move {
            let handle = rfd::AsyncFileDialog::new()
                .set_title("Select Folder to Index")
                .pick_folder()
                .await;
            Message::FolderPicked(handle.map(|h| h.path().to_string_lossy().to_string()))
        }),
        Message::FolderPicked(Some(path)) => {
            if !app.settings.index_dirs.contains(&path) {
                app.settings.index_dirs.push(path.clone());
                if let Some(state) = &app.state {
                    let state = state.clone();
                    let path_clone = path;
                    let save_task = app.save_settings();
                    let scan_task = Task::future(async move {
                        let _ = state
                            .scanner
                            .scan_directory(
                                std::path::PathBuf::from(path_clone),
                                vec![],
                                state.indexing_cancel.clone(),
                            )
                            .await;
                        Message::IndexRebuilt
                    });
                    return Task::batch(vec![save_task, scan_task]);
                }
            }
            Task::none()
        }
        Message::ToggleSidebar => {
            app.sidebar_collapsed = !app.sidebar_collapsed;
            Task::none()
        }
        Message::RemoveFolder(i) | Message::RemoveIndexDir(i) => {
            if i < app.settings.index_dirs.len() {
                let removed_dir = app.settings.index_dirs.remove(i);
                if let Some(state) = &app.state {
                    let state = state.clone();
                    let save_task = app.save_settings();

                    let cleanup_task = Task::future(async move {
                        if let Ok(all_paths) = state.metadata_db.get_all_file_paths() {
                            let mut removed_any = false;
                            for file_path in all_paths {
                                let is_under = if file_path.starts_with(&removed_dir) {
                                    let remaining = &file_path[removed_dir.len()..];
                                    remaining.is_empty()
                                        || remaining.starts_with('\\')
                                        || remaining.starts_with('/')
                                } else {
                                    false
                                };
                                if is_under {
                                    let _ = state.indexer.remove_document(&file_path);
                                    let _ = state
                                        .metadata_db
                                        .remove_file(std::path::Path::new(&file_path));
                                    removed_any = true;
                                }
                            }
                            if removed_any {
                                let _ = state.indexer.commit();
                                state.indexer.invalidate_cache();
                            }
                        }
                        Message::IndexRebuilt
                    });

                    return Task::batch(vec![save_task, cleanup_task]);
                }
            }
            Task::none()
        }
        Message::RemoveExcludePattern(i) => {
            if i < app.settings.exclude_patterns.len() {
                app.settings.exclude_patterns.remove(i);
            }
            Task::none()
        }
        _ => Task::none(),
    }
}

pub fn view(app: &App) -> Element<'_, Message> {
    match app.active_tab {
        Tab::Search => search::search_view(app),
        Tab::Settings => settings::settings_view(app),
    }
}

pub fn subscription(app: &App) -> Subscription<Message> {
    let progress_sub = app
        .progress_rx
        .as_ref()
        .map_or_else(Subscription::none, |rx| {
            Subscription::run_with(SubscriptionData { rx: rx.clone() }, |data| {
                let rx = data.rx.clone();
                iced::stream::channel(
                    100,
                    move |mut output: iced::futures::channel::mpsc::Sender<Message>| {
                        let rx = rx.clone();
                        async move {
                            loop {
                                match rx.try_recv() {
                                    Ok(event) => {
                                        let _ = output
                                            .send(Message::PollProgressResult(Some(event)))
                                            .await;
                                    }
                                    Err(flume::TryRecvError::Empty) => {
                                        tokio::time::sleep(std::time::Duration::from_millis(100))
                                            .await;
                                    }
                                    Err(flume::TryRecvError::Disconnected) => break,
                                }
                            }
                        }
                    },
                )
            })
        });

    let event_sub = iced::window::events().map(|(id, event)| match event {
        iced::window::Event::Unfocused => Message::WindowUnfocused(id),
        iced::window::Event::Opened { .. } | iced::window::Event::Focused => {
            Message::WindowIdCaptured(id)
        }
        _ => Message::NoOp,
    });

    Subscription::batch(vec![progress_sub, event_sub])
}

pub const fn app_theme(app: &App) -> iced::Theme {
    if app.is_dark {
        iced::Theme::Dark
    } else {
        iced::Theme::Light
    }
}

pub fn app_title(app: &App) -> String {
    app.rebuild_status.as_ref().map_or_else(
        || "Flash Search".to_string(),
        |status| format!("Flash Search - {status}"),
    )
}

/// # Panics
///
/// Panics if the application fails to run.
pub fn run_ui(
    state: &Result<std::sync::Arc<AppState>, String>,
    progress_rx: flume::Receiver<ProgressEvent>,
    initial_dir: Option<String>,
) {
    let state_clone = state.clone();
    let progress_rx = Arc::new(Mutex::new(Some(progress_rx)));
    let initial_dir_clone = initial_dir;
    if let Err(e) = iced::application(
        move || {
            let rx = progress_rx.lock().take();
            let app = App::new(state_clone.clone(), rx, initial_dir_clone.clone());
            let task = if app.settings.auto_index_on_startup {
                Task::done(Message::RebuildIndex)
            } else {
                Task::none()
            };
            (app, task)
        },
        update,
        view,
    )
    .title(app_title)
    .theme(app_theme)
    .subscription(subscription)
    .run()
    {
        tracing::error!("Iced application failed to run: {e}");
        panic!("Iced application failed to run: {e}");
    }
}

fn parse_inline_query_filters(
    query_str: &str,
    min_size: &mut Option<u64>,
    max_size: &mut Option<u64>,
    min_modified: &mut Option<u64>,
    extensions: &mut ahash::AHashSet<String>,
) -> String {
    let mut clean_words = Vec::new();

    for word in query_str.split_whitespace() {
        if let Some(ext) = word.strip_prefix("ext:") {
            extensions.insert(ext.to_lowercase());
        } else if let Some(size_filter) = word.strip_prefix("size:") {
            if let Some(stripped) = size_filter.strip_prefix('>') {
                if let Some(parsed) = parse_size_val(stripped) {
                    *min_size = Some(parsed);
                }
            } else if let Some(stripped) = size_filter.strip_prefix('<') {
                if let Some(parsed) = parse_size_val(stripped) {
                    *max_size = Some(parsed);
                }
            } else if let Some(parsed) = parse_size_val(size_filter) {
                *min_size = Some(parsed);
            }
        } else if let Some(mod_filter) = word.strip_prefix("modified:") {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let one_day = 86400;

            match mod_filter.to_lowercase().as_str() {
                "today" => {
                    *min_modified = Some(now.saturating_sub(one_day));
                }
                "yesterday" => {
                    *min_modified = Some(now.saturating_sub(one_day * 2));
                }
                "week" | "last-week" => {
                    *min_modified = Some(now.saturating_sub(one_day * 7));
                }
                "month" | "last-month" => {
                    *min_modified = Some(now.saturating_sub(one_day * 30));
                }
                "year" | "last-year" => {
                    *min_modified = Some(now.saturating_sub(one_day * 365));
                }
                _ => {}
            }
        } else {
            clean_words.push(word);
        }
    }

    clean_words.join(" ")
}

fn parse_size_val(val: &str) -> Option<u64> {
    let val = val.trim();
    if val.is_empty() {
        return None;
    }

    let (num_str, multiplier) = if val.ends_with("kb") || val.ends_with("KB") {
        (&val[..val.len() - 2], 1024)
    } else if val.ends_with("mb") || val.ends_with("MB") {
        (&val[..val.len() - 2], 1024 * 1024)
    } else if val.ends_with("gb") || val.ends_with("GB") {
        (&val[..val.len() - 2], 1024 * 1024 * 1024)
    } else if val.ends_with('k') || val.ends_with('K') {
        (&val[..val.len() - 1], 1024)
    } else if val.ends_with('m') || val.ends_with('M') {
        (&val[..val.len() - 1], 1024 * 1024)
    } else if val.ends_with('g') || val.ends_with('G') {
        (&val[..val.len() - 1], 1024 * 1024 * 1024)
    } else {
        (val, 1)
    };

    num_str.parse::<u64>().ok().map(|n| n * multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(2048), "2.0 KB");
        assert_eq!(format_size(1_048_576), "1.0 MB");
    }

    #[test]
    fn test_file_item_from_search_result() {
        let sr = SearchResult::builder()
            .file_path("C:\\path\\to\\file.txt".to_string())
            .score(0.95)
            .maybe_title(Some(CompactString::from("My File")))
            .maybe_extension(Some(CompactString::from("txt")))
            .matched_terms(vec![])
            .snippets(Vec::new())
            .build();
        let fi = FileItem::from(sr);
        assert_eq!(fi.title, "My File");
        assert_eq!(fi.path, "C:\\path\\to\\file.txt");
        assert!((fi.score - 0.95).abs() < f32::EPSILON);
        assert_eq!(fi.extension.as_deref(), Some("txt"));
    }

    #[test]
    fn test_parse_size_filter() {
        let (min, max) = App::parse_size_filter("> 1MB");
        assert_eq!(min, Some(1_048_576 + 1));
        assert_eq!(max, None);

        let (min, max) = App::parse_size_filter(">= 2MB");
        assert_eq!(min, Some(2 * 1_048_576));
        assert_eq!(max, None);

        let (min, max) = App::parse_size_filter("< 10KB");
        assert_eq!(min, None);
        assert_eq!(max, Some(10 * 1024 - 1));
    }

    #[test]
    fn test_parse_size_val() {
        assert_eq!(parse_size_val("500"), Some(500));
        assert_eq!(parse_size_val("10KB"), Some(10 * 1024));
        assert_eq!(parse_size_val("5mb"), Some(5 * 1024 * 1024));
        assert_eq!(parse_size_val("2g"), Some(2 * 1024 * 1024 * 1024));
    }

    #[test]
    fn test_parse_inline_query_filters() {
        let mut min_size = None;
        let mut max_size = None;
        let mut min_modified = None;
        let mut extensions = ahash::AHashSet::new();

        let clean = parse_inline_query_filters(
            "hello world ext:pdf size:>2MB modified:today",
            &mut min_size,
            &mut max_size,
            &mut min_modified,
            &mut extensions,
        );

        assert_eq!(clean, "hello world");
        assert_eq!(min_size, Some(2 * 1024 * 1024));
        assert_eq!(max_size, None);
        assert!(min_modified.is_some());
        assert!(extensions.contains("pdf"));
    }
}
