use crate::commands::AppState;
use crate::commands::{
    get_file_preview_highlighted_internal, search_filenames_internal, search_query_internal,
};
use crate::error::FlashError;
use crate::indexer::searcher::SearchResult;
use crate::models::FilenameSearchResult;
use crate::scanner::ProgressEvent;
use crate::settings::AppSettings;
use compact_str::CompactString;
use iced::{Element, Subscription, Task};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

pub mod icons;
pub mod search;
pub mod settings;
pub mod theme;

#[derive(Clone, Debug)]
pub struct FileItem {
    pub title: CompactString,
    pub path: String,
    pub score: f32,
    pub extension: Option<CompactString>,
    pub modified: Option<u64>,
    pub size: Option<u64>,
    pub snippets: Vec<String>,
}

impl From<SearchResult> for FileItem {
    fn from(r: SearchResult) -> Self {
        FileItem {
            title: r.title.unwrap_or_else(|| {
                CompactString::from(
                    r.file_path
                        .split(['\\', '/'])
                        .next_back()
                        .unwrap_or("Unknown"),
                )
            }),
            path: r.file_path,
            score: r.score,
            extension: r.extension,
            modified: r.modified,
            size: r.size,
            snippets: r.snippets,
        }
    }
}

impl From<FilenameSearchResult> for FileItem {
    fn from(r: FilenameSearchResult) -> Self {
        let ext = r
            .file_name
            .split('.')
            .next_back()
            .map(CompactString::from);
        FileItem {
            title: r.file_name,
            path: r.file_path,
            score: 1.0,
            extension: ext,
            modified: None,
            size: None,
            snippets: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Tab {
    Search,
    Settings,
}

pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

pub fn format_date(timestamp: u64) -> String {
    jiff::Timestamp::from_second(timestamp as i64)
        .unwrap_or_else(|_| jiff::Timestamp::from_second(0).unwrap())
        .to_zoned(jiff::tz::TimeZone::system())
        .strftime("%Y/%m/%d")
        .to_string()
}

#[derive(Clone, Debug, PartialEq)]
pub enum SearchMode {
    FullText,
    Filename,
}

#[derive(Clone, Debug)]
pub enum Message {
    SearchQueryChanged(String),
    SearchSubmitted,
    SearchResultsReceived(u64, Vec<FileItem>),
    DebouncedSearch(u64),
    SearchError(FlashError),
    ResultSelected(usize),
    ItemHovered(Option<usize>),
    OpenFile(String),
    OpenSelected,
    CopyPath(String),
    TabChanged(Tab),
    RebuildIndex,
    IndexRebuilt,
    RebuildProgress(ProgressEvent),
    AddFolder,
    FolderPicked(Option<String>),
    RemoveFolder(usize),
    SaveSettings,
    ToggleTheme,
    ToggleSearchMode,
    ToggleCaseSensitive(bool),
    ToggleWholeWord(bool),
    FilterExtensionChanged(String),
    ToggleFilterExtension(String),
    ToggleSidebar,
    ClearFilters,
    MinSizeChanged(String),
    MaxSizeChanged(String),
    SizeUnitChanged(String),
    FilterSizeChanged(String),
    PreviewRequested(usize),
    PreviewLoaded(Option<crate::models::PreviewResult>),
    ShowContextMenu(usize),
    CloseContextMenu,
    OpenFolder(String),
    MoveUp,
    MoveDown,
    FocusSearch,
    DismissError,
    Quit,
    GlobalHotkeyChanged(String),
    StartPollingProgress,
    PollProgressResult(Option<ProgressEvent>),
    PollTray,
    ToggleMinimizeToTray(bool),
    ToggleAutoStart(bool),
    ToggleContextMenu(bool),
    PollHotkey,
    WindowIdCaptured(iced::window::Id),
    NotImplemented(String),
    MaxResultsChanged(String),
    ExcludePatternsChanged(String),
    CustomExtensionsChanged(String),
    NoOp,
}

pub struct App {
    state: Option<Arc<AppState>>,
    error: Option<String>,
    search_error: Option<String>,
    search_query: String,
    results: Vec<FileItem>,
    selected_index: Option<usize>,
    hovered_item_index: Option<usize>,
    is_searching: bool,
    sidebar_collapsed: bool,
    settings: AppSettings,
    active_tab: Tab,
    files_indexed: i32,
    index_size: String,
    is_dark: bool,
    search_mode: SearchMode,
    filter_extension: String,
    filter_extensions: ahash::AHashSet<String>,
    min_size: String,
    max_size: String,
    size_unit: String,
    filter_size: String,
    preview_result: Option<crate::models::PreviewResult>,
    is_loading_preview: bool,
    rebuild_progress: Option<f32>,
    rebuild_status: Option<String>,
    progress_rx: Option<Arc<tokio::sync::Mutex<mpsc::Receiver<ProgressEvent>>>>,
    tray_icon: Option<tray_icon::TrayIcon>,
    hotkey_manager: Option<global_hotkey::GlobalHotKeyManager>,
    pub context_menu_index: Option<usize>,
    search_id: u64,
    window_id: Option<iced::window::Id>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: None,
            error: None,
            search_error: None,
            search_query: String::new(),
            results: Vec::new(),
            selected_index: None,
            hovered_item_index: None,
            is_searching: false,
            sidebar_collapsed: false,
            settings: AppSettings::default(),
            active_tab: Tab::Search,
            files_indexed: 0,
            index_size: "0 MB".to_string(),
            is_dark: true,
            search_mode: SearchMode::FullText,
            filter_extension: String::new(),
            filter_extensions: ahash::AHashSet::default(),
            min_size: String::new(),
            max_size: String::new(),
            size_unit: "MB".to_string(),
            filter_size: String::new(),
            preview_result: None,
            is_loading_preview: false,
            rebuild_progress: None,
            rebuild_status: None,
            progress_rx: None,
            tray_icon: crate::system::tray::create_tray_icon().ok(),
            hotkey_manager: None,
            context_menu_index: None,
            search_id: 0,
            window_id: None,
        }
    }
}

use iced::widget::Id;
use std::sync::OnceLock;

pub fn get_search_input_id() -> Id {
    static ID: OnceLock<Id> = OnceLock::new();
    ID.get_or_init(Id::unique).clone()
}

impl App {
    fn new(state: Result<Arc<AppState>, String>) -> Self {
        match state {
            Ok(state) => {
                let settings = state.settings_manager.load().unwrap_or_default();
                let stats = state.indexer.get_statistics().unwrap_or_default();
                let index_size = format!("{:.1} MB", (stats.total_size_bytes as f64) / 1_048_576.0);
                let is_dark = matches!(settings.theme, crate::settings::Theme::Dark);

                let mut app = App {
                    state: Some(state),
                    settings: settings.clone(),
                    files_indexed: stats.total_documents as i32,
                    index_size,
                    is_dark,
                    ..Default::default()
                };

                if let Ok(manager) = global_hotkey::GlobalHotKeyManager::new() {
                    if let Ok(hotkey) = settings
                        .global_hotkey
                        .parse::<global_hotkey::hotkey::HotKey>()
                    {
                        if manager.register(hotkey).is_err() {
                            app.error = Some(format!(
                                "Hotkey conflict: '{}' is already registered by another application. Please choose an alternative in Settings.",
                                settings.global_hotkey
                            ));
                        }
                    } else if !settings.global_hotkey.is_empty() {
                        app.error = Some(format!(
                            "Invalid hotkey format: '{}'",
                            settings.global_hotkey
                        ));
                    }
                    app.hotkey_manager = Some(manager);
                }

                app
            }
            Err(err_msg) => App {
                error: Some(err_msg),
                ..Default::default()
            },
        }
    }

