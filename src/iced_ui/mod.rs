use crate::commands::AppState;
use crate::commands::{search_query_internal, search_filenames_internal, get_file_preview_highlighted_internal};
use crate::error::FlashError;
use crate::indexer::searcher::SearchResult;
use crate::models::FilenameSearchResult;
use crate::scanner::ProgressEvent;
use crate::settings::AppSettings;
use iced::{Element, Settings, Theme, Task};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

pub mod search;
pub mod settings;

#[derive(Clone, Debug)]
pub struct FileItem {
    pub title: String,
    pub path: String,
    pub score: f32,
    pub extension: Option<String>,
}

impl From<SearchResult> for FileItem {
    fn from(r: SearchResult) -> Self {
        let ext = r.file_path.split('.').last().map(String::from);
        FileItem {
            title: r.file_path.split(['\\', '/']).last().unwrap_or("Unknown").to_string(),
            path: r.file_path,
            score: r.score,
            extension: ext,
        }
    }
}

impl From<FilenameSearchResult> for FileItem {
    fn from(r: FilenameSearchResult) -> Self {
        let ext = r.file_name.split('.').last().map(String::from);
        FileItem {
            title: r.file_name,
            path: r.file_path,
            score: 1.0,
            extension: ext,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Tab { Search, Settings }

#[derive(Clone, Debug, PartialEq)]
pub enum SearchMode { FullText, Filename }

#[derive(Clone, Debug)]
pub enum Message {
    SearchQueryChanged(String),
    SearchSubmitted,
    SearchResultsReceived(Vec<FileItem>),
    SearchError(String),
    ResultSelected(usize),
    OpenFile(String),
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
    FilterExtensionChanged(String),
    FilterSizeChanged(String),
    PreviewRequested(usize),
    PreviewLoaded(Option<String>),
    MoveUp,
    MoveDown,
    DismissError,
    Quit,
    MaxResultsChanged(String),
    ExcludePatternsChanged(String),
    PollProgressResult(Option<ProgressEvent>),
    StartPollingProgress,
}

pub struct App {
    state: Option<Arc<AppState>>,
    error: Option<String>,
    search_error: Option<String>,
    search_query: String,
    results: Vec<FileItem>,
    selected_index: Option<usize>,
    is_searching: bool,
    settings: AppSettings,
    active_tab: Tab,
    files_indexed: i32,
    index_size: String,
    is_dark: bool,
    search_mode: SearchMode,
    filter_extension: String,
    filter_size: String,
    preview_content: Option<String>,
    is_loading_preview: bool,
    rebuild_progress: Option<f32>,
    rebuild_status: Option<String>,
    progress_rx: Option<Arc<tokio::sync::Mutex<mpsc::Receiver<ProgressEvent>>>>,
}

impl App {
    fn new(state: Result<Arc<AppState>, String>) -> Self {
        match state {
            Ok(state) => {
                let settings = state.settings_manager.load().unwrap_or_default();
                let stats = state.indexer.get_statistics().unwrap_or_default();
                let index_size = format!("{:.1} MB", (stats.total_size_bytes as f64) / 1_048_576.0);
                let is_dark = matches!(settings.theme, crate::settings::Theme::Dark);
                App { 
                    state: Some(state), error: None, search_error: None, search_query: String::new(), results: Vec::new(), selected_index: None, 
                    is_searching: false, settings, active_tab: Tab::Search,
                    files_indexed: stats.total_documents as i32, index_size, is_dark,
                    search_mode: SearchMode::FullText, filter_extension: String::new(),
                    filter_size: String::new(), preview_content: None, is_loading_preview: false,
                    rebuild_progress: None, rebuild_status: None, progress_rx: None,
                }
            }
            Err(err_msg) => {
                App {
                    state: None,
                    error: Some(err_msg),
                    search_error: None,
                    search_query: String::new(),
                    results: Vec::new(),
                    selected_index: None,
                    is_searching: false,
                    settings: AppSettings::default(),
                    active_tab: Tab::Search,
                    files_indexed: 0,
                    index_size: "0 MB".to_string(),
                    is_dark: false,
                    search_mode: SearchMode::FullText,
                    filter_extension: String::new(),
                    filter_size: String::new(),
                    preview_content: None,
                    is_loading_preview: false,
                    rebuild_progress: None,
                    rebuild_status: None,
                    progress_rx: None,
                }
            }
        }
    }

    fn state(&self) -> Option<&Arc<AppState>> {
        self.state.as_ref()
    }

    fn parse_size_filter(size_str: &str) -> (Option<u64>, Option<u64>) {
        let size_str = size_str.trim();
        if size_str.is_empty() {
            return (None, None);
        }
        
        let (op, num_str) = if size_str.starts_with(">=") {
            (">=", &size_str[2..])
        } else if size_str.starts_with("<=") {
            ("<=", &size_str[2..])
        } else if size_str.starts_with(">") {
            (">", &size_str[1..])
        } else if size_str.starts_with("<") {
            ("<", &size_str[1..])
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
        
        let num: u64 = num_str
            .trim_end_matches(|c: char| c.is_alphabetic())
            .parse()
            .unwrap_or(0);
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
        
        let query = self.search_query.clone();
        let max_results = self.settings.max_results;
        let mode = self.search_mode.clone();
        let extension = if self.filter_extension.is_empty() {
            None
        } else {
            Some(vec![self.filter_extension.clone()])
        };
        let (min_size, max_size) = Self::parse_size_filter(&self.filter_size);
        
        self.is_searching = true;
        self.results.clear();
        self.preview_content = None;
        
        Task::future(async move {
            let result = match mode {
                SearchMode::Filename => {
                    match search_filenames_internal(query, max_results, &state).await {
                        Ok(results) => Message::SearchResultsReceived(results.into_iter().map(FileItem::from).collect()),
                        Err(e) => Message::SearchError(e.to_string()),
                    }
                }
                SearchMode::FullText => {
                    match search_query_internal(
                        query, max_results, &state, min_size, max_size, extension
                    ).await {
                        Ok(results) => Message::SearchResultsReceived(results.into_iter().map(FileItem::from).collect()),
                        Err(e) => Message::SearchError(e.to_string()),
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
        
        Task::future(async move {
            let preview = match get_file_preview_highlighted_internal(path, query).await {
                Ok(result) => Some(result.content),
                Err(_) => None,
            };
            Message::PreviewLoaded(preview)
        })
    }

    fn save_settings(&self) { 
        if let Some(state) = &self.state {
            let _ = state.settings_manager.save(&self.settings);
        }
    }
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    if app.error.is_some() {
        match message {
            Message::DismissError => {
                app.error = None;
                Task::none()
            }
            Message::Quit => {
                std::process::exit(0);
            }
            _ => Task::none()
        }
    } else {
        match message {
            Message::SearchQueryChanged(q) => { app.search_query = q; Task::none() }
            Message::SearchSubmitted => app.perform_search(),
            Message::SearchResultsReceived(results) => { 
                app.results = results; 
                app.is_searching = false;
                app.search_error = None;
                if !app.results.is_empty() {
                    app.selected_index = Some(0);
                }
                app.load_preview()
            }
            Message::SearchError(err) => {
                app.search_error = Some(err);
                app.is_searching = false;
                app.results.clear();
                Task::none()
            }
            Message::ResultSelected(idx) => { 
                app.selected_index = Some(idx); 
                app.load_preview()
            }
            Message::OpenFile(path) => { let _ = opener::open(PathBuf::from(path)); Task::none() }
            Message::CopyPath(path) => {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(&path);
                }
                Task::none()
            }
            Message::TabChanged(tab) => { app.active_tab = tab; Task::none() }
            Message::RebuildIndex => {
                let state = match &app.state {
                    Some(s) => s.clone(),
                    None => return Task::none(),
                };
                let settings = app.settings.clone();
                app.rebuild_progress = Some(0.0);
                app.rebuild_status = Some("Starting rebuild...".to_string());
                app.files_indexed = 0;
                let rx = app.progress_rx.clone();
                Task::batch(vec![
                    Task::future(async move {
                        let _ = state.indexer.clear();
                        let _ = state.indexer.commit();
                        let _ = state.metadata_db.clear();
                        for dir in settings.index_dirs {
                            let _ = state.scanner.scan_directory(PathBuf::from(dir), settings.exclude_patterns.clone()).await;
                        }
                        Message::IndexRebuilt
                    }),
                    Task::perform(async move {
                        if let Some(r) = rx {
                            let mut g = r.lock().await;
                            g.recv().await
                        } else {
                            None
                        }
                    }, Message::PollProgressResult)
                ])
            }
            Message::PollProgressResult(Some(event)) => {
                let p = if event.total > 0 {
                    (event.processed as f32) / (event.total as f32)
                } else {
                    0.0
                };
                app.rebuild_progress = Some(p);
                app.rebuild_status = Some(event.status.clone());
                app.files_indexed = event.processed as i32;
                
                let rx = app.progress_rx.clone();
                Task::perform(async move {
                    if let Some(r) = rx {
                        let mut g = r.lock().await;
                        g.recv().await
                    } else {
                        None
                    }
                }, Message::PollProgressResult)
            }
            Message::PollProgressResult(None) => Task::none(),
            Message::RebuildProgress(_) => Task::none(), // Fallback
            Message::IndexRebuilt => {
                let stats = app.state.as_ref().map(|s| s.indexer.get_statistics().unwrap_or_default()).unwrap_or_default();
                app.files_indexed = stats.total_documents as i32;
                app.index_size = format!("{:.1} MB", (stats.total_size_bytes as f64) / 1_048_576.0);
                app.rebuild_progress = None;
                app.rebuild_status = None;
                Task::none()
            }
            Message::AddFolder => {
                Task::perform(async { rfd::AsyncFileDialog::new().pick_folder().await.map(|p| p.path().to_string_lossy().to_string()) }, Message::FolderPicked)
            }
            Message::FolderPicked(Some(f)) => {
                if !app.settings.index_dirs.iter().any(|d| d == &f) {
                    app.settings.index_dirs.push(f);
                    app.save_settings();
                }
                Task::none()
            }
            Message::FolderPicked(None) => Task::none(),
            Message::RemoveFolder(i) => { if i < app.settings.index_dirs.len() { app.settings.index_dirs.remove(i); } Task::none() }
            Message::SaveSettings => { app.save_settings(); Task::none() }
            Message::MaxResultsChanged(val) => {
                if let Ok(num) = val.parse() {
                    app.settings.max_results = num;
                }
                Task::none()
            }
            Message::ExcludePatternsChanged(val) => {
                app.settings.exclude_patterns = val.split(',').map(|s| s.trim().to_string()).collect();
                Task::none()
            }
            Message::ToggleTheme => {
                app.is_dark = !app.is_dark;
                app.settings.theme = if app.is_dark { crate::settings::Theme::Dark } else { crate::settings::Theme::Light };
                app.save_settings();
                Task::none()
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
            Message::FilterExtensionChanged(ext) => { app.filter_extension = ext; Task::none() }
            Message::FilterSizeChanged(size) => { app.filter_size = size; Task::none() }
            Message::PreviewRequested(idx) => {
                app.selected_index = Some(idx);
                app.load_preview()
            }
            Message::PreviewLoaded(content) => {
                app.preview_content = content;
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
                std::process::exit(0);
            }
            Message::StartPollingProgress => Task::none(),
        }
    }
}

fn subscription(app: &App) -> Subscription<Message> {
    if matches!(app.active_tab, Tab::Search) && !app.results.is_empty() {
        iced::keyboard::on_key_press(|key, _modifiers| match key {
            iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp) => Some(Message::MoveUp),
            iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown) => Some(Message::MoveDown),
            _ => None,
        })
    } else {
        Subscription::none()
    }
}

fn view(app: &App) -> Element<Message> {
    if let Some(ref error) = app.error {
        return error_view(error);
    }
    
    match app.active_tab {
        Tab::Search => search::search_view(app),
        Tab::Settings => settings::settings_view(app),
    }
}

fn error_view(error: &str) -> Element<Message> {
    use iced::widget::{column, button, text};
    
    column![
        text("Startup Error").size(24).style(iced::theme::Text::Color(iced::color!(1.0, 0.3, 0.3))),
        text(error).size(14),
        button("Quit").on_press(Message::Quit)
    ]
    .spacing(20)
    .padding(40)
    .into()
}

pub fn run_ui(state: Result<std::sync::Arc<AppState>, String>, progress_rx: mpsc::Receiver<ProgressEvent>) {
    let settings = Settings::default();
    let mut app = App::new(state);
    let is_dark = app.is_dark;
    app.progress_rx = Some(Arc::new(tokio::sync::Mutex::new(progress_rx)));
    
    let _ = iced::application("Flash Search", update, view)
        .subscription(subscription)
        .settings(settings)
        .theme(move |_| if is_dark { Theme::Dark } else { Theme::Light })
        .run_with(|| (app, Task::none()));
}
