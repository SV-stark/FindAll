//! Window construction and the Iced application entry point.
//!
//! Separate from dispatch so the module tree reflects the Elm architecture's
//! three phases: `model` (state), `update` (commands), `view` (rendering), with
//! wiring here rather than mixed into the state module.

use super::icons;
use super::state::{App, app_theme};
use super::{Message, subscription, update};
use crate::commands::AppState;
use crate::scanner::ProgressEvent;
use iced::Task;
use parking_lot::Mutex;
use std::sync::Arc;

/// Renders the active screen.
#[must_use]
pub fn view(app: &App) -> iced::Element<'_, Message> {
    match app.active_tab {
        super::model::Tab::Search => super::search::search_view(app),
        super::model::Tab::Settings => super::settings::settings_view(app),
    }
}

/// Window title, reflecting indexing progress so the task is visible when the
/// window is not focused.
#[must_use]
pub fn app_title(app: &App) -> String {
    app.rebuild_status.as_ref().map_or_else(
        || "Flash Search".to_string(),
        |status| format!("Flash Search - {status}"),
    )
}

/// Runs the application to completion.
///
/// Returns a `FlashError` rather than panicking: under the release profile's
/// `panic = "abort"` a panic would terminate the process with no unwinding and
/// no chance to persist the index or write a crash note.
pub fn run_ui(
    state: &Result<Arc<AppState>, String>,
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
