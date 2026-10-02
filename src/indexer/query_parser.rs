use regex::Regex;
use std::sync::OnceLock;

static OPERATOR_REGEX: OnceLock<Regex> = OnceLock::new();
static SIZE_REGEX: OnceLock<Regex> = OnceLock::new();

/// A single `operator:value` token extracted from a raw query string.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OperatorToken<'a> {
    operator: &'a str,
    value: &'a str,
    /// Byte range of the whole `operator:value` token inside the raw query.
    span: std::ops::Range<usize>,
}

/// Splits `input` into operator tokens and the leftover free-text fragments.
///
/// The leftover text is rebuilt by concatenating the gaps *between* operator
/// spans, rather than calling `String::replace` per operator. The previous
/// approach was O(n^2) and — worse — `replace` removes the first *textual*
/// occurrence, so a query like `ext:pdf notes ext:pdf` could strip an unrelated
/// word that happened to equal the operator text.
fn split_operators(input: &str) -> (Vec<OperatorToken<'_>>, String) {
    let operator_regex = OPERATOR_REGEX.get_or_init(|| {
        // Constant pattern; a compile failure here is a programming error and is
        // caught by `test_operator_regex_compiles`.
        Regex::new(r#"(?i)(ext|path|title|size|modified|date):(?:"([^"]*)"|(\S+))"#)
            .expect("OPERATOR_REGEX must be a valid regex")
    });

    let mut tokens = Vec::new();
    let mut text = String::with_capacity(input.len());
    let mut cursor = 0usize;

    for cap in operator_regex.captures_iter(input) {
        let whole = cap.get(0).expect("group 0 of a match is always present");
        let value = cap
            .get(2)
            .or_else(|| cap.get(3))
            .map_or("", |m| m.as_str());

        if whole.start() < cursor {
            // Overlapping match (should not happen with `captures_iter`, but
            // guard so the leftover text can never be built out of order).
            continue;
        }

        text.push_str(&input[cursor..whole.start()]);
        tokens.push(OperatorToken {
            operator: cap.get(1).map_or("", |m| m.as_str()),
            value,
            span: whole.start()..whole.end(),
        });
        cursor = whole.end();
    }

    text.push_str(&input[cursor..]);
    (tokens, text)
}

/// Parsed query with operators and search terms
#[derive(Debug, Clone, Default)]
pub struct ParsedQuery {
    /// Free-text query with all `operator:value` tokens removed
    pub text_query: String,
    /// Extension filter(s) from `ext:`. Repeated `ext:` tokens are OR-ed together.
    pub extensions: Vec<String>,
    /// Path substring filter from `path:` (lowercased when `!case_sensitive`)
    pub path_filter: Option<String>,
    /// Title substring filter from `title:`
    pub title_filter: Option<String>,
    /// Size filters from `size:`
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    /// Date filters from `modified:` / `date:`
    pub min_modified: Option<u64>,
    pub max_modified: Option<u64>,
    pub case_sensitive: bool,
}

impl ParsedQuery {
    #[must_use]
    pub fn new(query: &str, case_sensitive: bool) -> Self {
        Self::parse(query, case_sensitive)
    }

    fn parse(input: &str, case_sensitive: bool) -> Self {
        let (tokens, leftover) = split_operators(input);

        let size_regex = SIZE_REGEX.get_or_init(|| {
            // Constant pattern; a compile failure is a programming error and is
            // covered by `test_size_regex_compiles`.
            Regex::new(r"(?i)^([<>]?)(\d+(?:\.\d+)?)(MB|KB|GB|B)?$")
                .expect("SIZE_REGEX must be a valid regex")
        });

        let mut extensions = Vec::new();
        let mut path_filter: Option<String> = None;
        let mut title_filter: Option<String> = None;
        let mut min_size: Option<u64> = None;
        let mut max_size: Option<u64> = None;
        let mut min_modified: Option<u64> = None;
        let mut max_modified: Option<u64> = None;

        for token in &tokens {
            match token.operator.to_ascii_lowercase().as_str() {
                "ext" => {
                    let ext = token.value.trim_start_matches('.').to_lowercase();
                    if !ext.is_empty() && !extensions.contains(&ext) {
                        extensions.push(ext);
                    }
                }
                "path" => {
                    path_filter = Some(if case_sensitive {
                        token.value.to_string()
                    } else {
                        token.value.to_lowercase()
                    });
                }
                "title" => {
                    title_filter = Some(if case_sensitive {
                        token.value.to_string()
                    } else {
                        token.value.to_lowercase()
                    });
                }
                "size" => {
                    if let Some(scap) = size_regex.captures(token.value)
                        && let Some(num_str) = scap.get(2)
                        && let Ok(num) = num_str.as_str().parse::<f64>()
                    {
                        let op = scap.get(1).map_or("", |m| m.as_str());
                        let multiplier: u64 = scap.get(3).map_or(1, |m| {
                            match m.as_str().to_ascii_uppercase().as_str() {
                                "GB" => 1024 * 1024 * 1024,
                                "MB" => 1024 * 1024,
                                "KB" => 1024,
                                _ => 1,
                            }
                        });

                        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                        let bytes = (num * multiplier as f64).round() as u64;
                        match op {
                            // Repeated `>`/`size:` keep the strictest bound. A
                            // contradictory pair (`size:>10MB size:<1MB`) keeps
                            // both bounds as written, which yields an empty
                            // result set — the honest answer to a query that
                            // asks for the impossible.
                            ">" => min_size = Some(min_size.map_or(bytes, |m| m.max(bytes))),
                            "<" => max_size = Some(max_size.map_or(bytes, |m| m.min(bytes))),
                            _ => {
                                min_size = Some(bytes);
                                max_size = Some(bytes);
                            }
                        }
                    }
                }
                "modified" | "date" => {
                    if let Some((min_ts, max_ts)) = parse_date_range(token.value) {
                        min_modified = Some(min_modified.map_or(min_ts, |m| m.min(min_ts)));
                        max_modified = Some(max_modified.map_or(max_ts, |m| m.max(max_ts)));
                    }
                }
                _ => {}
            }
        }

        let text_query = leftover.split_whitespace().collect::<Vec<_>>().join(" ");

        Self {
            text_query: if text_query.is_empty() {
                "*".to_string()
            } else {
                text_query
            },
            extensions,
            path_filter,
            title_filter,
            min_size,
            max_size,
            min_modified,
            max_modified,
            case_sensitive,
        }
    }

    /// Check if a timestamp matches the modified date filter
    #[must_use]
    pub const fn matches_modified(&self, modified: Option<u64>) -> bool {
        if self.min_modified.is_none() && self.max_modified.is_none() {
            return true;
        }
        let Some(m) = modified else {
            return false;
        };
        if let Some(min) = self.min_modified
            && m < min
        {
            return false;
        }
        if let Some(max) = self.max_modified
            && m > max
        {
            return false;
        }
        true
    }

    /// Check if a path matches the extension filter(s).
    ///
    /// With no `ext:` filter this returns `true`; otherwise the path's
    /// extension must be one of the requested ones (OR semantics).
    #[must_use]
    pub fn matches_extension(&self, path: &str) -> bool {
        if self.extensions.is_empty() {
            return true;
        }
        let ext = path
            .rsplit_once('.')
            .map_or("", |(_, e)| e)
            .to_ascii_lowercase();
        self.extensions.contains(&ext)
    }

    /// Check if a path matches the path filter
    #[must_use]
    pub fn matches_path(&self, path: &str) -> bool {
        self.path_filter.as_ref().is_none_or(|filter| {
            if self.case_sensitive {
                path.contains(filter)
            } else {
                path.to_ascii_lowercase().contains(filter)
            }
        })
    }

    /// Check if a title matches the title filter
    #[must_use]
    pub fn matches_title(&self, title: Option<&str>) -> bool {
        self.title_filter.as_ref().is_none_or(|filter| {
            title.is_some_and(|t| {
                if self.case_sensitive {
                    t.contains(filter)
                } else {
                    t.to_lowercase().contains(filter)
                }
            })
        })
    }

    /// True when no post-retrieval filtering is required, letting the searcher
    /// skip the per-document `path:`/`title:` checks entirely.
    #[must_use]
    pub const fn needs_post_filter(&self) -> bool {
        self.path_filter.is_some() || self.title_filter.is_some()
    }
}

/// Extract search terms for highlighting from a query
#[must_use]
pub fn extract_highlight_terms(query: &str, case_sensitive: bool) -> Vec<String> {
    let parsed = ParsedQuery::new(query, case_sensitive);

    let mut terms: Vec<String> = parsed
        .text_query
        .split_whitespace()
        .filter(|t| *t != "*")
        .map(|t| {
            if case_sensitive {
                t.to_string()
            } else {
                t.to_lowercase()
            }
        })
        .collect();

    // `path:`/`title:` filters are applied as post-filters, so their terms still
    // need highlighting when they are the only thing the user typed.
    for filter in [parsed.title_filter.as_ref(), parsed.path_filter.as_ref()]
        .into_iter()
        .flatten()
    {
        if !terms.iter().any(|t| t == filter) {
            terms.push(filter.clone());
        }
    }

    terms
}

/// Parses natural language and range date expressions into timestamp intervals using `jiff`.
#[must_use]
pub fn parse_date_range(val: &str) -> Option<(u64, u64)> {
    let now = jiff::Zoned::now();
    let val_lower = val.to_lowercase();

    match val_lower.as_str() {
        "today" => {
            let start_of_day = now
                .date()
                .at(0, 0, 0, 0)
                .to_zoned(now.time_zone().clone())
                .ok()?;
            let min_ts = u64::try_from(start_of_day.timestamp().as_second()).ok()?;
            let max_ts = u64::try_from(now.timestamp().as_second()).ok()?;
            Some((min_ts, max_ts))
        }
        "yesterday" => {
            let start_of_today = now
                .date()
                .at(0, 0, 0, 0)
                .to_zoned(now.time_zone().clone())
                .ok()?;
            let start_of_yesterday = start_of_today
                .checked_sub(jiff::SignedDuration::from_secs(86400))
                .ok()?;
            let min_ts = u64::try_from(start_of_yesterday.timestamp().as_second()).ok()?;
            let max_ts = u64::try_from(start_of_today.timestamp().as_second()).ok()?;
            Some((min_ts, max_ts))
        }
        "7d" | "week" | "last 7 days" => {
            let start = now
                .checked_sub(jiff::SignedDuration::from_secs(7 * 86400))
                .ok()?;
            let min_ts = u64::try_from(start.timestamp().as_second()).ok()?;
            let max_ts = u64::try_from(now.timestamp().as_second()).ok()?;
            Some((min_ts, max_ts))
        }
        "30d" | "month" | "last 30 days" => {
            let start = now
                .checked_sub(jiff::SignedDuration::from_secs(30 * 86400))
                .ok()?;
            let min_ts = u64::try_from(start.timestamp().as_second()).ok()?;
            let max_ts = u64::try_from(now.timestamp().as_second()).ok()?;
            Some((min_ts, max_ts))
        }
        _ => {
            if let Some((start_str, end_str)) = val.split_once("..") {
                let start_date: jiff::civil::Date = start_str.trim().parse().ok()?;
                let end_date: jiff::civil::Date = end_str.trim().parse().ok()?;
                let start_zoned = start_date
                    .at(0, 0, 0, 0)
                    .to_zoned(now.time_zone().clone())
                    .ok()?;
                let end_zoned = end_date
                    .at(23, 59, 59, 0)
                    .to_zoned(now.time_zone().clone())
                    .ok()?;
                let min_ts = u64::try_from(start_zoned.timestamp().as_second()).ok()?;
                let max_ts = u64::try_from(end_zoned.timestamp().as_second()).ok()?;
                Some((min_ts, max_ts))
            } else if let Ok(single_date) = val.parse::<jiff::civil::Date>() {
                let start_zoned = single_date
                    .at(0, 0, 0, 0)
                    .to_zoned(now.time_zone().clone())
                    .ok()?;
                let end_zoned = single_date
                    .at(23, 59, 59, 0)
                    .to_zoned(now.time_zone().clone())
                    .ok()?;
                let min_ts = u64::try_from(start_zoned.timestamp().as_second()).ok()?;
                let max_ts = u64::try_from(end_zoned.timestamp().as_second()).ok()?;
                Some((min_ts, max_ts))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ext_operator() {
        let query = "ext:pdf report";
        let parsed = ParsedQuery::new(query, false);
        assert_eq!(parsed.extensions, vec!["pdf".to_string()]);
        assert_eq!(parsed.text_query, "report");
    }

    #[test]
    fn test_repeated_ext_operators_accumulate() {
        let parsed = ParsedQuery::new("report ext:pdf ext:docx ext:PDF", false);
        assert_eq!(parsed.extensions, vec!["pdf".to_string(), "docx".to_string()]);
        assert_eq!(parsed.text_query, "report");
    }

    #[test]
    fn test_operator_removal_does_not_eat_free_text() {
        // Regression: `String::replace` per operator stripped the first textual
        // match anywhere, so repeated operators corrupted the free-text query.
        let parsed = ParsedQuery::new("ext:pdf ext:pdf notes", false);
        assert_eq!(parsed.text_query, "notes");

        let parsed = ParsedQuery::new("size:>1MB size:>1MB notes", false);
        assert_eq!(parsed.text_query, "notes");
    }

    #[test]
    fn test_quoted_operator_values() {
        let parsed = ParsedQuery::new("notes path:\"My Documents\" title:\"Q3 Report\"", false);
        assert_eq!(parsed.path_filter.as_deref(), Some("my documents"));
        assert_eq!(parsed.title_filter.as_deref(), Some("q3 report"));
        assert_eq!(parsed.text_query, "notes");
    }

    #[test]
    fn test_contradictory_size_bounds_are_preserved() {
        // `size:>10MB size:<1MB` asks for the impossible. Both bounds are kept so
        // the query returns nothing rather than silently returning something the
        // user did not ask for.
        let parsed = ParsedQuery::new("x size:>10MB size:<1MB", false);
        assert_eq!(parsed.min_size, Some(10_485_760));
        assert_eq!(parsed.max_size, Some(1_048_576));
    }

    #[test]
    fn test_repeated_bounds_keep_the_strictest() {
        let parsed = ParsedQuery::new("x size:>1MB size:>4MB", false);
        assert_eq!(parsed.min_size, Some(4_194_304));

        let parsed = ParsedQuery::new("x size:<8MB size:<2MB", false);
        assert_eq!(parsed.max_size, Some(2_097_152));
    }

    #[test]
    fn test_bare_query_becomes_match_all() {
        let parsed = ParsedQuery::new("   ", false);
        assert_eq!(parsed.text_query, "*");
        assert!(!parsed.needs_post_filter());
    }

    #[test]
    fn test_operator_regex_compiles() {
        OPERATOR_REGEX
            .get_or_init(|| {
                Regex::new(r#"(?i)(ext|path|title|size|modified|date):(?:"([^"]*)"|(\S+))"#)
                    .expect("OPERATOR_REGEX must be a valid regex")
            });
        assert!(!OPERATOR_REGEX.get().unwrap().as_str().is_empty());
    }

    #[test]
    fn test_size_regex_compiles() {
        SIZE_REGEX.get_or_init(|| {
            Regex::new(r"(?i)^([<>]?)(\d+(?:\.\d+)?)(MB|KB|GB|B)?$")
                .expect("SIZE_REGEX must be a valid regex")
        });
        assert!(!SIZE_REGEX.get().unwrap().as_str().is_empty());
    }

    #[test]
    fn test_parse_path_operator() {
        let query = "path:documents important";
        let parsed = ParsedQuery::new(query, false);
        assert_eq!(parsed.path_filter, Some("documents".to_string()));
        assert_eq!(parsed.text_query, "important");
        assert!(parsed.needs_post_filter());
    }

    #[test]
    fn test_parse_size_operators() {
        let query = "size:>1MB document";
        let parsed = ParsedQuery::new(query, false);
        assert_eq!(parsed.min_size, Some(1_048_576));
        assert_eq!(parsed.text_query, "document");
    }

    #[test]
    fn test_multiple_operators() {
        let query = "ext:pdf path:reports annual size:<10MB";
        let parsed = ParsedQuery::new(query, false);
        assert_eq!(parsed.extensions, vec!["pdf".to_string()]);
        assert_eq!(parsed.path_filter, Some("reports".to_string()));
        assert_eq!(parsed.max_size, Some(10_485_760));
        assert_eq!(parsed.text_query, "annual");
    }

    #[test]
    fn test_matches_extension() {
        let parsed = ParsedQuery::new("ext:pdf", false);
        assert!(parsed.matches_extension("file.pdf"));
        assert!(parsed.matches_extension("FILE.PDF"));
        assert!(!parsed.matches_extension("file.txt"));
        assert!(!parsed.matches_extension("pdf"));

        let parsed = ParsedQuery::new("ext:pdf ext:docx", false);
        assert!(parsed.matches_extension("a.docx"));
        assert!(!parsed.matches_extension("a.txt"));

        // No filter at all matches everything.
        let parsed = ParsedQuery::new("anything", false);
        assert!(parsed.matches_extension("a.txt"));
    }

    #[test]
    fn test_matches_path() {
        let parsed = ParsedQuery::new("path:reports", false);
        assert!(parsed.matches_path("/home/user/reports/annual.pdf"));
        assert!(!parsed.matches_path("/home/user/documents/annual.pdf"));
    }

    #[test]
    fn test_matches_title() {
        let parsed = ParsedQuery::new("title:annual", false);
        assert!(parsed.matches_title(Some("Annual Report")));
        assert!(!parsed.matches_title(Some("Monthly Report")));
        assert!(!parsed.matches_title(None));
    }

    #[test]
    fn test_needs_post_filter_only_for_substring_operators() {
        assert!(!ParsedQuery::new("ext:pdf size:>1MB modified:today", false).needs_post_filter());
        assert!(ParsedQuery::new("path:docs", false).needs_post_filter());
        assert!(ParsedQuery::new("title:report", false).needs_post_filter());
    }

    #[test]
    fn test_extract_highlight_terms() {
        let terms = extract_highlight_terms("ext:pdf report title:annual", false);
        assert!(terms.contains(&"report".to_string()));
        assert!(terms.contains(&"annual".to_string()));
        assert!(!terms.iter().any(|t| t.starts_with("ext:")));
    }

    #[cfg(test)]
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn test_parse_no_panic(s in "\\PC*") {
                let _ = ParsedQuery::new(&s, false);
            }

            #[test]
            fn test_parse_with_known_operators(
                op in "ext|path|title|size",
                val in "[a-zA-Z0-9_.-]+",
                text in "\\PC*"
            ) {
                let input = format!("{op}:{val} {text}");
                let parsed = ParsedQuery::new(&input, false);

                match op.as_str() {
                    "ext" => {
                        let expected = val.trim_start_matches('.').to_lowercase();
                        // `val` may be a bare "." which normalises to empty; the
                        // parser drops empty extensions.
                        if !expected.is_empty() {
                            assert!(
                                parsed.extensions.contains(&expected),
                                "expected {expected:?} in {:?}",
                                parsed.extensions
                            );
                        }
                    },
                    "path" => assert_eq!(parsed.path_filter, Some(val.to_lowercase())),
                    "title" => assert_eq!(parsed.title_filter, Some(val.to_lowercase())),
                    "size" => {},
                    _ => unreachable!("op is restricted to the four operators above"),
                }
            }
        }
    }
}
