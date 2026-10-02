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
use std::sync::atomic::{AtomicUsize, Ordering};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum DateFilter {
    #[default]
    Anytime,
    Today,
    Last7Days,
    Last30Days,
}

impl std::fmt::Display for DateFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Anytime => write!(f, "Anytime"),
            Self::Today => write!(f, "Today"),
            Self::Last7Days => write!(f, "Last 7 Days"),
            Self::Last30Days => write!(f, "Last 30 Days"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum SearchMode {
    #[default]
    FullText,
    Filename,
}

impl std::fmt::Display for SearchMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FullText => write!(f, "Full Text"),
            Self::Filename => write!(f, "Filename"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum SortBy {
    #[default]
    Relevance,
    DateModified,
    Size,
    Name,
}

impl std::fmt::Display for SortBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Relevance => write!(f, "Relevance"),
            Self::DateModified => write!(f, "Date Modified"),
            Self::Size => write!(f, "Size"),
            Self::Name => write!(f, "Name"),
        }
    }
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

/// Formats a Unix timestamp for display.
///
/// Never panics: an out-of-range timestamp renders as `Unknown` instead of
/// unwrapping. The release profile uses `panic = "abort"`, so any panic on this
/// path terminates the process with no chance to report anything.
#[must_use]
pub fn format_date(timestamp: u64) -> String {
    let Ok(secs) = i64::try_from(timestamp) else {
        return "Unknown".to_string();
    };
    jiff::Timestamp::from_second(secs).map_or_else(
        |_| "Unknown".to_string(),
        |ts| {
            ts.to_zoned(jiff::tz::TimeZone::system())
                .strftime("%Y-%m-%d %H:%M")
                .to_string()
        },
    )
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
    ToggleCategory(Vec<String>),
    MinSizeChanged(String),
    MaxSizeChanged(String),
    SizeUnitChanged(String),
    DateFilterChanged(DateFilter),
    SearchModeChanged(SearchMode),
    SortByChanged(SortBy),
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
    PreviewLoaded(usize, crate::models::PreviewResult),
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
    ToggleWindow,
    RestoreWindow,
    SelectPreviousResult,
    SelectNextResult,
    OpenSelectedResult,
    ShowSelectedInFolder,
    CopySelectedPath,
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
    pub(crate) sort_by: SortBy,
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
    pub(crate) active_search_id: Arc<AtomicUsize>,
    pub(crate) active_preview_id: Arc<AtomicUsize>,
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
            sort_by: SortBy::default(),
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
            active_search_id: Arc::new(AtomicUsize::new(0)),
            active_preview_id: Arc::new(AtomicUsize::new(0)),
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

                if settings.minimize_to_tray {
                    app.tray_icon = crate::system::tray::create_tray_icon().ok();
                }

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
                    let now = jiff::Zoned::now();
                    now.with()
                        .hour(0)
                        .minute(0)
                        .second(0)
                        .build()
                        .unwrap_or(now)
                        .timestamp()
                        .as_second() as u64
                },
            ),
            DateFilter::Last7Days => Some(
                #[allow(clippy::cast_sign_loss)]
                {
                    let now = jiff::Zoned::now();
                    now.checked_sub(jiff::SignedDuration::from_secs(7 * 24 * 3600))
                        .unwrap_or(now)
                        .timestamp()
                        .as_second() as u64
                },
            ),
            DateFilter::Last30Days => Some(
                #[allow(clippy::cast_sign_loss)]
                {
                    let now = jiff::Zoned::now();
                    now.checked_sub(jiff::SignedDuration::from_secs(30 * 24 * 3600))
                        .unwrap_or(now)
                        .timestamp()
                        .as_second() as u64
                },
            ),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn perform_search(&mut self, debounce: bool) -> Task<Message> {
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
            .map(|n| n.saturating_mul(multiplier));
        let max_size = self
            .max_size
            .trim()
            .parse::<u64>()
            .ok()
            .map(|n| n.saturating_mul(multiplier));

        let (min_size, max_size) = if min_size.is_none() && max_size.is_none() {
            Self::parse_size_filter(&self.filter_size)
        } else {
            (min_size, max_size)
        };

        let min_modified = self.get_min_modified();

        // Inline `ext:` / `path:` / `title:` / `size:` / `modified:` operators are
        // NOT stripped here. `ParsedQuery` in the indexer is the single source of
        // truth for that DSL and the searcher applies the results, so the GUI, the
        // CLI, and the IPC endpoint all interpret a query identically. This used to
        // run a second, divergent parser: it only understood a subset of operators,
        // and its `modified:today` meant "last 24 hours" rather than "since
        // midnight", so the same query returned different results depending on
        // whether it came from the window or from `--cli`.
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
        self.active_search_id
            .store(current_search_id, Ordering::Relaxed);
        let active_search_id = self.active_search_id.clone();
        let case_sensitive = self.settings.case_sensitive;

        Task::future(async move {
            if debounce {
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            }

            if active_search_id.load(Ordering::Relaxed) != current_search_id {
                return Message::NoOp;
            }

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

    pub fn sort_results(&mut self) {
        match self.sort_by {
            SortBy::Relevance => {
                self.results.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            SortBy::DateModified => {
                self.results
                    .sort_by_key(|b| std::cmp::Reverse(b.modified.unwrap_or(0)));
            }
            SortBy::Size => {
                self.results
                    .sort_by_key(|b| std::cmp::Reverse(b.size.unwrap_or(0)));
            }
            SortBy::Name => {
                self.results.sort_by_key(|a| a.title.to_lowercase());
            }
        }
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
            app.perform_search(true)
        }
        Message::SearchSubmitted => app.perform_search(false),
        Message::SearchResultsReceived(id, results) => {
            if id == app.search_id {
                app.results = results;
                app.sort_results();
                app.is_searching = false;
                app.selected_index = None;
            }
            Task::none()
        }
        Message::SortByChanged(sort) => {
            app.sort_by = sort;
            app.sort_results();
            Task::none()
        }
        Message::SearchError(e) => {
            app.is_searching = false;
            app.search_error = Some(e.to_string());
            Task::none()
        }
        Message::ResultSelected(idx) => {
            // `results` can shrink between a click being emitted and the message
            // being handled (a new search replacing the list, or results being
            // cleared). Indexing into it unchecked is a panic, and the release
            // profile aborts on panic.
            let Some(item) = app.results.get(idx).cloned() else {
                tracing::debug!("Ignoring selection of out-of-range result {idx}");
                return Task::none();
            };
            app.selected_index = Some(idx);
            if app.settings.show_preview_panel {
                let query = app.search_query.clone();
                if let Some(state) = &app.state {
                    let state = state.clone();
                    app.is_loading_preview = true;
                    let next_preview_id = app.active_preview_id.fetch_add(1, Ordering::Relaxed) + 1;
                    let active_preview_id = app.active_preview_id.clone();
                    return Task::future(async move {
                        match get_file_preview_highlighted_internal(item.path, query, &state).await
                        {
                            Ok(preview) => {
                                if active_preview_id.load(Ordering::Relaxed) == next_preview_id {
                                    Message::PreviewLoaded(next_preview_id, preview)
                                } else {
                                    Message::NoOp
                                }
                            }
                            Err(e) => {
                                if active_preview_id.load(Ordering::Relaxed) == next_preview_id {
                                    Message::StatusUpdate(format!("Preview error: {e}"))
                                } else {
                                    Message::NoOp
                                }
                            }
                        }
                    });
                }
            }
            Task::none()
        }
        Message::PreviewLoaded(id, preview) => {
            if id == app.active_preview_id.load(Ordering::Relaxed) {
                app.preview_result = Some(preview);
                app.is_loading_preview = false;
            }
            Task::none()
        }
        Message::ItemHovered(idx) => {
            app.hovered_item_index = idx;
            Task::none()
        }
        Message::OpenFile(path) => Task::perform(
            async move {
                let _ = opener::open(std::path::Path::new(&path));
            },
            |()| Message::NoOp,
        ),
        Message::OpenFolder(path) => Task::perform(
            async move {
                let _ = crate::commands::open_folder_internal(&path);
            },
            |()| Message::NoOp,
        ),
        Message::CopyPath(path) => Task::perform(
            async move {
                let _ = crate::commands::copy_to_clipboard_internal(&path);
            },
            |()| Message::NoOp,
        ),
        Message::FilterExtensionChanged(ext) => {
            app.filter_extension = ext;
            app.perform_search(true)
        }
        Message::ToggleFilterExtension(ext) => {
            if app.filter_extensions.contains(&ext) {
                app.filter_extensions.remove(&ext);
            } else {
                app.filter_extensions.insert(ext);
            }
            app.perform_search(false)
        }
        Message::ToggleCategory(exts) => {
            let all_present = exts.iter().all(|e| app.filter_extensions.contains(e));
            if all_present {
                for e in &exts {
                    app.filter_extensions.remove(e);
                }
            } else {
                for e in exts {
                    app.filter_extensions.insert(e);
                }
            }
            app.perform_search(false)
        }
        Message::MinSizeChanged(s) => {
            app.min_size = s;
            app.perform_search(true)
        }
        Message::MaxSizeChanged(s) => {
            app.max_size = s;
            app.perform_search(true)
        }
        Message::SizeUnitChanged(u) => {
            app.size_unit = u;
            app.perform_search(false)
        }
        Message::DateFilterChanged(d) => {
            app.date_filter = d;
            app.perform_search(false)
        }
        Message::SearchModeChanged(m) => {
            app.search_mode = m;
            app.perform_search(false)
        }
        Message::ToggleCaseSensitive(b) => {
            app.settings.case_sensitive = b;
            app.perform_search(false)
        }
        Message::ToggleWholeWord(b) => {
            app.settings.whole_word = b;
            app.perform_search(false)
        }
        Message::ClearFilters => {
            app.filter_extension.clear();
            app.filter_extensions.clear();
            app.min_size.clear();
            app.max_size.clear();
            app.date_filter = DateFilter::Anytime;
            app.perform_search(false)
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
            if b {
                if app.tray_icon.is_none() {
                    app.tray_icon = crate::system::tray::create_tray_icon().ok();
                }
            } else {
                app.tray_icon = None;
            }
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
                    // Stop any in-flight run *before* clearing, otherwise its
                    // writer keeps flushing into the index we are about to drop.
                    state.cancel_indexing();

                    if let Err(e) = state.indexer.clear() {
                        tracing::error!("Failed to clear search index: {e}");
                    }
                    let _ = state.indexer.commit();
                    if let Err(e) = state.metadata_db.clear() {
                        tracing::error!("Failed to clear metadata DB: {e}");
                    }
                    if let Some(ref filename_index) = state.filename_index
                        && let Err(e) = filename_index.clear()
                    {
                        tracing::error!("Failed to clear filename index: {e}");
                    }

                    let dirs_to_scan = if index_dirs.is_empty() {
                        crate::commands::get_home_dir_internal()
                            .ok()
                            .into_iter()
                            .collect::<Vec<String>>()
                    } else {
                        index_dirs
                    };

                    // Route through `start_indexing` so the run is cancellable,
                    // tracked, and honours the user's exclude patterns. Scanning
                    // the directories sequentially keeps progress reporting
                    // meaningful instead of interleaving several scans.
                    for dir in dirs_to_scan {
                        if let Err(e) = state
                            .start_indexing(std::path::PathBuf::from(&dir))
                            .await
                        {
                            tracing::error!("Failed to start indexing {dir}: {e}");
                        }
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
                        if let Err(e) =
                            state.start_indexing(std::path::PathBuf::from(path_clone)).await
                        {
                            tracing::error!("Failed to start indexing: {e}");
                        }
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
                // Persist immediately: an exclude rule that is not saved is lost
                // on restart, and `save_settings_internal` also pushes the new
                // globs to the live watcher.
                app.save_settings()
            } else {
                Task::none()
            }
        }
        Message::SaveSettings => app.save_settings(),
        Message::ResetSettings => {
            app.settings = AppSettings::default();
            // Persist and reconfigure the live watcher; previously this only
            // mutated in-memory state and was lost on restart.
            app.save_settings()
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
        Message::ToggleWindow | Message::RestoreWindow => app
            .window_id
            .map_or_else(Task::none, |id| iced::window::minimize(id, false)),
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
                        if let Err(e) =
                            state.start_indexing(std::path::PathBuf::from(path_clone)).await
                        {
                            tracing::error!("Failed to start indexing: {e}");
                        }
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
                        app_purge_directory(&state, &removed_dir).await
                    });

                    return Task::batch(vec![save_task, cleanup_task]);
                }
            }
            Task::none()
        }
        Message::RemoveExcludePattern(i) => {
            if i < app.settings.exclude_patterns.len() {
                app.settings.exclude_patterns.remove(i);
                app.save_settings()
            } else {
                Task::none()
            }
        }
        Message::ExportResults(format) => {
            let results: Vec<crate::indexer::searcher::SearchResult> = app
                .results
                .iter()
                .map(|item| crate::indexer::searcher::SearchResult {
                    file_path: item.path.clone(),
                    score: item.score,
                    title: Some(compact_str::CompactString::from(item.title.clone())),
                    extension: item.extension.clone(),
                    modified: item.modified,
                    size: item.size,
                    matched_terms: Vec::new(),
                    snippets: item.snippets.clone(),
                })
                .collect();
            Task::future(async move {
                match crate::commands::export_results_internal(results, format).await {
                    Ok(()) => Message::StatusUpdate("Results exported successfully".to_string()),
                    Err(e) => Message::StatusUpdate(format!("Export failed: {e}")),
                }
            })
        }
        Message::SelectPreviousResult => {
            if !app.results.is_empty() {
                let next_idx = match app.selected_index {
                    Some(idx) => {
                        if idx == 0 {
                            app.results.len() - 1
                        } else {
                            idx - 1
                        }
                    }
                    None => 0,
                };
                return Task::done(Message::ResultSelected(next_idx));
            }
            Task::none()
        }
        Message::SelectNextResult => {
            if !app.results.is_empty() {
                let next_idx = match app.selected_index {
                    Some(idx) => {
                        if idx == app.results.len() - 1 {
                            0
                        } else {
                            idx + 1
                        }
                    }
                    None => 0,
                };
                return Task::done(Message::ResultSelected(next_idx));
            }
            Task::none()
        }
        Message::OpenSelectedResult => {
            if let Some(idx) = app.selected_index
                && idx < app.results.len()
            {
                let path = app.results[idx].path.clone();
                return Task::done(Message::OpenFile(path));
            }
            Task::none()
        }
        Message::ShowSelectedInFolder => {
            if let Some(idx) = app.selected_index
                && idx < app.results.len()
            {
                let path = app.results[idx].path.clone();
                return Task::done(Message::OpenFolder(path));
            }
            Task::none()
        }
        Message::CopySelectedPath => {
            if let Some(idx) = app.selected_index
                && idx < app.results.len()
            {
                let path = app.results[idx].path.clone();
                return Task::done(Message::CopyPath(path));
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

#[allow(clippy::too_many_lines)]
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
                            while let Ok(event) = rx.recv_async().await {
                                let _ = output.send(Message::PollProgressResult(Some(event))).await;
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

    let hotkey_str = app.settings.global_hotkey.clone();
    let minimize_to_tray = app.settings.minimize_to_tray;
    let system_sub = Subscription::run_with(
        SystemSubscriptionData {
            hotkey_str,
            minimize_to_tray,
        },
        |data| {
            let hotkey_str = data.hotkey_str.clone();
            iced::stream::channel(
                10,
                move |mut output: iced::futures::channel::mpsc::Sender<Message>| {
                    let hotkey_str = hotkey_str.clone();
                    async move {
                        let (tx, mut rx) = tokio::sync::mpsc::channel(10);

                        // The poller exits on shutdown and drops its sender, which
                        // closes `rx` and ends this stream. Previously it looped
                        // forever with no exit condition: changing the hotkey in
                        // Settings spawned a fresh thread each time and the old
                        // ones kept burning a core and re-registering the hotkey.
                        let shutdown = std::thread::spawn(move || {
                            let manager = global_hotkey::GlobalHotKeyManager::new().ok();
                            let registered_hotkey = manager.as_ref().and_then(|m| {
                                let hk = parse_hotkey(&hotkey_str)?;
                                m.register(hk).is_ok().then_some(hk)
                            });

                            loop {
                                if crate::is_shutting_down() {
                                    break;
                                }

                                if let Some(hk) = registered_hotkey
                                    && let Ok(event) =
                                        global_hotkey::GlobalHotKeyEvent::receiver().try_recv()
                                    && event.id == hk.id()
                                    && event.state == global_hotkey::HotKeyState::Released
                                    && tx.blocking_send(Message::ToggleWindow).is_err()
                                {
                                    break;
                                }

                                if let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv()
                                {
                                    let msg = match event.id.0.as_str() {
                                        "show" => Some(Message::RestoreWindow),
                                        "quit" => Some(Message::Quit),
                                        _ => None,
                                    };
                                    if let Some(msg) = msg
                                        && tx.blocking_send(msg).is_err()
                                    {
                                        break;
                                    }
                                }

                                if let Ok(tray_icon::TrayIconEvent::Click {
                                    button: tray_icon::MouseButton::Left,
                                    ..
                                }) = tray_icon::TrayIconEvent::receiver().try_recv()
                                    && tx.blocking_send(Message::ToggleWindow).is_err()
                                {
                                    break;
                                }

                                std::thread::sleep(std::time::Duration::from_millis(50));
                            }

                            // Unregister so the hotkey is released immediately
                            // rather than lingering until process exit.
                            if let (Some(manager), Some(hk)) = (manager, registered_hotkey) {
                                let _ = manager.unregister(hk);
                            }
                        });

                        while let Some(msg) = rx.recv().await {
                            let _ = output.send(msg).await;
                        }

                        let _ = shutdown.join();
                    }
                },
            )
        },
    );

    let keyboard_sub = iced::event::listen().map(|event| match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => {
            match key {
                iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp) => {
                    Message::SelectPreviousResult
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown) => {
                    Message::SelectNextResult
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) => {
                    if modifiers.control() {
                        Message::ShowSelectedInFolder
                    } else {
                        Message::OpenSelectedResult
                    }
                }
                iced::keyboard::Key::Character(ref c)
                    if c.eq_ignore_ascii_case("c") && modifiers.control() =>
                {
                    Message::CopySelectedPath
                }
                _ => Message::NoOp,
            }
        }
        _ => Message::NoOp,
    });

    Subscription::batch(vec![progress_sub, event_sub, system_sub, keyboard_sub])
}