    fn parse_size_filter(size_str: &str) -> (Option<u64>, Option<u64>) {
        let size_str = size_str.trim();
        if size_str.is_empty() {
            return (None, None);
        }

        let (op, num_str) = if let Some(stripped) = size_str.strip_prefix(">=") {
            (">=", stripped)
        } else if let Some(stripped) = size_str.strip_prefix("<=") {
            ("<=", stripped)
        } else if let Some(stripped) = size_str.strip_prefix(">") {
            (">", stripped)
        } else if let Some(stripped) = size_str.strip_prefix("<") {
            ("<", stripped)
        } else {
            (">=", size_str)
        };

        let num_str = num_str.trim();
        let multiplier: u64 = if num_str.ends_with("KB") {
            1024
        } else if num_str.ends_with("MB") {
            1024 * 1024
        } else if num_str.ends_with("GB") {
            1024 * 1024 * 1024
        } else {
            1
        };

        let num: u64 = match num_str
            .trim_end_matches(|c: char| c.is_alphabetic())
            .parse()
        {
            Ok(n) => n,
            Err(_) => {
                // If we can't parse the number, we treat it as no filter.
                return (None, None);
            }
        };
        let bytes = num * multiplier;

        match op {
            ">" => (Some(bytes + 1), None),
            "<" => (None, Some(bytes.saturating_sub(1))),
            ">=" => (Some(bytes), None),
            "<=" => (None, Some(bytes)),
            _ => (Some(bytes), None),
        }
    }

