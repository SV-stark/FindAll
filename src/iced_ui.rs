use crate::commands::AppState;
use crate::commands::{search_query_internal, search_filenames_internal, get_file_preview_highlighted_internal};
use crate::indexer::searcher::SearchResult;
use crate::models::FilenameSearchResult;
use crate::scanner::ProgressEvent;
use crate::settings::AppSettings;
use iced::widget::{button, column, container, row, text, TextInput, Space, Scrollable};
use iced::{Element, Length, Settings, Theme, Task};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

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
    ResultSelected(usize),
    OpenFile(String),
    CopyPath(String),
    TabChanged(Tab),
    RebuildIndex,
    IndexRebuilt,
    AddFolder,
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
}

pub struct App {
    state: Arc<AppState>,
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
}

impl App {
    fn new(state: Arc<AppState>) -> Self {
        let settings = state.settings_manager.load().unwrap_or_default();
        let stats = state.indexer.get_statistics().unwrap_or_default();
        let index_size = format!("{:.1} MB", (stats.total_size_bytes as f64) / 1_048_576.0);
        let is_dark = matches!(settings.theme, crate::settings::Theme::Dark);
        App { 
            state, search_query: String::new(), results: Vec::new(), selected_index: None, 
            is_searching: false, settings, active_tab: Tab::Search,
            files_indexed: stats.total_documents as i32, index_size, is_dark,
            search_mode: SearchMode::FullText, filter_extension: String::new(),
            filter_size: String::new(), preview_content: None, is_loading_preview: false,
        }
    }

