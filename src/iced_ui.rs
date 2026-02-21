use crate::commands::AppState;
use crate::indexer::searcher::SearchResult;
use crate::scanner::ProgressEvent;
use crate::settings::AppSettings;
use iced::widget::{button, column, container, horizontal_space, row, text, vertical_space, TextInput};
use iced::{Task, Length, Settings, Theme};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use std::fmt;

#[derive(Debug, Clone)]
pub struct FileItem {
    pub title: String,
    pub path: String,
    pub score: f32,
}

impl From<SearchResult> for FileItem {
    fn from(r: SearchResult) -> Self {
        FileItem {
            title: r.file_path.split(['\\', '/']).last().unwrap_or("Unknown").to_string(),
            path: r.file_path,
            score: r.score,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FilterType {
    All,
    Text,
    Image,
    Document,
    FilenameOnly,
}

impl fmt::Display for FilterType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FilterType::All => write!(f, "All Files"),
            FilterType::Text => write!(f, "Text"),
            FilterType::Image => write!(f, "Image"),
            FilterType::Document => write!(f, "Document"),
            FilterType::FilenameOnly => write!(f, "Filename Only"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Tab {
    Search,
    Settings,
}

pub struct App {
    state: Arc<AppState>,
    search_query: String,
    results: Vec<FileItem>,
    is_searching: bool,
    settings: AppSettings,
    active_tab: Tab,
    files_indexed: i32,
    index_size: String,
}

impl App {
    fn new(state: Arc<AppState>) -> Self {
        let settings = state.settings_manager.load().unwrap_or_default();
        let stats = state.indexer.get_statistics().unwrap_or_default();
        let index_size = format!("{:.1} MB", (stats.total_size_bytes as f64) / 1_048_576.0);

        App {
            state,
            search_query: String::new(),
            results: Vec::new(),
            is_searching: false,
            settings,
            active_tab: Tab::Search,
            files_indexed: stats.total_documents as i32,
            index_size,
        }
    }

    fn perform_search(&mut self) -> Task<Message> {
        let query = self.search_query.clone();
        let state = self.state.clone();
        let max_results = self.settings.max_results;
        let search_id = self.current_search_id();

        self.is_searching = true;
        self.results.clear();

        Task::future(async move {
            let results = state.indexer.search(&query, max_results, None, None, None).await.unwrap_or_default();
            let items: Vec<FileItem> = results.into_iter().map(FileItem::from).collect();
            Message::SearchResultsReceived(search_id, items)
        })
    }

    fn current_search_id(&self) -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        COUNTER.fetch_add(1, Ordering::Relaxed)
    }

    fn open_file(&self, path: &str) {
        let path_buf = PathBuf::from(path);
        let _ = opener::open(path_buf);
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    SearchQueryChanged(String),
    SearchSubmitted,
    SearchResultsReceived(u64, Vec<FileItem>),
    OpenFile(String),
    TabChanged(Tab),
    RebuildIndex,
    IndexRebuilt,
    AddFolder,
    RemoveFolder(usize),
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::SearchQueryChanged(query) => {
            app.search_query = query;
            Task::none()
        }
        Message::SearchSubmitted => app.perform_search(),
        Message::SearchResultsReceived(_, results) => {
            app.results = results;
            app.is_searching = false;
            Task::none()
        }
        Message::OpenFile(path) => {
            app.open_file(&path);
            Task::none()
        }
        Message::TabChanged(tab) => {
            app.active_tab = tab;
            Task::none()
        }
        Message::RebuildIndex => {
            let state = app.state.clone();
            let settings = app.settings.clone();
            Task::future(async move {
                let _ = state.indexer.clear();
                let _ = state.indexer.commit();
                let _ = state.metadata_db.clear();
                for dir_str in settings.index_dirs {
                    let dir = PathBuf::from(dir_str);
                    let _ = state.scanner.scan_directory(dir, settings.exclude_patterns.clone()).await;
                }
                Message::IndexRebuilt
            })
        }
        Message::IndexRebuilt => {
            let stats = app.state.indexer.get_statistics().unwrap_or_default();
            app.files_indexed = stats.total_documents as i32;
            app.index_size = format!("{:.1} MB", (stats.total_size_bytes as f64) / 1_048_576.0);
            Task::none()
        }
        Message::AddFolder => {
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                let folder_str = folder.to_string_lossy().to_string();
                if !app.settings.index_dirs.iter().any(|d| d == &folder_str) {
                    app.settings.index_dirs.push(folder_str);
                }
            }
            Task::none()
        }
        Message::RemoveFolder(index) => {
            if index < app.settings.index_dirs.len() {
                app.settings.index_dirs.remove(index);
            }
            Task::none()
        }
    }
}

fn view(app: &App) -> iced::Element<Message> {
    match app.active_tab {
        Tab::Search => search_view(app),
        Tab::Settings => settings_view(app),
    }
}

fn search_view(app: &App) -> iced::Element<Message> {
    let search_input = TextInput::new("Search files...", &app.search_query)
        .on_input(Message::SearchQueryChanged)
        .on_submit(Message::SearchSubmitted)
        .padding(8.0)
        .size(16.0);

    let search_btn = button("Search").on_press(Message::SearchSubmitted).padding(8.0);

    let stats = row([])
        .spacing(20.0)
        .push(text(format!("Files: {}", app.files_indexed)))
        .push(text(format!("Index: {}", app.index_size)));

    let rebuild_btn = button("Rebuild").on_press(Message::RebuildIndex).padding(8.0);

    let search_tab = button("Search").on_press(Message::TabChanged(Tab::Search));
    let settings_tab = button("Settings").on_press(Message::TabChanged(Tab::Settings));

    let tabs = row([search_tab.into(), settings_tab.into()]).spacing(10.0);

    let results_list = if app.is_searching {
        container(text("Searching...")).into()
    } else if app.results.is_empty() {
        container(text("No results - enter a search query")).into()
    } else {
        let items: Vec<_> = app.results.iter().take(50).map(|item| {
            let file_row = row([text(&item.title).size(14.0).into()])
                .spacing(5.0);
            button(file_row)
                .on_press(Message::OpenFile(item.path.clone()))
                .width(Length::Fill)
                .padding(5.0)
                .into()
        }).collect();
        column(items).spacing(2.0).into()
    };

    column([
        tabs.into(),
        horizontal_space().height(20.0).into(),
        search_input.into(),
        horizontal_space().height(10.0).into(),
        search_btn.into(),
        horizontal_space().height(10.0).into(),
        stats.into(),
        rebuild_btn.into(),
        horizontal_space().height(10.0).into(),
        results_list,
    ])
    .spacing(5.0)
    .padding(20.0)
    .into()
}

fn settings_view(app: &App) -> iced::Element<Message> {
    let title = text("Settings").size(24.0);

    let dirs_header = text("Index Directories").size(16.0);
    let mut dirs = column([dirs_header.into()]).spacing(5.0);

    for (i, dir) in app.settings.index_dirs.iter().enumerate() {
        let dir_row = row([
            text(dir).size(12.0).into(),
            horizontal_space().into(),
            button("Remove").on_press(Message::RemoveFolder(i)).into(),
        ]).spacing(10.0);
        dirs = dirs.push(dir_row);
    }

    let add_btn = button("Add Folder").on_press(Message::AddFolder).padding(8.0);
    dirs = dirs.push(add_btn);

    let search_tab = button("Search").on_press(Message::TabChanged(Tab::Search));
    let settings_tab = button("Settings").on_press(Message::TabChanged(Tab::Settings));
    let tabs = row([search_tab.into(), settings_tab.into()]).spacing(10.0);

    column([
        tabs.into(),
        title.into(),
        vertical_space().height(20.0).into(),
        dirs.into(),
    ])
    .spacing(10.0)
    .padding(20.0)
    .into()
}

pub fn run_ui(state: Arc<AppState>, _progress_rx: mpsc::Receiver<ProgressEvent>) {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    tracing::info!("Starting Flash Search UI with Iced...");

    let settings = Settings::default();
    let app = App::new(state);
    let _ = iced::application("Flash Search", update, view)
        .settings(settings)
        .theme(|_| Theme::Light)
        .run_with(|| (app, Task::none()));
}
