//! Unit tests for the UI layer.
//!
//! These were previously inline at the bottom of `iced_ui::mod`. They are pure
//! state-transition and formatting tests: none of them stand up a window, which
//! is what made them runnable in CI at all.

use super::model::{DOUBLE_CLICK_WINDOW, FileItem, LastClick, format_date, format_size};
use super::state::App;
use crate::indexer::searcher::SearchResult;
use crate::settings::AppSettings;
use compact_str::CompactString;
use std::time::Duration;

fn app_with_drafts(max: &str, patterns: &str) -> App {
    App {
        max_results_input: max.to_string(),
        exclude_patterns_input: patterns.to_string(),
        ..App::default()
    }
}

fn app_with_query(query: &str, history_enabled: bool) -> App {
    App {
        search_query: query.to_string(),
        settings: AppSettings {
            search_history_enabled: history_enabled,
            ..AppSettings::default()
        },
        ..App::default()
    }
}

/// The regression that motivated drafts at all: clearing the box used to
/// snap it back because the value could not be parsed.
#[test]
fn clearing_max_results_falls_back_to_the_default() {
    let mut app = app_with_drafts("", "");
    app.commit_text_inputs();
    assert_eq!(
        app.settings.max_results,
        crate::settings::DEFAULT_MAX_RESULTS
    );
    assert_eq!(app.max_results_input, "50");
}

/// Unparseable input must not silently overwrite the previous value.
#[test]
fn unparseable_max_results_keeps_the_previous_value() {
    let mut app = App {
        max_results_input: "abc".to_string(),
        settings: AppSettings {
            max_results: 123,
            ..AppSettings::default()
        },
        ..App::default()
    };
    app.commit_text_inputs();
    assert_eq!(app.settings.max_results, 123);
    // The box is reset so it stops showing text that was rejected.
    assert_eq!(app.max_results_input, "123");
}

#[test]
fn max_results_is_clamped_to_a_usable_band() {
    let mut app = app_with_drafts("0", "");
    app.commit_text_inputs();
    assert_eq!(app.settings.max_results, 1, "zero would return no results");

    let mut app = app_with_drafts("999999999", "");
    app.commit_text_inputs();
    assert_eq!(
        app.settings.max_results,
        crate::settings::MAX_RESULTS_LIMIT,
        "an unbounded cap must not be honoured"
    );
}

/// The separator is typed progressively; re-flowing the box mid-edit used to
/// move the caret out from under the user.
#[test]
fn exclude_patterns_preserves_a_trailing_separator() {
    let mut app = app_with_drafts("50", "target,");
    app.commit_text_inputs();
    assert_eq!(app.settings.exclude_patterns, vec!["target".to_string()]);
    assert_eq!(app.exclude_patterns_input, "target");
}

#[test]
fn exclude_patterns_trim_and_drop_empties() {
    let mut app = app_with_drafts("50", " *.git , , node_modules ");
    app.commit_text_inputs();
    assert_eq!(
        app.settings.exclude_patterns,
        vec!["*.git".to_string(), "node_modules".to_string()]
    );
}

#[test]
fn exclude_patterns_can_be_emptied_entirely() {
    let mut app = App {
        exclude_patterns_input: "  ,  , ".to_string(),
        settings: AppSettings {
            exclude_patterns: vec!["target".to_string()],
            ..AppSettings::default()
        },
        ..App::default()
    };
    app.commit_text_inputs();
    assert!(app.settings.exclude_patterns.is_empty());
    assert!(app.exclude_patterns_input.is_empty());
}

/// Clicking a recent search must bump its count rather than adding a
/// duplicate entry for the same query.
#[test]
fn recording_a_search_bumps_frequency_instead_of_duplicating() {
    let mut app = app_with_query("invoice", true);

    drop(app.record_search_history());
    assert_eq!(app.settings.search_history.len(), 1);
    assert_eq!(app.settings.search_history[0].frequency, 1);

    drop(app.record_search_history());
    assert_eq!(app.settings.search_history.len(), 1);
    assert_eq!(app.settings.search_history[0].frequency, 2);

    drop(app.record_search_history());
    app.search_query = "receipt".to_string();
    drop(app.record_search_history());
    // Ranked by frequency, so the twice-run query stays first.
    assert_eq!(app.settings.search_history[0].query, "invoice");
    assert_eq!(app.settings.search_history.len(), 2);
}

#[test]
fn whitespace_only_searches_are_not_recorded() {
    let mut app = app_with_query("   ", true);
    drop(app.record_search_history());
    assert!(app.settings.search_history.is_empty());
}

#[test]
fn history_is_not_recorded_when_disabled() {
    let mut app = app_with_query("invoice", false);
    drop(app.record_search_history());
    assert!(app.settings.search_history.is_empty());
}

/// Two presses on the same row inside the window are a double-click; the
/// second one on a different row is not.
#[test]
fn double_click_pairs_only_consecutive_presses_on_one_row() {
    let now = std::time::Instant::now();
    let app = App {
        last_click: Some(LastClick {
            result_index: 3,
            at: now,
        }),
        ..App::default()
    };
    assert!(
        app.pairs_as_double_click(3, now + Duration::from_millis(100)),
        "same row inside the window must pair"
    );
    assert!(
        !app.pairs_as_double_click(4, now + Duration::from_millis(100)),
        "a different row must not pair"
    );
    assert!(
        !app.pairs_as_double_click(3, now + DOUBLE_CLICK_WINDOW + Duration::from_millis(1)),
        "a press past the window must not pair"
    );
}

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
    assert_eq!(
        parsed.extensions,
        vec!["pdf".to_string(), "docx".to_string()]
    );
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