    fn perform_search(&mut self) -> Task<Message> {
        let state = match &self.state {
            Some(s) => s.clone(),
            None => return Task::none(),
        };

        let mut query = self.search_query.clone();

        // Wrap in quotes if whole_word is enabled and not already wrapped
        if self.settings.whole_word
            && !query.starts_with('"')
            && !query.ends_with('"')
            && !query.contains(':')
        {
            query = format!("\"{}\"", query);
        }

        let max_results = self.settings.max_results;
        let mode = self.search_mode.clone();

        // Combine manual extension filter and checkbox filters
        let mut extensions: ahash::AHashSet<String> = self
            .filter_extension
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        for ext in &self.filter_extensions {
            extensions.insert(ext.clone());
        }

        let extension = if extensions.is_empty() {
            None
        } else {
            Some(extensions.into_iter().collect())
        };

        // Calculate size filters
        let multiplier: u64 = match self.size_unit.as_str() {
            "KB" => 1024,
            "GB" => 1024 * 1024 * 1024,
            _ => 1024 * 1024, // Default to MB
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

        // Fallback to the old filter_size string if min/max are empty
        let (min_size, max_size) = if min_size.is_none() && max_size.is_none() {
            Self::parse_size_filter(&self.filter_size)
        } else {
            (min_size, max_size)
        };

        self.is_searching = true;
        self.results.clear();
        self.preview_result = None;
        self.search_id += 1;
        let current_search_id = self.search_id;
        let case_sensitive = self.settings.case_sensitive;

        Task::future(async move {
            let result = match mode {
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
                        query.clone(),
                        max_results,
                        &state,
                        min_size,
                        max_size,
                        extension,
                        case_sensitive,
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
            };
            result
        })
    }

    fn load_preview(&mut self) -> Task<Message> {
        let idx = match self.selected_index {
            Some(i) => i,
            None => return Task::none(),
        };

        let item = match self.results.get(idx) {
            Some(i) => i.clone(),
            None => return Task::none(),
        };

        let path = item.path.clone();
        let query = self.search_query.clone();
        self.is_loading_preview = true;

        let state = match &self.state {
            Some(s) => s.clone(),
            None => return Task::none(),
        };

        Task::future(async move {
            let preview = get_file_preview_highlighted_internal(path, query, &state)
                .await
                .ok();
            Message::PreviewLoaded(preview)
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
                    let _ = watcher.update_watch_list(settings.index_dirs.clone());
                },
                |_| Message::NoOp,
            );
        }
        Task::none()
    }
}

pub fn update(app: &mut App, message: Message) -> Task<Message> {
    if app.error.is_some() {
        match message {
            Message::DismissError => {
                app.error = None;
                Task::none()
            }
            Message::Quit => {
                // Set shutdown flag to initiate graceful shutdown
                crate::SHUTDOWN_FLAG.store(true, std::sync::atomic::Ordering::SeqCst);
                Task::none()
            }
            _ => Task::none(),
        }
    } else {
        match message {
            Message::SearchQueryChanged(q) => {
                app.search_query = q;
                if !app.search_query.is_empty() {
                    app.search_id += 1;
                    let current_id = app.search_id;
                    Task::perform(
                        tokio::time::sleep(std::time::Duration::from_millis(150)),
                        move |_| Message::DebouncedSearch(current_id),
                    )
                } else {
                    app.search_id += 1;
                    app.results.clear();
                    Task::none()
                }
            }
            Message::DebouncedSearch(id) => {
                if id == app.search_id {
                    app.perform_search()
                } else {
                    Task::none()
                }
            }
            Message::SearchSubmitted => app.perform_search(),
            Message::SearchResultsReceived(id, results) => {
                if id != app.search_id {
                    return Task::none();
                }
                app.results = results;
                app.is_searching = false;
                app.search_error = None;
                if !app.results.is_empty() {
                    app.selected_index = Some(0);
                }
                app.load_preview()
            }
            Message::SearchError(err) => {
                app.search_error = Some(err.to_string());
                app.is_searching = false;
                app.results.clear();
                Task::none()
            }
            Message::ResultSelected(idx) => {
                app.context_menu_index = None;
                app.selected_index = Some(idx);
                app.load_preview()
            }
            Message::ItemHovered(idx) => {
                app.hovered_item_index = idx;
                Task::none()
            }
            Message::ToggleSidebar => {
                app.sidebar_collapsed = !app.sidebar_collapsed;
                Task::none()
            }
            Message::FocusSearch => iced::widget::operation::focus(get_search_input_id()),
            Message::OpenSelected => {
                if let Some(idx) = app.selected_index {
                    if let Some(item) = app.results.get(idx) {
                        let path = item.path.clone();
                        return Task::perform(
                            async move { path },
                            Message::OpenFile,
                        );
                    }
                }
                Task::none()
            }
            Message::OpenFile(path) => {
                app.context_menu_index = None;
                let p = PathBuf::from(&path);
                if p.is_absolute() && p.exists() {
                    // Offload the file opening to a background thread to avoid blocking the UI
                    Task::perform(
                        async move {
                            let _ = opener::open(p);
                        },
                        |_| Message::NoOp,
                    )
                } else {
                    tracing::warn!("Blocked attempt to open invalid or relative path: {}", path);
                    Task::none()
                }
            }
            Message::CopyPath(path) => {
                app.context_menu_index = None;
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(&path);
                }
                Task::none()
            }
            Message::TabChanged(tab) => {
                app.context_menu_index = None;
                app.active_tab = tab;
                Task::none()
            }
            Message::ShowContextMenu(idx) => {
                app.context_menu_index = Some(idx);
                app.selected_index = Some(idx);
                app.load_preview()
            }
            Message::CloseContextMenu => {
                app.context_menu_index = None;
                Task::none()
            }
            Message::OpenFolder(path) => {
                app.context_menu_index = None;
                let p = PathBuf::from(&path);
                if p.is_absolute() {
                    if let Some(parent) = p.parent() {
                        let parent_buf = parent.to_path_buf();
                        if parent_buf.exists() {
                            return Task::perform(
                                async move {
                                    let _ = opener::open(parent_buf);
                                },
                                |_| Message::NoOp,
                            );
                        }
                    }
                }
                tracing::warn!("Blocked attempt to open folder for invalid path: {}", path);
                Task::none()
            }
            Message::RebuildIndex => {
                let state = match &app.state {
                    Some(s) => s.clone(),
                    None => return Task::none(),
                };
                let settings = app.settings.clone();
                app.rebuild_progress = Some(0.0);
                app.rebuild_status = Some("Starting rebuild...".to_string());
                if let Some(tray) = &app.tray_icon {
                    let _ = tray.set_tooltip(Some("Flash Search - Rebuilding Index..."));
                }
                app.files_indexed = 0;
                let rx = app.progress_rx.clone();
                Task::batch(vec![
                    Task::future(async move {
                        if let Err(e) = state.indexer.clear() {
                            tracing::error!("Failed to clear indexer: {}", e);
                            return Message::SearchError(FlashError::config(
                                "rebuild",
                                e.to_string(),
                            ));
                        }
                        let _ = state.indexer.commit();
                        let _ = state.metadata_db.clear();
                        for dir in settings.index_dirs {
                            let _ = state
                                .scanner
                                .scan_directory(
                                    PathBuf::from(dir),
                                    settings.exclude_patterns.clone(),
                                )
                                .await;
                        }
                        Message::IndexRebuilt
                    }),
                    Task::perform(
                        async move {
                            if let Some(r) = rx {
                                let mut g = r.lock().await;
                                g.recv().await
                            } else {
                                None
                            }
                        },
                        Message::PollProgressResult,
                    ),
                ])
            }
            Message::PollProgressResult(Some(event)) => {
                let p = if event.total > 0 {
                    (event.processed as f32) / (event.total as f32)
                } else {
                    0.0
                };
                app.rebuild_progress = Some(p);

                let percent = p * 100.0;
                let status_msg = if event.eta_seconds > 0 {
                    let mins = event.eta_seconds / 60;
                    let secs = event.eta_seconds % 60;
                    format!(
                        "{} ({:.1}%) - ETA: {:02}:{:02}",
                        event.status, percent, mins, secs
                    )
                } else if event.total > 0 {
                    format!("{} ({:.1}%)", event.status, percent)
                } else {
                    event.status.clone()
                };

                app.rebuild_status = Some(status_msg);
                app.files_indexed = event.processed as i32;

                let rx = app.progress_rx.clone();
                Task::perform(
                    async move {
                        if let Some(r) = rx {
                            let mut g = r.lock().await;
                            g.recv().await
                        } else {
                            None
                        }
                    },
                    Message::PollProgressResult,
                )
            }
            Message::PollProgressResult(None) => Task::none(),
            Message::RebuildProgress(_) => Task::none(), // Fallback
            Message::IndexRebuilt => {
                let stats = app
                    .state
                    .as_ref()
                    .map(|s| s.indexer.get_statistics().unwrap_or_default())
                    .unwrap_or_default();
                app.files_indexed = stats.total_documents as i32;
                app.index_size = format!("{:.1} MB", (stats.total_size_bytes as f64) / 1_048_576.0);
                if let Some(tray) = &app.tray_icon {
                    let _ = tray.set_tooltip(Some("Flash Search - Ready"));
                }
                app.rebuild_progress = None;
                app.rebuild_status = None;
                Task::none()
            }
            Message::AddFolder => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .pick_folder()
                        .await
                        .map(|p| p.path().to_string_lossy().to_string())
                },
                Message::FolderPicked,
            ),
            Message::FolderPicked(Some(f)) => {
                if !app.settings.index_dirs.iter().any(|d| d == &f) {
                    app.settings.index_dirs.push(f.clone());
                    let save_task = app.save_settings();

                    // Automatically start scanning the new folder
                    let state = match &app.state {
                        Some(s) => s.clone(),
                        None => return save_task,
                    };

                    app.rebuild_progress = Some(0.0);
                    app.rebuild_status = Some("Scanning new folder...".to_string());

                    let exclude_patterns = app.settings.exclude_patterns.clone();
                    let rx = app.progress_rx.clone();

                    return Task::batch(vec![
                        save_task,
                        Task::future(async move {
                            let _ = state
                                .scanner
                                .scan_directory(PathBuf::from(f), exclude_patterns)
                                .await;
                            Message::IndexRebuilt
                        }),
                        Task::perform(
                            async move {
                                if let Some(r) = rx {
                                    let mut g = r.lock().await;
                                    g.recv().await
                                } else {
                                    None
                                }
                            },
                            Message::PollProgressResult,
                        ),
                    ]);
                }
                Task::none()
            }
            Message::FolderPicked(None) => Task::none(),
            Message::RemoveFolder(i) => {
                if i < app.settings.index_dirs.len() {
                    app.settings.index_dirs.remove(i);
                    return app.save_settings();
                }
                Task::none()
            }
            Message::SaveSettings => app.save_settings(),
            Message::MaxResultsChanged(val) => {
                if let Ok(num) = val.parse() {
                    app.settings.max_results = num;
                }
                Task::none()
            }
            Message::ExcludePatternsChanged(val) => {
                app.settings.exclude_patterns =
                    val.split(',').map(|s| s.trim().to_string()).collect();
                Task::none()
            }
            Message::CustomExtensionsChanged(val) => {
                app.settings.custom_extensions = val;
                Task::none()
            }
            Message::ToggleTheme => {
                app.is_dark = !app.is_dark;
                app.settings.theme = if app.is_dark {
                    crate::settings::Theme::Dark
                } else {
                    crate::settings::Theme::Light
                };
                app.save_settings()
            }
            Message::ToggleMinimizeToTray(enabled) => {
                app.settings.minimize_to_tray = enabled;
                app.save_settings()
            }
            Message::ToggleAutoStart(enabled) => {
                app.settings.auto_start_on_boot = enabled;
                let save_task = app.save_settings();
                let _ = crate::system::startup::set_auto_start(enabled);
                save_task
            }
            Message::ToggleContextMenu(enabled) => {
                app.settings.context_menu_enabled = enabled;
                let save_task = app.save_settings();
                let _ = crate::system::context_menu::register_context_menu(enabled);
                save_task
            }
            Message::ToggleSearchMode => {
                app.search_mode = match app.search_mode {
                    SearchMode::FullText => SearchMode::Filename,
                    SearchMode::Filename => SearchMode::FullText,
                };
                if !app.search_query.is_empty() {
                    app.perform_search()
                } else {
                    Task::none()
                }
            }
            Message::ToggleCaseSensitive(val) => {
                app.settings.case_sensitive = val;
                let save_task = app.save_settings();
                if !app.search_query.is_empty() {
                    return Task::batch(vec![save_task, app.perform_search()]);
                }
                save_task
            }
            Message::ToggleWholeWord(val) => {
                app.settings.whole_word = val;
                let save_task = app.save_settings();
                if !app.search_query.is_empty() {
                    return Task::batch(vec![save_task, app.perform_search()]);
                }
                save_task
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
            Message::ClearFilters => {
                app.filter_extensions.clear();
                app.min_size.clear();
                app.max_size.clear();
                app.filter_extension.clear();
                app.filter_size.clear();
                Task::none()
            }
            Message::MinSizeChanged(val) => {
                app.min_size = val;
                Task::none()
            }
            Message::MaxSizeChanged(val) => {
                app.max_size = val;
                Task::none()
            }
            Message::SizeUnitChanged(val) => {
                app.size_unit = val;
                Task::none()
            }
            Message::FilterSizeChanged(size) => {
                app.filter_size = size;
                Task::none()
            }
            Message::PreviewRequested(idx) => {
                app.selected_index = Some(idx);
                app.load_preview()
            }
            Message::PreviewLoaded(result) => {
                app.preview_result = result;
                app.is_loading_preview = false;
                Task::none()
            }
            Message::MoveUp => {
                if let Some(current) = app.selected_index {
                    if current > 0 {
                        app.selected_index = Some(current - 1);
                        return app.load_preview();
                    }
                }
                Task::none()
            }
            Message::MoveDown => {
                if let Some(current) = app.selected_index {
                    if !app.results.is_empty() && current < app.results.len() - 1 {
                        app.selected_index = Some(current + 1);
                        return app.load_preview();
                    }
                }
                Task::none()
            }
            Message::DismissError => Task::none(),
            Message::Quit => {
                // Set shutdown flag to initiate graceful shutdown
                crate::SHUTDOWN_FLAG.store(true, std::sync::atomic::Ordering::SeqCst);
                Task::none()
            }
            Message::GlobalHotkeyChanged(val) => {
                app.settings.global_hotkey = val;
                Task::none()
            }
            Message::StartPollingProgress => Task::none(),
            Message::WindowIdCaptured(id) => {
                if app.window_id.is_none() {
                    app.window_id = Some(id);
                }
                Task::none()
            }
            Message::PollHotkey => {
                if let Ok(event) = global_hotkey::GlobalHotKeyEvent::receiver().try_recv() {
                    tracing::info!("Global hotkey pressed: {:?}", event);
                    if let Some(id) = app.window_id {
                        return iced::window::gain_focus(id).map(|_: ()| Message::NoOp);
                    }
                }
                Task::none()
            }
            Message::PollTray => {
                if let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
                    if event.id.0 == "quit" {
                        crate::SHUTDOWN_FLAG.store(true, std::sync::atomic::Ordering::SeqCst);
                    } else if event.id.0 == "show" {
                        if let Some(id) = app.window_id {
                            return iced::window::gain_focus(id).map(|_: ()| Message::NoOp);
                        }
                    }
                }
                Task::none()
            }
            Message::NotImplemented(feature) => {
                app.error = Some(format!(
                    "Feature '{}' is not yet implemented in this version.",
                    feature
                ));
                Task::none()
            }
            Message::NoOp => Task::none(),
        }
    }
}

