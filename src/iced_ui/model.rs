//! View-model types and pure display helpers.
//!
//! Everything here is data plus formatting: no `Message`, no `App`, no I/O. That
//! makes it directly testable, which the previous monolithic `iced_ui::mod`
//! arrangement made impractical — these types had no way to be exercised without
//! standing up the whole application.

use crate::indexer::searcher::SearchResult;
use compact_str::CompactString;
use iced::widget::Id;

/// Which top-level screen is showing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tab {
    Search,
    Settings,
}

/// A row in the results list.
///
/// A view model, distinct from `SearchResult`: the title is always resolved to a
/// displayable string here, so the view never has to decide what to show when a
/// document has no embedded title.
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
            // Fall back to the file name; a document with neither a title nor a
            // parseable path still needs something to display.
            title: r.title.as_ref().map_or_else(
                || {
                    std::path::Path::new(&path_clone)
                        .file_name()
                        .and_then(std::ffi::OsStr::to_str)
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

impl From<crate::indexer::filename_index::FilenameSearchResult> for FileItem {
    fn from(r: crate::indexer::filename_index::FilenameSearchResult) -> Self {
        let path_clone = r.file_path.clone();
        Self {
            // Filename matches are exact by construction, so there is no BM25
            // score to carry across.
            score: 1.0,
            path: r.file_path,
            title: r.file_name.to_string(),
            extension: std::path::Path::new(&path_clone)
                .extension()
                .and_then(std::ffi::OsStr::to_str)
                .map(CompactString::from),
            // The filename index stores no size or mtime; the result list shows
            // blanks rather than misleading zeros for these.
            size: None,
            modified: None,
            snippets: Vec::new(),
        }
    }
}

/// Which result a right-click context menu is acting on.
///
/// Right-clicking a result used to emit `ShowContextMenu`, which no handler
/// matched, so the right-click silently did nothing. The menu state is explicit
/// so the action buttons always know which row they refer to, even if the results
/// list changes underneath them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextMenuState {
    pub result_index: usize,
    pub path: String,
    pub pinned: bool,
}

/// A press on the results list, retained long enough to pair with the next one.
#[derive(Debug, Clone, Copy)]
pub struct LastClick {
    pub result_index: usize,
    pub at: std::time::Instant,
}

/// Maximum gap between two presses on the same row that still counts as a double-click.
///
/// Iced's `MouseArea` exposes only single- and right-click, so double-clicks are
/// reconstructed by timing two presses. Windows' own default is 500ms; slightly
/// under that keeps the app feeling responsive without misfiring on slow
/// deliberate clicks.
pub const DOUBLE_CLICK_WINDOW: std::time::Duration = std::time::Duration::from_millis(400);

/// Sidebar date filter. Serialized because it seeds `default_filters`.
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

/// Whether to search document content or file names.
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

/// Result ordering.
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

/// Stable Iced widget id for the search box, so focus survives re-renders.
#[must_use]
pub fn get_search_input_id() -> Id {
    static ID: std::sync::OnceLock<Id> = std::sync::OnceLock::new();
    ID.get_or_init(Id::unique).clone()
}

/// Stable Iced subscription id for indexing progress.
#[must_use]
pub fn get_progress_subscription_id() -> Id {
    static ID: std::sync::OnceLock<Id> = std::sync::OnceLock::new();
    ID.get_or_init(Id::unique).clone()
}

/// Formats a byte count with a binary unit.
///
/// Precision falls as magnitude grows: two decimals for GB, one for MB and KB.
/// A byte value below 1 KB is shown exactly, because rounding it to "0.0 KB"
/// would be actively misleading.
#[must_use]
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
/// unwrapping. The release profile uses `panic = "abort"`, so a panic on this
/// path would terminate the process with no chance to report anything.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn compact(s: &str) -> CompactString {
        CompactString::from(s)
    }

    #[test]
    fn byte_counts_below_a_kilobyte_are_exact() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1), "1 B");
        assert_eq!(format_size(1023), "1023 B");
    }

    #[test]
    fn units_pick_up_at_the_right_thresholds() {
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn gibibytes_keep_two_decimals() {
        // 1.5 GiB exactly: the extra precision matters at this magnitude because
        // files are commonly compared in whole GiB.
        let gb = 1024u64 * 1024 * 1024;
        assert_eq!(format_size(gb + gb / 2), "1.50 GB");
    }

    #[test]
    fn absurdly_large_sizes_do_not_overflow() {
        // `u64::MAX` is ~16 EiB; it must format rather than panic or wrap.
        let out = format_size(u64::MAX);
        assert!(out.ends_with(" GB"), "got: {out}");
    }

    #[test]
    fn timestamps_outside_the_representable_range_render_unknown() {
        // `jiff` cannot represent arbitrarily large seconds. The release profile
        // aborts on panic, so this must not unwrap.
        assert_eq!(format_date(u64::MAX), "Unknown");
        assert_eq!(format_date(1_000_000_000_000_000), "Unknown");
    }

    #[test]
    fn epoch_formats_successfully() {
        assert_ne!(format_date(0), "Unknown");
    }

    #[test]
    fn file_item_falls_back_to_the_file_name_when_a_document_has_no_title() {
        let converted = FileItem::from(SearchResult {
            file_path: "C:\\docs\\report.pdf".to_string(),
            score: 2.0,
            title: None,
            extension: Some(compact("pdf")),
            modified: Some(1),
            size: Some(2),
            matched_terms: vec![],
            snippets: vec![],
        });
        assert_eq!(converted.title, "report.pdf");
        assert_eq!(converted.extension.as_deref(), Some("pdf"));
    }

    #[test]
    fn a_document_title_wins_over_the_file_name() {
        let converted = FileItem::from(SearchResult {
            file_path: "C:\\docs\\report.pdf".to_string(),
            score: 1.0,
            title: Some(compact("Quarterly Report")),
            extension: Some(compact("pdf")),
            modified: None,
            size: None,
            matched_terms: vec![],
            snippets: vec![],
        });
        assert_eq!(converted.title, "Quarterly Report");
    }

    #[test]
    fn filename_matches_carry_no_score_size_or_mtime() {
        let converted = FileItem::from(crate::indexer::filename_index::FilenameSearchResult {
            file_path: "C:\\src\\main.rs".to_string(),
            file_name: CompactString::from("main.rs"),
        });
        assert_eq!(converted.title, "main.rs");
        assert_eq!(converted.extension.as_deref(), Some("rs"));
        assert!(converted.size.is_none());
        assert!(converted.modified.is_none());
        assert!(converted.snippets.is_empty());
    }
}
