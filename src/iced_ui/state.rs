//! Application state: the `Message` enum, the `App` struct, and its
//! construction and lifecycle methods.
//!
//! Split out of `iced_ui::mod` so that message handling (`super::update`) and
//! rendering (`super::view`) can be read without wading through the state
//! definition. Nothing in this module performs I/O on the caller's thread.

use super::model::{
    ContextMenuState, DOUBLE_CLICK_WINDOW, DateFilter, FileItem, LastClick, SearchMode, SortBy, Tab,
};
use super::theme;
use crate::commands::{AppState, search_filenames_internal, search_query_internal};
use crate::error::FlashError;
use crate::indexer::searcher::SearchParams;
use crate::scanner::ProgressEvent;
use crate::settings::AppSettings;
use crate::settings::Theme as AppSettingsTheme;
use iced::Task;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Every event the UI can react to.

#[derive(Debug, Clone)]
pub enum Message {
    TabChanged(Tab),
    SearchQueryChanged(String),
    SearchSubmitted,
    RunRecentSearch(String),
    ClearSearchHistory,
    SearchResultsReceived(usize, Vec<FileItem>),
    SearchError(FlashError),
    ResultSelected(usize),
    ItemHovered(Option<usize>),
    OpenFile(String),
    OpenFolder(String),
    CopyPath(String),
    ShowContextMenu(usize),
    HideContextMenu,
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
    CommitTextInputs,
    CustomExtensionsChanged(String),
    GlobalHotkeyChanged(String),
    AddFolder,
    RemoveFolder(usize),
    ToggleMinimizeToTray(bool),
    ToggleAutoStart(bool),
    ToggleAutoStartDone(bool),
    ToggleAutoStartFailed(bool, String),
    ToggleContextMenu(bool),
    DoubleClickActionChanged(crate::settings::DoubleClickAction),
    TogglePreviewPanel(bool),
    ToggleSearchHistory(bool),
    ToggleFilenameIndex(bool),
    ToggleContextMenuDone(bool),
    ToggleContextMenuFailed(bool, String),
    ToggleGitignore(bool),
    ToggleTheme,
    RebuildIndex,
    RemoveIndexDir(usize),
    ExcludePatternAdded(String),
    RemoveExcludePattern(usize),
    SaveSettings,
    ResetSettings,
    ThemeChanged(crate::settings::Theme),
    FontSizeChanged(crate::settings::FontSize),
    // Lifecycle
    PollProgressResult(Option<ProgressEvent>),
    PreviewLoaded(usize, crate::models::PreviewResult),
    IndexRebuilt,
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
    /// The user asked to close the window. Honoured only when
    /// `minimize_to_tray` is off, or turned into a minimize when it is on.
    WindowCloseRequested(iced::window::Id),
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
    FocusSearch,
    Escape,
}

/// Subscription identity for the indexing-progress stream.
///
/// Equality is defined by the channel: two subscriptions reading the same
/// `flume` receiver are the same logical subscription, so recreating it every
/// frame would make Iced drop and re-add the stream continuously.
#[derive(Debug, Clone)]
pub struct SubscriptionData {
    pub rx: flume::Receiver<ProgressEvent>,
}

impl Hash for SubscriptionData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // `same_channel` defines equality, so the hash must be constant.
        // Iced uses this only for deduplication within a single run.
        0u8.hash(state);
    }
}

impl PartialEq for SubscriptionData {
    fn eq(&self, other: &Self) -> bool {
        self.rx.same_channel(&other.rx)
    }
}