pub fn subscription(_app: &App) -> Subscription<Message> {
    use iced::keyboard;
    use iced::Event;

    let subs = vec![
        iced::time::every(std::time::Duration::from_millis(200)).map(|_| Message::PollHotkey),
        iced::time::every(std::time::Duration::from_millis(500)).map(|_| Message::PollTray),
        iced::event::listen_with(|event, _status, window_id| {
            if let Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event {
                match key.as_ref() {
                    keyboard::Key::Named(keyboard::key::Named::ArrowUp) => Some(Message::MoveUp),
                    keyboard::Key::Named(keyboard::key::Named::ArrowDown) => Some(Message::MoveDown),
                    keyboard::Key::Character("/") => Some(Message::FocusSearch),
                    keyboard::Key::Named(keyboard::key::Named::Enter) => Some(Message::OpenSelected),
                    _ => Some(Message::WindowIdCaptured(window_id)),
                }
            } else {
                Some(Message::WindowIdCaptured(window_id))
            }
        }),
    ];

    Subscription::batch(subs)
}

pub fn view(app: &App) -> Element<'_, Message> {
    if let Some(err) = &app.error {
        return error_view(err);
    }
    match app.active_tab {
        Tab::Search => search::search_view(app),
        Tab::Settings => settings::settings_view(app),
    }
}

