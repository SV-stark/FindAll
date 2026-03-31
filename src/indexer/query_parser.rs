use regex::Regex;
use std::sync::OnceLock;

static OPERATOR_REGEX: OnceLock<Regex> = OnceLock::new();

/// Parsed query with operators and search terms
#[derive(Debug, Clone)]
pub struct ParsedQuery {
    /// The original text query (for Tantivy)
    pub text_query: String,
    /// Extension filter (e.g., "pdf", "docx")
    pub extension: Option<String>,
    /// Path filter (search in specific path)
    pub path_filter: Option<String>,
    /// Title filter
    pub title_filter: Option<String>,
    /// Size filters
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    /// Whether fuzzy matching is enabled
    pub fuzzy: bool,
    pub case_sensitive: bool,
}

impl ParsedQuery {
    #[must_use]
    pub fn new(query: &str, case_sensitive: bool) -> Self {
        Self::parse(query, case_sensitive)
    }

    #[allow(clippy::too_many_lines)]
    fn parse(input: &str, case_sensitive: bool) -> Self {
        let mut extension = None;
        let mut path_filter = None;
        let mut title_filter = None;
        let mut min_size = None;
        let mut max_size = None;
        let fuzzy = true;

        // Parse operators: ext:pdf, path:docs, title:report, size:>1MB
        let operator_regex = OPERATOR_REGEX.get_or_init(|| {
            Regex::new(r#"(?i)(ext|path|title|size):(?:"([^"]*)"|(\S+))"#).unwrap()
        });

        let size_regex = Regex::new(r"(?i)^([<>]?)(\d+(?:\.\d+)?)(MB|KB|GB|B)?$").unwrap();

        let mut remaining = input.to_string();

        // Process all operators
        for cap in operator_regex.captures_iter(input) {
            let operator = cap
                .get(1)
                .map(|m| m.as_str().to_lowercase())
                .unwrap_or_default();
            let value = cap
                .get(2)
                .map(|m| m.as_str().to_string()) // Quoted value
                .or_else(|| cap.get(3).map(|m| m.as_str().to_string())) // Unquoted value
                .unwrap_or_default();

            match operator.as_str() {
                "ext" => {
                    extension = Some(value.trim_start_matches('.').to_lowercase());
                    if let Some(m) = cap.get(0) {
                        remaining = remaining.replace(m.as_str(), "");
                    }
                }
                "path" => {
                    path_filter = Some(if case_sensitive {
                        value
                    } else {
                        value.to_lowercase()
                    });
                    if let Some(m) = cap.get(0) {
                        remaining = remaining.replace(m.as_str(), "");
                    }
                }
                "title" => {
                    title_filter = Some(if case_sensitive {
                        value
                    } else {
                        value.to_lowercase()
                    });
                    if let Some(m) = cap.get(0) {
                        remaining = remaining.replace(m.as_str(), "");
                    }
                }
                "size" => {
                    if let Some(scap) = size_regex.captures(&value) {
                        let op = scap.get(1).map_or("", |m| m.as_str());
                        if let Some(num_str) = scap.get(2) {
                            if let Ok(num) = num_str.as_str().parse::<f64>() {
                                let multiplier = scap.get(3).map_or(1, |m| {
                                    match m.as_str().to_uppercase().as_str() {
                                        "GB" => 1024 * 1024 * 1024,
                                        "MB" => 1024 * 1024,
                                        "KB" => 1024,
                                        _ => 1,
                                    }
                                });

                                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                                let bytes = (num * f64::from(multiplier)).round() as u64;
                                match op {
                                    ">" => min_size = Some(bytes + 1),
                                    "<" => max_size = Some(bytes.saturating_sub(1)),
                                    _ => {
                                        min_size = Some(bytes);
                                        max_size = Some(bytes + 1);
                                    }
                                }
                            }
                        }
                    }
                    if let Some(m) = cap.get(0) {
                        remaining = remaining.replace(m.as_str(), "");
                    }
                }
                _ => {}
            }
        }

        // Clean up remaining text for full-text search
        let text_query = remaining
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string();

        Self {
            text_query: if text_query.is_empty() {
                "*".to_string()
            } else {
                text_query
            },
            extension,
            path_filter,
            title_filter,
            min_size,
            max_size,
            fuzzy,
            case_sensitive,
        }
    }

    /// Check if a path matches the extension filter
    #[must_use]
    pub fn matches_extension(&self, path: &str) -> bool {
        self.extension.as_ref().map_or(true, |ext| {
            let path_lower = path.to_lowercase();
            path_lower.ends_with(&format!(".{ext}"))
        })
    }

    /// Check if a path matches the path filter
    #[must_use]
    pub fn matches_path(&self, path: &str) -> bool {
        self.path_filter.as_ref().map_or(true, |filter| {
            if self.case_sensitive {
                path.contains(filter)
            } else {
                path.to_lowercase().contains(filter)
            }
        })
    }

    /// Check if a title matches the title filter
    #[must_use]
    pub fn matches_title(&self, title: Option<&str>) -> bool {
        self.title_filter.as_ref().map_or(true, |filter| {
            title.is_some_and(|t| {
                if self.case_sensitive {
                    t.contains(filter)
                } else {
                    t.to_lowercase().contains(filter)
                }
            })
        })
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

    if terms.is_empty() && parsed.text_query == "*" {
        terms.push("*".to_string());
    }

    // Also add title filter terms
    if let Some(title) = parsed.title_filter {
        terms.push(title);
    }

    terms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ext_operator() {
        let query = "ext:pdf report";
        let parsed = ParsedQuery::new(query, false);
        assert_eq!(parsed.extension, Some("pdf".to_string()));
        assert_eq!(parsed.text_query, "report");
    }

    #[test]
    fn test_parse_path_operator() {
        let query = "path:documents important";
        let parsed = ParsedQuery::new(query, false);
        assert_eq!(parsed.path_filter, Some("documents".to_string()));
        assert_eq!(parsed.text_query, "important");
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
        assert_eq!(parsed.extension, Some("pdf".to_string()));
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
    fn test_extract_highlight_terms() {
        let terms = extract_highlight_terms("ext:pdf report title:annual", false);
        assert!(terms.contains(&"report".to_string()));
        assert!(terms.contains(&"annual".to_string()));
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
                        assert_eq!(parsed.extension, Some(expected));
                    },
                    "path" => assert_eq!(parsed.path_filter, Some(val.to_lowercase())),
                    "title" => assert_eq!(parsed.title_filter, Some(val.to_lowercase())),
                    "size" => {},
                    _ => unreachable!(),
                }
            }
        }
    }
}