/// Removes every indexed file under `dir` from both the search index and the
/// metadata database.
///
/// The previous implementation materialised *every* stored path as a `String`,
/// scanned the whole list, and then issued one `delete_term` plus one redb write
/// transaction per match. Removing a 50 000-file directory meant 50 000
/// transaction commits. This version does a single prefixed lookup, one batched
/// delete, and one transaction, all on a blocking thread.
async fn app_purge_directory(
    state: &std::sync::Arc<crate::commands::AppState>,
    dir: &str,
) -> Message {
    let dir = std::path::PathBuf::from(dir);
    let indexer = state.indexer.clone();
    let metadata_db = state.metadata_db.clone();

    let result = tokio::task::spawn_blocking(move || -> crate::error::Result<usize> {
        let paths = metadata_db.paths_under(&dir)?;
        if paths.is_empty() {
            return Ok(0);
        }

        let borrowed: Vec<&std::path::Path> =
            paths.iter().map(std::path::Path::new).collect();
        metadata_db.remove_files(&borrowed)?;
        indexer.remove_documents_batch(&paths)?;
        indexer.commit()?;
        Ok(paths.len())
    })
    .await;

    match result {
        Ok(Ok(count)) => {
            tracing::info!("Removed {count} files from the index after dropping a directory");
            state.indexer.invalidate_cache();
        }
        Ok(Err(e)) => tracing::error!("Failed to purge removed directory from index: {e}"),
        Err(e) => tracing::error!("Directory purge task panicked: {e}"),
    }

    Message::IndexRebuilt
}