#[allow(dead_code)]
fn error_view(error: &str) -> Element<'_, Message> {
    use iced::widget::{button, column, container, text, Space};
    use iced::{Alignment, Length, Padding};

    container(
        column![
            text("An Error Occurred").size(24),
            Space::new().height(Length::Fixed(16.0)),
            text(error).size(14),
            Space::new().height(Length::Fixed(24.0)),
            button(text("Dismiss"))
                .on_press(Message::DismissError)
                .padding(Padding::new(12.0))
        ]
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

pub fn app_title(_app: &App) -> String {
    String::from("Flash Search")
}

pub fn app_theme(app: &App) -> iced::Theme {
    if app.is_dark {
        iced::Theme::Dark
    } else {
        iced::Theme::Light
    }
}
fn load_app_icon() -> Option<iced::window::icon::Icon> {
    let icon_bytes = include_bytes!("../../FindAll.png");
    match image::load_from_memory(icon_bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            iced::window::icon::from_rgba(rgba.into_raw(), width, height).ok()
        }
        Err(e) => {
            tracing::warn!("Failed to load app icon FindAll.png: {}", e);
            None
        }
    }
}

pub fn run_ui(
    state: Result<std::sync::Arc<AppState>, String>,
    progress_rx: mpsc::Receiver<ProgressEvent>,
) {
    let progress_mutex = Arc::new(tokio::sync::Mutex::new(progress_rx));

    iced::application(
        move || {
            let mut app = App::new(state.clone());
            app.progress_rx = Some(progress_mutex.clone());
            (app, Task::done(Message::StartPollingProgress))
        },
        update,
        view,
    )
    .title(app_title)
    .theme(app_theme)
    .font(icons::FONT_BYTES)
    .subscription(subscription)
    .window(iced::window::Settings {
        size: iced::Size::new(1000.0, 700.0),
        position: iced::window::Position::Centered,
        icon: load_app_icon(),
        ..Default::default()
    })
    .antialiasing(false)
    .run()
    .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_item_from_search_result() {
        let sr = SearchResult::builder()
            .file_path("C:\\path\\to\\file.txt".to_string())
            .score(0.95)
            .maybe_title(Some("My File".into()))
            .maybe_extension(Some("txt".into()))
            .matched_terms(vec![])
            .snippets(Vec::new())
            .build();
        let fi = FileItem::from(sr);
        assert_eq!(fi.title, "My File");
        assert_eq!(fi.path, "C:\\path\\to\\file.txt");
        assert_eq!(fi.score, 0.95);
        assert_eq!(fi.extension.as_deref(), Some("txt"));
    }

    #[test]
    fn test_parse_size_filter() {
        let (min, max) = App::parse_size_filter(">= 2MB");
        assert_eq!(min, Some(2 * 1024 * 1024));
        assert_eq!(max, None);

        let (min, max) = App::parse_size_filter("< 10KB");
        assert_eq!(min, None);
        assert_eq!(max, Some(10 * 1024 - 1));
    }
}
