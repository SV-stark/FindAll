//! Message dispatch: the `update` function and the helpers it delegates to.
//!
//! This is the largest single unit in the UI layer and is deliberately kept
//! separate from [`super::state`] (what the state *is*) and
//! [`super::subscription`] (what the UI is listening to). Splitting on that
//! boundary means a change to state shape does not require reading dispatch
//! logic, and vice versa.

use super::Message;
use super::model::{ContextMenuState, DateFilter, LastClick, SearchMode, Tab, get_search_input_id};
use super::state::App;
use super::theme;
use crate::commands::get_file_preview_highlighted_internal;
use crate::settings::AppSettings;
use iced::Task;
use std::sync::atomic::Ordering;

/// Applies one `Message` to the state and returns any follow-up work.
///
/// A single `match` over every message.
///
/// Long by necessity: it is the complete dispatch table, and each arm is short
/// and usually delegates to a named method on `App`. Splitting it by message
/// group would scatter related transitions across files and make it harder to
/// confirm that every variant is handled — the compiler already guarantees that,
/// which is the property actually worth preserving.
#[allow(clippy::too_many_lines)]
pub fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::TabChanged(tab) => {
            // Leaving Settings must flush the text drafts; otherwise a value
            // typed but not submitted is silently dropped on tab change.
            let leaving_settings = app.active_tab == Tab::Settings && tab != Tab::Settings;
            app.active_tab = tab;
            if leaving_settings {
                app.commit_text_inputs();
                app.save_settings()
            } else {
                Task::none()
            }
        }
        Message::SearchQueryChanged(q) => {
            app.search_query = q;
            app.perform_search(true)
        }
        Message::SearchSubmitted => {
            // Only an explicitly submitted query is recorded; recording every
            // keystroke-driven search would fill the history with prefixes.
            let record = app.record_search_history();
            Task::batch(vec![app.perform_search(false), record])
        }
        Message::RunRecentSearch(query) => {
            app.search_query = query;
            let record = app.record_search_history();
            Task::batch(vec![app.perform_search(true), record])
        }
        Message::ClearSearchHistory => {
            app.settings.search_history.clear();
            app.save_settings()
        }
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

            // Pair the press with the previous one to detect a double-click.
            // A new search invalidates the pairing because indices now refer to
            // different rows.
            if app.search_id != app.click_search_id {
                app.last_click = None;
                app.click_search_id = app.search_id;
            }
            let now = std::time::Instant::now();
            let is_double_click = app.pairs_as_double_click(idx, now);
            // Always re-arm, so a triple-click is a double-click followed by a
            // single click rather than two double-clicks.
            app.last_click = Some(LastClick {
                result_index: idx,
                at: now,
            });

            if is_double_click {
                return app.run_double_click_action();
            }

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
        Message::ShowContextMenu(idx) => {
            // Bounds-checked: the list can shrink between the click being
            // emitted and this handler running.
            app.context_menu = app.results.get(idx).map(|item| ContextMenuState {
                result_index: idx,
                path: item.path.clone(),
                pinned: app.settings.pinned_files.contains(&item.path),
            });
            if app.context_menu.is_some() {
                app.selected_index = Some(idx);
            } else {
                tracing::debug!("Ignoring context menu on out-of-range result {idx}");
            }
            Task::none()
        }
        Message::HideContextMenu => {
            app.context_menu = None;
            Task::none()
        }
        Message::FocusSearch => {
            // Switching to the Search tab first, otherwise focus lands on an
            // input that is not mounted.
            app.active_tab = Tab::Search;
            // Clear the pending double-click so a keyboard focus does not get
            // mistaken for the second half of a click pair.
            app.last_click = None;
            iced::widget::operation::focus(get_search_input_id())
        }
        Message::Escape => {
            // Dismiss in priority order: open menu, then selection, then query.
            // The first two only mutate state, so they share a branch.
            if app.context_menu.take().is_some() || app.selected_index.take().is_some() {
                Task::none()
            } else if !app.search_query.is_empty() {
                app.search_query.clear();
                app.perform_search(true)
            } else {
                Task::none()
            }
        }
        Message::PinFile(path) => {
            app.settings.pinned_files.push(path);
            let save = app.save_settings();
            if let Some(menu) = &mut app.context_menu {
                menu.pinned = true;
            }
            save
        }
        Message::UnpinFile(path) => {
            app.settings.pinned_files.retain(|p| p != &path);
            let save = app.save_settings();
            if let Some(menu) = &mut app.context_menu {
                menu.pinned = false;
            }
            save
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
            // The filename index is not built when the setting is off, so
            // selecting that mode would fail deep in the search layer with an
            // opaque error. Refuse it here instead.
            if m == SearchMode::Filename && !app.settings.filename_index_enabled {
                app.search_error =
                    Some("Filename search is disabled. Enable it in Settings.".to_string());
                return Task::none();
            }
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
            // Own the raw text only. Committing on every keystroke is what made
            // the field un-clearable.
            app.max_results_input = s;
            Task::none()
        }
        Message::ExcludePatternsChanged(s) => {
            app.exclude_patterns_input = s;
            Task::none()
        }
        Message::CommitTextInputs => {
            app.commit_text_inputs();
            app.save_settings()
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
            // The label on this checkbox is "minimize to system tray on window
            // close", but there was no close-request handler anywhere, so
            // closing the window quit the app regardless. The setting only
            // decided whether a tray icon got created, and the toggle never
            // called `save_settings()`, so it was also lost on restart.
            //
            // Now the tray icon follows the setting *and* the choice is
            // persisted; `Message::WindowCloseRequested` (subscription) is what
            // actually intercepts the close.
            app.settings.minimize_to_tray = b;
            if b {
                if app.tray_icon.is_none() {
                    match crate::system::tray::create_tray_icon() {
                        Ok(icon) => app.tray_icon = Some(icon),
                        Err(e) => {
                            tracing::error!("Failed to create tray icon: {e}");
                            app.error = Some(format!("Could not create the system tray icon: {e}"));
                            // Roll the checkbox back: the OS state and the
                            // setting must not silently disagree.
                            app.settings.minimize_to_tray = false;
                            return app.save_settings();
                        }
                    }
                }
            } else {
                app.tray_icon = None;
            }
            app.save_settings()
        }
        Message::ToggleAutoStart(b) => {
            // The checkbox used to flip the setting and nothing else, so the
            // app never actually registered with the OS. Perform the real
            // registration and revert the checkbox if it fails, otherwise the
            // setting and the OS state silently disagree.
            //
            // `set_auto_start` shells out to the platform helper on some
            // backends, so it runs on a blocking thread rather than on an
            // async worker.
            Task::perform(
                async move {
                    let result = tokio::task::spawn_blocking(move || {
                        crate::system::startup::set_auto_start(b)
                    })
                    .await;
                    match result {
                        Ok(Ok(())) => Message::ToggleAutoStartDone(b),
                        Ok(Err(e)) => {
                            tracing::error!("Failed to update auto-start registration: {e}");
                            Message::ToggleAutoStartFailed(b, e.to_string())
                        }
                        Err(e) => {
                            tracing::error!("Auto-start task panicked: {e}");
                            Message::ToggleAutoStartFailed(b, e.to_string())
                        }
                    }
                },
                Message::from,
            )
        }
        Message::ToggleAutoStartDone(b) => {
            app.settings.auto_start_on_boot = b;
            app.save_settings()
        }
        Message::ToggleAutoStartFailed(b, error) => {
            // Roll the checkbox back to its previous value and tell the user why.
            app.settings.auto_start_on_boot = !b;
            app.error = Some(format!("Could not update auto-start: {error}"));
            Task::none()
        }
        Message::ToggleContextMenu(b) => Task::perform(
            async move {
                let result = tokio::task::spawn_blocking(move || {
                    crate::system::context_menu::register_context_menu(b)
                })
                .await;
                match result {
                    Ok(Ok(())) => Message::ToggleContextMenuDone(b),
                    Ok(Err(e)) => {
                        tracing::error!("Failed to update shell context menu: {e}");
                        Message::ToggleContextMenuFailed(b, e.to_string())
                    }
                    Err(e) => {
                        tracing::error!("Context menu task panicked: {e}");
                        Message::ToggleContextMenuFailed(b, e.to_string())
                    }
                }
            },
            Message::from,
        ),
        Message::ToggleContextMenuDone(b) => {
            app.settings.context_menu_enabled = b;
            app.save_settings()
        }
        Message::ToggleContextMenuFailed(b, error) => {
            app.settings.context_menu_enabled = !b;
            app.error = Some(format!(
                "Could not update the Explorer context menu: {error}"
            ));
            Task::none()
        }
        Message::ToggleGitignore(b) => {
            // This only flipped the in-memory field and returned `Task::none()`,
            // so the choice never reached disk and was gone on restart.
            app.settings.use_gitignore = b;
            app.save_settings()
        }
        Message::ToggleTheme => {
            // Cycle System -> Light -> Dark -> System, so `Theme::Auto` is
            // reachable again. The old two-way flip overwrote the stored value
            // with an explicit Light/Dark, which destroyed a user's `Auto`
            // choice on the first toggle.
            app.settings.theme = match app.settings.theme {
                crate::settings::Theme::Auto => crate::settings::Theme::Light,
                crate::settings::Theme::Light => crate::settings::Theme::Dark,
                crate::settings::Theme::Dark => crate::settings::Theme::Auto,
            };
            app.is_dark = app.resolve_is_dark();
            app.save_settings()
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
                        if let Err(e) = state.start_indexing(std::path::PathBuf::from(&dir)).await {
                            tracing::error!("Failed to start indexing {dir}: {e}");
                        }
                    }
                    Message::IndexRebuilt
                });
            }
            Task::none()
        }
        Message::ExcludePatternAdded(p) => {
            if !p.is_empty() && !app.settings.exclude_patterns.contains(&p) {
                app.settings.exclude_patterns.push(p);
                // Keep the draft in step, or the next `save_settings` would fold
                // the stale draft back over the pattern just added.
                app.exclude_patterns_input = app.settings.exclude_patterns.join(", ");
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
            // Drafts are seeded from the settings they belong to; leaving them
            // stale would let the next `save_settings` write the old values back
            // over the reset.
            app.max_results_input = app.settings.max_results.to_string();
            app.exclude_patterns_input = app.settings.exclude_patterns.join(", ");
            app.filter_extensions = app
                .settings
                .default_filters
                .file_types
                .iter()
                .cloned()
                .collect();
            theme::set_text_scale(app.settings.font_size);
            app.is_dark = app.resolve_is_dark();
            app.error = None;
            // Persist and reconfigure the live watcher; previously this only
            // mutated in-memory state and was lost on restart.
            app.save_settings()
        }
        Message::ThemeChanged(t) => {
            app.settings.theme = t;
            app.is_dark = app.resolve_is_dark();
            app.save_settings()
        }
        Message::FontSizeChanged(f) => {
            app.settings.font_size = f;
            theme::set_text_scale(f);
            app.save_settings()
        }
        Message::DoubleClickActionChanged(action) => {
            app.settings.double_click_action = action;
            app.save_settings()
        }
        Message::TogglePreviewPanel(b) => {
            app.settings.show_preview_panel = b;
            if !b {
                // Drop the loaded preview so the pane cannot keep rendering a
                // stale file after being switched off.
                app.preview_result = None;
                app.is_loading_preview = false;
            }
            app.save_settings()
        }
        Message::ToggleSearchHistory(b) => {
            app.settings.search_history_enabled = b;
            if !b {
                // Turning history off drops what was already collected: keeping
                // it would leave the setting claiming nothing is recorded while
                // a full query log sits on disk.
                app.settings.search_history.clear();
            }
            app.save_settings()
        }
        Message::ToggleFilenameIndex(b) => {
            app.settings.filename_index_enabled = b;
            // Leave Filename mode if it is currently selected, otherwise the UI
            // would sit in a mode that cannot run until the next restart.
            if !b && app.search_mode == SearchMode::Filename {
                app.search_mode = SearchMode::FullText;
            }
            app.save_settings()
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
        Message::WindowCloseRequested(id) => {
            // With the tray enabled, closing the window hides it instead of
            // quitting. Without a tray there would be no way back, so the close
            // proceeds.
            if app.settings.minimize_to_tray && app.tray_icon.is_some() {
                iced::window::minimize(id, true)
            } else {
                iced::window::close(id)
            }
        }
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
                        if let Err(e) = state
                            .start_indexing(std::path::PathBuf::from(path_clone))
                            .await
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

                    let cleanup_task =
                        Task::future(
                            async move { app_purge_directory(&state, &removed_dir).await },
                        );

                    return Task::batch(vec![save_task, cleanup_task]);
                }
            }
            Task::none()
        }
        Message::RemoveExcludePattern(i) => {
            if i < app.settings.exclude_patterns.len() {
                app.settings.exclude_patterns.remove(i);
                app.exclude_patterns_input = app.settings.exclude_patterns.join(", ");
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
            app.context_menu = None;
            if let Some(idx) = app.selected_index
                && idx < app.results.len()
            {
                let path = app.results[idx].path.clone();
                return Task::done(Message::OpenFile(path));
            }
            Task::none()
        }
        Message::ShowSelectedInFolder => {
            app.context_menu = None;
            if let Some(idx) = app.selected_index
                && idx < app.results.len()
            {
                let path = app.results[idx].path.clone();
                return Task::done(Message::OpenFolder(path));
            }
            Task::none()
        }
        Message::CopySelectedPath => {
            app.context_menu = None;
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

        let borrowed: Vec<&std::path::Path> = paths.iter().map(std::path::Path::new).collect();
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