pub const fn app_theme(app: &App) -> iced::Theme {
    match app.settings.theme {
        crate::settings::Theme::Dark => iced::Theme::Dark,
        crate::settings::Theme::Light => iced::Theme::Light,
        crate::settings::Theme::Auto => {
            if app.is_dark {
                iced::Theme::Dark
            } else {
                iced::Theme::Light
            }
        }
    }
}

pub fn app_title(app: &App) -> String {
    app.rebuild_status.as_ref().map_or_else(
        || "Flash Search".to_string(),
        |status| format!("Flash Search - {status}"),
    )
}

/// Runs the Iced application.
///
/// # Errors
///
/// Returns a `FlashError` if the window fails to run. This used to `panic!`,
/// which under the release profile's `panic = "abort"` meant an immediate process
/// kill with no unwinding and no chance to persist the index or write a crash
/// note.
pub fn run_ui(
    state: &Result<std::sync::Arc<AppState>, String>,
    progress_rx: flume::Receiver<ProgressEvent>,
    initial_dir: Option<String>,
) -> crate::error::Result<()> {
    let state_clone = state.clone();
    let progress_rx = Arc::new(Mutex::new(Some(progress_rx)));
    let initial_dir_clone = initial_dir;
    iced::application(
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
    .font(icons::FONT_BYTES)
    .run()
    .map_err(|e| crate::error::FlashError::config("run_ui", format!("Iced failed to run: {e}")))
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
    fn test_format_date_handles_extreme_timestamps() {
        // Must never panic: this runs on the render path and the release profile
        // uses `panic = "abort"`. The exact string depends on the local zone, so
        // only the overflow behaviour is asserted.
        assert_eq!(format_date(u64::MAX), "Unknown");
        let rendered = format_date(0);
        assert!(rendered.starts_with("1970-01-01"), "got {rendered}");
    }

    #[test]
    fn test_size_and_date_filters_parse_from_query() {
        // The UI no longer strips inline operators itself; `ParsedQuery` in the
        // indexer is the single parser. Verify it agrees with what the sidebar
        // produces so the two input paths cannot drift.
        use crate::indexer::query_parser::ParsedQuery;

        let parsed = ParsedQuery::new("hello world ext:pdf size:>2MB", false);
        assert_eq!(parsed.text_query, "hello world");
        assert_eq!(parsed.extensions, vec!["pdf".to_string()]);
        assert_eq!(parsed.min_size, Some(2 * 1024 * 1024));
        assert_eq!(parsed.max_size, None);

        // The sidebar's "> 1MB" adds 1 because Tantivy ranges are inclusive; the
        // inline operator has the same effect. Both therefore exclude exactly
        // 1 MiB, so they must agree.
        let (min, max) = App::parse_size_filter("> 1MB");
        assert_eq!(min, Some(1_048_576 + 1));
        let parsed = ParsedQuery::new("x size:>1MB", false);
        assert_eq!(parsed.min_size.map(|v| v + 1), min);
        assert_eq!(parsed.max_size, max);
    }

    #[test]
    fn test_repeated_ext_operators_are_or_ed() {
        use crate::indexer::query_parser::ParsedQuery;

        let parsed = ParsedQuery::new("report ext:pdf ext:docx", false);
        assert_eq!(parsed.text_query, "report");
        assert_eq!(parsed.extensions, vec!["pdf".to_string(), "docx".to_string()]);
    }

    #[test]
    fn test_operator_text_is_not_eaten_from_free_text() {
        use crate::indexer::query_parser::ParsedQuery;

        // Regression: the old implementation removed operators with
        // `String::replace`, which also stripped any identical free-text word.
        let parsed = ParsedQuery::new("ext:pdf ext:pdf notes", false);
        assert_eq!(parsed.text_query, "notes");

        let parsed = ParsedQuery::new("path:docs path:docs notes", false);
        assert_eq!(parsed.text_query, "notes");
    }

    #[test]
    fn test_contradictory_size_bounds_return_nothing_rather_than_everything() {
        use crate::indexer::query_parser::ParsedQuery;

        let parsed = ParsedQuery::new("x size:>10MB size:<1MB", false);
        assert_eq!(parsed.min_size, Some(10_485_760));
        assert_eq!(parsed.max_size, Some(1_048_576));
    }
}

#[derive(Debug, Clone)]
struct SystemSubscriptionData {
    hotkey_str: String,
    minimize_to_tray: bool,
}

impl std::hash::Hash for SystemSubscriptionData {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hotkey_str.hash(state);
        self.minimize_to_tray.hash(state);
    }
}

