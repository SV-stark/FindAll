//! Iced user interface.
//!
//! The Elm architecture maps onto four modules here, one per phase:
//!
//! - [`model`] — what the UI state *is*: view models, filter enums, and the pure
//!   formatting helpers. No `Message`, no I/O.
//! - [`state`] — the `App` struct, the `Message` enum, and construction.
//! - [`commands`] — `update`: applying one message to the state.
//! - [`run`] — `view` and the window wiring.
//!
//! Two supporting modules sit alongside them:
//!
//! - [`subscription`] — event streams (progress, hotkeys, tray, keyboard).
//! - [`hotkey`] — hotkey string parsing, split out because it is a pure function
//!   with no reference to `App` or `Message`.
//!
//! This file is deliberately just wiring and re-exports. It was previously a
//! 2,300-line module holding all of the above plus a 750-line `update` and the
//! test suite, which made the boundaries invisible and left no seam for testing
//! anything but the pure helpers.
//!
//! The public surface is unchanged: `iced_ui::{App, Message, update, view,
//! subscription, run_ui}` is what `lib.rs` and the view modules consume.

pub mod icons;
pub mod search;
pub mod settings;
pub mod theme;

mod commands;
mod hotkey;
mod model;
mod run;
mod state;
mod subscription;

#[cfg(test)]
mod tests;

pub use commands::update;
pub use model::{
    ContextMenuState, DOUBLE_CLICK_WINDOW, DateFilter, FileItem, LastClick, SearchMode, SortBy,
    Tab, format_date, format_size, get_progress_subscription_id, get_search_input_id,
};
pub use run::{app_title, run_ui, view};
pub use state::{App, Message, SubscriptionData, app_theme};
pub use subscription::subscription;