    fn parse_size_filter(size_str: &str) -> (Option<u64>, Option<u64>) {
        let size_str = size_str.trim();
        if size_str.is_empty() {
            return (None, None);
        }
        
        let (op, num_str) = if size_str.starts_with(">") {
            (">", &size_str[1..])
        } else if size_str.starts_with("<") {
            ("<", &size_str[1..])
        } else if size_str.starts_with(">=") {
            (">=", &size_str[2..])
        } else if size_str.starts_with("<=") {
            ("<=", &size_str[2..])
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
        let query = self.search_query.clone();
        let state = self.state.clone();
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
            let items = match mode {
                SearchMode::Filename => {
                    match search_filenames_internal(query, max_results, &state).await {
                        Ok(results) => results.into_iter().map(FileItem::from).collect(),
                        Err(_) => Vec::new(),
                    }
                }
                SearchMode::FullText => {
                    match search_query_internal(
                        query, max_results, &state, min_size, max_size, extension
                    ).await {
                        Ok(results) => results.into_iter().map(FileItem::from).collect(),
                        Err(_) => Vec::new(),
                    }
                }
            };
            Message::SearchResultsReceived(items)
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

    fn save_settings(&self) { let _ = self.state.settings_manager.save(&self.settings); }
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::SearchQueryChanged(q) => { app.search_query = q; Task::none() }
        Message::SearchSubmitted => app.perform_search(),
        Message::SearchResultsReceived(results) => { 
            app.results = results; 
            app.is_searching = false; 
            if !app.results.is_empty() {
                app.selected_index = Some(0);
            }
            app.load_preview()
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
            let state = app.state.clone();
            let settings = app.settings.clone();
            Task::future(async move {
                let _ = state.indexer.clear();
                let _ = state.indexer.commit();
                let _ = state.metadata_db.clear();
                for dir in settings.index_dirs {
                    let _ = state.scanner.scan_directory(PathBuf::from(dir), settings.exclude_patterns.clone()).await;
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
                let f = folder.to_string_lossy().to_string();
                if !app.settings.index_dirs.iter().any(|d| d == &f) { app.settings.index_dirs.push(f); }
            }
            Task::none()
        }
        Message::RemoveFolder(i) => { if i < app.settings.index_dirs.len() { app.settings.index_dirs.remove(i); } Task::none() }
        Message::SaveSettings => { app.save_settings(); Task::none() }
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
                if current < app.results.len() - 1 {
                    app.selected_index = Some(current + 1);
                    return app.load_preview();
                }
            }
            Task::none()
        }
    }
}

fn view(app: &App) -> Element<Message> {
    match app.active_tab {
        Tab::Search => search_view(app),
        Tab::Settings => settings_view(app),
    }
}

fn search_view(app: &App) -> Element<Message> {
    let mode_text = match app.search_mode {
        SearchMode::FullText => "Full Text",
        SearchMode::Filename => "Filename Only",
    };
    
    let input = TextInput::new("Search files...", &app.search_query)
        .on_input(Message::SearchQueryChanged)
        .on_submit(Message::SearchSubmitted)
        .padding(10).size(18);
    
    let filter_ext = TextInput::new("ext:pdf", &app.filter_extension)
        .on_input(Message::FilterExtensionChanged)
        .on_submit(Message::SearchSubmitted)
        .padding(5).size(14).width(Length::Fixed(80.0));
    
    let filter_size = TextInput::new("size:>1MB", &app.filter_size)
        .on_input(Message::FilterSizeChanged)
        .on_submit(Message::SearchSubmitted)
        .padding(5).size(14).width(Length::Fixed(100.0));
    
    let tabs = row([
 button("Search").on_press(Message::TabChanged(Tab::Search)).into(),
        button("Settings").on_press(Message::TabChanged(Tab::Settings)).into()
    ]).spacing(10);
    
    let theme_btn = button(if app.is_dark { "Light" } else { "Dark" }).on_press(Message::ToggleTheme).padding(8);
    let mode_btn = button(mode_text).on_press(Message::ToggleSearchMode).padding(8);
    let rebuild_btn = button("Rebuild").on_press(Message::RebuildIndex).padding(8);
    
    let toolbar = row([theme_btn.into(), mode_btn.into(), rebuild_btn.into()]).spacing(10);
    let stats = text(format!("Files: {} | Index: {}", app.files_indexed, app.index_size));
    
    let results_panel: Element<Message> = if app.is_searching {
        container(text("Searching...")).width(Length::Fill).height(Length::Fill)
            .center_x(Length::Fill).center_y(Length::Fill).into()
    } else if app.results.is_empty() {
        container(text("No results - enter a query")).width(Length::Fill).height(Length::Fill)
            .center_x(Length::Fill).center_y(Length::Fill).into()
    } else {
        let items: Vec<_> = app.results.iter().enumerate().map(|(i, item)| {
            let ext_str = item.extension.as_deref().unwrap_or("");
            
            let item_content = row([
                text(&item.title).size(14).width(Length::Fill).into(),
                text(ext_str).size(12).color(iced::Color::from_rgb(0.5, 0.5, 0.5)).into(),
            ]).spacing(10);
            
            button(item_content)
                .on_press(Message::ResultSelected(i))
                .width(Length::Fill)
                .padding(8)
                .into()
        }).collect();
        Scrollable::new(column(items).spacing(2)).height(Length::Fill).into()
    };
    
    let preview_panel: Element<Message> = if app.is_loading_preview {
        container(text("Loading preview...")).width(Length::Fill).height(Length::Fill)
            .center_x(Length::Fill).center_y(Length::Fill).into()
    } else if let Some(ref preview) = app.preview_content {
        let preview_text = text(preview).size(12);
        Scrollable::new(container(preview_text).padding(10).width(Length::Fill))
            .height(Length::Fill).into()
    } else {
        container(text("Select a file to preview")).width(Length::Fill).height(Length::Fill)
            .center_x(Length::Fill).center_y(Length::Fill).into()
    };
    
    let filter_row = row([filter_ext.into(), filter_size.into(), button("Search").on_press(Message::SearchSubmitted).padding(5).into()])
        .spacing(10);
    
    let main_content = row([
        column([input.into(), filter_row.into(), results_panel.into()])
            .width(Length::Fill)
            .spacing(10)
            .into(),
        column([text("Preview").size(16).into(), preview_panel.into()])
            .width(Length::Fixed(350.0))
            .spacing(10)
            .into(),
    ]).spacing(10);
    
    column([tabs.into(), toolbar.into(), stats.into(), main_content.into()])
        .padding(20).spacing(10).into()
}

fn settings_view(app: &App) -> Element<Message> {
    let title = text("Settings").size(24);
    let tabs = row([
        button("Search").on_press(Message::TabChanged(Tab::Search)).into(),
        button("Settings").on_press(Message::TabChanged(Tab::Settings)).into()
    ]).spacing(10);
    
    let max_results_label = text("Max Results:");
    let max_results_val = app.settings.max_results.to_string();
    let max_results_input = TextInput::new("100", &max_results_val)
        .padding(5).size(14).width(Length::Fixed(100.0));
    
    let exclude_label = text("Exclude Patterns (comma separated):");
    let exclude_val = app.settings.exclude_patterns.join(", ");
    let exclude_input = TextInput::new("*.git, target, node_modules", &exclude_val)
        .padding(5).size(14).width(Length::Fill);
    
    let mut dirs = column([text("Index Directories").size(16).into()]).spacing(5);
    for (i, dir) in app.settings.index_dirs.iter().enumerate() {
        let row_item = row([
            text(dir).size(12).width(Length::Fill).into(),
            button("X").on_press(Message::RemoveFolder(i)).into()
        ]).spacing(10);
        dirs = dirs.push(row_item);
    }
    
    let add_btn = button("Add Folder").on_press(Message::AddFolder).padding(8);
    let save_btn = button("Save Settings").on_press(Message::SaveSettings).padding(10);
    
    let settings_form = column([
        max_results_label.into(),
        max_results_input.into(),
        Space::new(Length::Fixed(0.0), Length::Fixed(15.0)).into(),
        exclude_label.into(),
        exclude_input.into(),
        Space::new(Length::Fixed(0.0), Length::Fixed(15.0)).into(),
        dirs.into(),
        add_btn.into(),
        Space::new(Length::Fixed(0.0), Length::Fixed(15.0)).into(),
        save_btn.into(),
    ]).spacing(5);
    
    column([tabs.into(), title.into(), Space::new(Length::Fixed(0.0), Length::Fixed(20.0)).into(), settings_form.into()])
        .padding(20).spacing(10).into()
}

pub fn run_ui(state: Arc<AppState>, _progress_rx: mpsc::Receiver<ProgressEvent>) {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive(tracing::Level::INFO.into()))
        .init();

    let settings = Settings::default();
    let app = App::new(state);
    let is_dark = app.is_dark;
    
    let _ = iced::application("Flash Search", update, view)
        .settings(settings)
        .theme(move |_| if is_dark { Theme::Dark } else { Theme::Light })
        .run_with(|| (app, Task::none()));
}