impl PartialEq for SystemSubscriptionData {
    fn eq(&self, other: &Self) -> bool {
        self.hotkey_str == other.hotkey_str && self.minimize_to_tray == other.minimize_to_tray
    }
}

impl Eq for SystemSubscriptionData {}

fn parse_hotkey(s: &str) -> Option<global_hotkey::hotkey::HotKey> {
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};
    let parts: Vec<&str> = s.split('+').map(str::trim).collect();
    if parts.is_empty() {
        return None;
    }
    let mut modifiers = Modifiers::empty();
    let mut key_code = None;

    for part in parts {
        match part.to_lowercase().as_str() {
            "alt" => modifiers.insert(Modifiers::ALT),
            "ctrl" | "control" => modifiers.insert(Modifiers::CONTROL),
            "shift" => modifiers.insert(Modifiers::SHIFT),
            "meta" | "win" | "super" | "command" => modifiers.insert(Modifiers::SUPER),
            "space" => key_code = Some(Code::Space),
            k => {
                if k.len() == 1 {
                    let c = k.chars().next().unwrap();
                    key_code = match c.to_ascii_uppercase() {
                        'A' => Some(Code::KeyA),
                        'B' => Some(Code::KeyB),
                        'C' => Some(Code::KeyC),
                        'D' => Some(Code::KeyD),
                        'E' => Some(Code::KeyE),
                        'F' => Some(Code::KeyF),
                        'G' => Some(Code::KeyG),
                        'H' => Some(Code::KeyH),
                        'I' => Some(Code::KeyI),
                        'J' => Some(Code::KeyJ),
                        'K' => Some(Code::KeyK),
                        'L' => Some(Code::KeyL),
                        'M' => Some(Code::KeyM),
                        'N' => Some(Code::KeyN),
                        'O' => Some(Code::KeyO),
                        'P' => Some(Code::KeyP),
                        'Q' => Some(Code::KeyQ),
                        'R' => Some(Code::KeyR),
                        'S' => Some(Code::KeyS),
                        'T' => Some(Code::KeyT),
                        'U' => Some(Code::KeyU),
                        'V' => Some(Code::KeyV),
                        'W' => Some(Code::KeyW),
                        'X' => Some(Code::KeyX),
                        'Y' => Some(Code::KeyY),
                        'Z' => Some(Code::KeyZ),
                        '0' => Some(Code::Digit0),
                        '1' => Some(Code::Digit1),
                        '2' => Some(Code::Digit2),
                        '3' => Some(Code::Digit3),
                        '4' => Some(Code::Digit4),
                        '5' => Some(Code::Digit5),
                        '6' => Some(Code::Digit6),
                        '7' => Some(Code::Digit7),
                        '8' => Some(Code::Digit8),
                        '9' => Some(Code::Digit9),
                        _ => None,
                    };
                }
            }
        }
    }

    key_code.map(|code| HotKey::new(Some(modifiers), code))
}