impl Eq for SubscriptionData {}

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
    /// Previous click on the results list, used to synthesise a double-click.
    ///
    /// Iced's `MouseArea` exposes only single- and right-click, so a
    /// double-click is reconstructed by timing two consecutive presses on the
    /// same row. Without this there was no pointer-driven way to open a result
    /// at all — only the Enter key worked.
    pub(crate) last_click: Option<LastClick>,
    /// `search_id` the pending [`LastClick`] belongs to.
    pub(crate) click_search_id: usize,
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
    /// Raw text currently in the "Maximum Search Results" field.
    ///
    /// Kept separate from `settings.max_results` because the setting is a
    /// `usize`: writing through on every keystroke made an unparseable value
    /// (including the empty string) silently snap the field back, so the box
    /// could never be cleared. Committed by [`Message::CommitTextInputs`].
    pub(crate) max_results_input: String,
    /// Raw text currently in the "Exclude Patterns" field.
    ///
    /// Same round-trip problem as `max_results_input`, and worse: the view
    /// rendered `exclude_patterns.join(", ")` while the handler split on `,`
    /// and dropped empties, so typing a separator re-flowed the text under the
    /// cursor.
    pub(crate) exclude_patterns_input: String,
    pub(crate) preview_result: Option<crate::models::PreviewResult>,
    pub(crate) is_loading_preview: bool,
    pub(crate) context_menu: Option<ContextMenuState>,
    #[allow(dead_code)]
    pub(crate) tray_icon: Option<tray_icon::TrayIcon>,
    pub(crate) window_id: Option<iced::window::Id>,
    pub(crate) progress_rx: Option<flume::Receiver<ProgressEvent>>,
    pub(crate) active_search_id: Arc<AtomicUsize>,
    pub(crate) active_preview_id: Arc<AtomicUsize>,
}

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
            last_click: None,
            click_search_id: 0,
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
            max_results_input: String::new(),
            exclude_patterns_input: String::new(),
            preview_result: None,
            context_menu: None,
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
    /// Builds the initial state, reconciling persisted settings against the OS.
    pub(super) fn new(
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
                // `Auto` resolves against the OS appearance rather than
                // defaulting to light. Uses `resolve_is_dark` so there is a
                // single definition of the mapping.
                let is_dark = resolve_is_dark_for(settings.theme);

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

                // Seed the settings-field drafts from what was actually loaded
                // so the boxes start out showing the persisted values.
                app.max_results_input = settings.max_results.to_string();
                app.exclude_patterns_input = settings.exclude_patterns.join(", ");

                // Publish the persisted font size before the first frame, so the
                // very first render already uses it instead of flashing the
                // default and then resizing.
                theme::set_text_scale(settings.font_size);

                // Reconcile the OS-side integration with the stored setting at
                // launch. Previously the checkbox and the actual registry state
                // could disagree indefinitely: the setting was persisted but
                // nothing ever read it back to register or remove the entry.
                crate::system::startup::sync_auto_start(settings.auto_start_on_boot);
                crate::system::context_menu::sync_context_menu(settings.context_menu_enabled);

                app
            }
            Err(e) => Self {
                error: Some(e),
                progress_rx,
                ..Default::default()
            },
        }
    }

    pub(super) fn parse_size_filter(size_str: &str) -> (Option<u64>, Option<u64>) {
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

    pub(super) fn get_min_modified(&self) -> Option<u64> {
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
    pub(super) fn perform_search(&mut self, debounce: bool) -> Task<Message> {
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

    /// Records the current query into the search history.
    ///
    /// No-op for an empty or whitespace-only query, and when
    /// [`AppSettings::search_history_enabled`] is off. Returns the task that
    /// persists the update; the in-memory copy is refreshed optimistically so the
    /// welcome card appears immediately.
    pub(super) fn record_search_history(&mut self) -> Task<Message> {
        let query = self.search_query.trim().to_string();
        if !self.settings.search_history_enabled || query.is_empty() {
            return Task::none();
        }

        // Mirror the persistence layer's ranking so the UI and the stored value
        // cannot disagree if the write fails.
        let mut history = std::mem::take(&mut self.settings.search_history);
        match history.iter_mut().find(|item| item.query == query) {
            Some(item) => {
                item.frequency = item.frequency.saturating_add(1);
                item.last_used = crate::settings::now_unix_secs();
            }
            None => history.push(crate::settings::SearchHistoryItem {
                query: query.clone(),
                frequency: 1,
                last_used: crate::settings::now_unix_secs(),
            }),
        }
        history.sort_by_key(|item| std::cmp::Reverse(item.frequency));
        history.truncate(50);
        self.settings.search_history = history;

        // Persisted through the single settings path rather than a dedicated
        // command: a second implementation of the same ranking would either
        // double-count the entry the UI already inserted or drift from it.
        self.save_settings()
    }

    /// Whether a press at `idx` and `at` completes a double-click.
    ///
    /// Does not mutate state; the caller is responsible for re-arming
    /// [`App::last_click`] so a triple-click is not counted twice.
    pub(super) fn pairs_as_double_click(&self, idx: usize, at: std::time::Instant) -> bool {
        matches!(self.last_click, Some(prev)
        if prev.result_index == idx && at.saturating_duration_since(prev.at) <= DOUBLE_CLICK_WINDOW)
    }

    /// Performs [`AppSettings::double_click_action`] against the selected result.
    ///
    /// Bounds-checked: the selection is re-read from `results` rather than
    /// trusted from the click that triggered it.
    pub(super) fn run_double_click_action(&self) -> Task<Message> {
        let Some(path) = self
            .selected_index
            .and_then(|idx| self.results.get(idx))
            .map(|item| item.path.clone())
        else {
            return Task::none();
        };

        match self.settings.double_click_action {
            crate::settings::DoubleClickAction::OpenFile => Task::perform(
                async move {
                    if let Err(e) = opener::open(std::path::Path::new(&path)) {
                        tracing::error!("Double-click open failed for {path}: {e}");
                        Message::StatusUpdate(format!("Could not open: {e}"))
                    } else {
                        Message::NoOp
                    }
                },
                Message::from,
            ),
            crate::settings::DoubleClickAction::ShowInFolder => Task::perform(
                async move {
                    if let Err(e) = crate::commands::open_folder_internal(&path) {
                        tracing::error!("Double-click reveal failed for {path}: {e}");
                        Message::StatusUpdate(format!("Could not reveal in folder: {e}"))
                    } else {
                        Message::NoOp
                    }
                },
                Message::from,
            ),
            crate::settings::DoubleClickAction::Preview => {
                // The preview panel already loads on single click, so this arm
                // is deliberately inert rather than re-fetching the same file.
                Task::none()
            }
        }
    }

    /// Folds the settings-text drafts into [`AppSettings`].
    ///
    /// An unparseable `max_results` keeps the previous value instead of
    /// resetting it, and the draft is reset to the value that was actually
    /// applied so the box stops showing rejected input. `max_results` is
    /// clamped to a sane band: zero would make every search return nothing and
    /// an absurd value would let a single query allocate unboundedly.
    pub(super) fn commit_text_inputs(&mut self) {
        match self.max_results_input.trim().parse::<usize>() {
            Ok(n) => {
                self.settings.max_results = n.clamp(1, crate::settings::MAX_RESULTS_LIMIT);
            }
            Err(_) if self.max_results_input.trim().is_empty() => {
                self.settings.max_results = crate::settings::DEFAULT_MAX_RESULTS;
            }
            Err(_) => {
                tracing::warn!(
                    "Ignoring unparseable max_results input {:?}; keeping {}",
                    self.max_results_input,
                    self.settings.max_results
                );
            }
        }
        self.max_results_input = self.settings.max_results.to_string();

        self.settings.exclude_patterns = self
            .exclude_patterns_input
            .split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(ToString::to_string)
            .collect();
        self.exclude_patterns_input = self.settings.exclude_patterns.join(", ");
    }

    /// Flushes the text drafts, persists settings, and pushes the new values into
    /// the live background workers.
    ///
    /// Takes `&mut self` so every persist path necessarily folds the drafts in
    /// first. Routing through one function is what stops an unrelated toggle
    /// from writing settings to disk while a typed-in value is still pending in
    /// a text box.
    ///
    /// Delegates to [`crate::commands::save_settings_internal`] rather than
    /// writing the file directly. This used to re-implement the save, and the
    /// partial copy skipped the `settings_cache` update and the watcher
    /// reconfiguration — so editing exclude patterns or custom extensions
    /// persisted to disk but the running scanner and watcher kept using the
    /// values they captured at startup.
    ///
    /// Returns a task that reports a failure to the user rather than silently
    /// discarding it.
    pub(super) fn save_settings(&mut self) -> Task<Message> {
        self.commit_text_inputs();
        let Some(state) = self.state.clone() else {
            return Task::none();
        };
        let settings = self.settings.clone();
        Task::perform(
            async move {
                match crate::commands::save_settings_internal(&settings, &state) {
                    Ok(()) => Message::NoOp,
                    Err(e) => {
                        tracing::error!("Failed to save settings: {e}");
                        Message::StatusUpdate(format!("Could not save settings: {e}"))
                    }
                }
            },
            Message::from,
        )
    }
}

/// The Iced theme for the current state.
///
/// `const` because Iced requires a plain `fn(&App) -> Theme`; it is called on
/// every retheme.
pub const fn app_theme(app: &App) -> iced::Theme {
    if app.is_dark {
        iced::Theme::Dark
    } else {
        iced::Theme::Light
    }
}

impl App {
    /// Resolves the effective light/dark value for the current theme setting.
    ///
    /// `Theme::Auto` used to be treated as "light" unconditionally: `is_dark`
    /// was initialised solely from `matches!(theme, Theme::Dark)`, so `Auto` and
    /// `Light` were indistinguishable and following the OS appearance was
    /// impossible. `Auto` now consults the OS preference directly.
    #[must_use]
    pub fn resolve_is_dark(&self) -> bool {
        resolve_is_dark_for(self.settings.theme)
    }
}

/// Maps a theme setting to an effective appearance.
///
/// A free function so construction (before an `App` exists) and the `App` method
/// share one definition; they previously duplicated the match.
fn resolve_is_dark_for(theme: AppSettingsTheme) -> bool {
    match theme {
        AppSettingsTheme::Dark => true,
        AppSettingsTheme::Light => false,
        AppSettingsTheme::Auto => system_prefers_dark(),
    }
}

/// Whether the OS is currently configured for a dark appearance.
///
/// `iced` 0.14 exposes no cross-platform "system appearance" query, so on
/// Windows this reads `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize`
/// (`AppsUseLightTheme`), which is the same source Explorer and Settings use.
/// Other platforms fall back to light, which is the previous behaviour rather
/// than a new failure mode.
fn system_prefers_dark() -> bool {
    #[cfg(windows)]
    {
        use winreg::RegKey;
        use winreg::enums::HKEY_CURRENT_USER;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let Ok(key) =
            hkcu.open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize")
        else {
            return false;
        };
        // `AppsUseLightTheme` is 0 when Windows is in dark mode.
        key.get_value::<u32, _>("AppsUseLightTheme")
            .is_ok_and(|v| v == 0)
    }

    #[cfg(not(windows))]
    {
        false
    }
}
