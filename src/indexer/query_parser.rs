use memchr::memchr;
use regex::Regex;

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
}

impl ParsedQuery {
    pub fn new(query: &str) -> Self {
        Self::parse(query)
    }

    fn parse(input: &str) -> Self {
        let mut extension = None;
        let mut path_filter = None;
        let mut title_filter = None;
        let mut min_size = None;
        let mut max_size = None;
        let fuzzy = true;

        // Parse operators
        // ext:pdf, path:docs, title:report, size:>1MB, size:<10MB, exact:"phrase"
        let operator_regex = Regex::new(
            r#"(?i)(ext|path|title|size):(?:([<>]?)(\d+(?:\.\d+)?)(MB|KB|GB|B)?|"([^"]*)"|(\S+))"#,
        )
        .unwrap();

        let mut remaining = input.to_string();

        // Process all operators
        for cap in operator_regex.captures_iter(input) {
            let operator = cap
                .get(1)
                .map(|m| m.as_str().to_lowercase())
                .unwrap_or_default();
            let value = cap
                .get(5)
                .map(|m| m.as_str().to_string()) // Quoted value
                .or_else(|| cap.get(6).map(|m| m.as_str().to_string())) // Unquoted value
                .unwrap_or_default();

            match operator.as_str() {
                "ext" => {
                    extension = Some(value.trim_start_matches('.').to_lowercase());
                    remaining = remaining.replace(cap.get(0).unwrap().as_str(), "");
                }
                "path" => {
                    path_filter = Some(value.to_lowercase());
                    remaining = remaining.replace(cap.get(0).unwrap().as_str(), "");
                }
                "title" => {
                    title_filter = Some(value.to_lowercase());
                    remaining = remaining.replace(cap.get(0).unwrap().as_str(), "");
                }
                "size" => {
                    // Handle size operators
                    if let Some(op) = cap.get(2) {
                        let op = op.as_str();
                        if let Some(num_str) = cap.get(3) {
                            if let Ok(num) = num_str.as_str().parse::<f64>() {
                                let multiplier = cap
                                    .get(4)
                                    .map(|m| match m.as_str().to_uppercase().as_str() {
                                        "GB" => 1024 * 1024 * 1024,
                                        "MB" => 1024 * 1024,
                                        "KB" => 1024,
                                        _ => 1,
                                    })
                                    .unwrap_or(1);

                                let bytes = (num * multiplier as f64) as u64;
                                match op {
                                    ">" => min_size = Some(bytes),
                                    "<" => max_size = Some(bytes),
                                    _ => {}
                                }
                            }
                        }
                    } else if let Ok(size_val) = value.parse::<u64>() {
                        // Exact size match (treat as minimum for practical purposes)
                        min_size = Some(size_val);
                        max_size = Some(size_val + 1);
                    }
                    remaining = remaining.replace(cap.get(0).unwrap().as_str(), "");
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
        }
    }

    /// Check if a path matches the extension filter
    pub fn matches_extension(&self, path: &str) -> bool {
        if let Some(ref ext) = self.extension {
            let path_lower = path.to_lowercase();
            path_lower.ends_with(&format!(".{}", ext))
        } else {
            true
        }
    }

    /// Check if a path matches the path filter
    pub fn matches_path(&self, path: &str) -> bool {
        if let Some(ref filter) = self.path_filter {
            path.to_lowercase().contains(filter)
        } else {
            true
        }
    }

    /// Check if a title matches the title filter
    pub fn matches_title(&self, title: Option<&str>) -> bool {
        if let Some(ref filter) = self.title_filter {
            if let Some(t) = title {
                t.to_lowercase().contains(filter)
            } else {
                false
            }
        } else {
            true
        }
    }
}

/// Extract search terms for highlighting from a query
pub fn extract_highlight_terms(query: &str) -> Vec<String> {
    let parsed = ParsedQuery::new(query);

    let mut terms = Vec::new();
    let bytes = parsed.text_query.as_bytes();
    let mut last_end = 0;

    let mut iter = memchr(b' ', bytes);
    while let Some(pos) = iter {
        let term = &bytes[last_end..pos];
        if !term.is_empty() {
            let term_str = String::from_utf8_lossy(term).to_lowercase();
            if !term_str.is_empty() && term_str != "*" {
                terms.push(term_str);
            }
        }
        last_end = pos + 1;
        iter = memchr(b' ', &bytes[last_end..]);
    }

    // Handle last segment
    if last_end < bytes.len() {
        let term = &bytes[last_end..];
        if !term.is_empty() {
            let term_str = String::from_utf8_lossy(term).to_lowercase();
            if !term_str.is_empty() && term_str != "*" {
                terms.push(term_str);
            }
        }
    }

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
        let parsed = ParsedQuery::new(query);
        assert_eq!(parsed.extension, Some("pdf".to_string()));
        assert_eq!(parsed.text_query, "report");
    }

    #[test]
    fn test_parse_path_operator() {
        let query = "path:documents important";
        let parsed = ParsedQuery::new(query);
        assert_eq!(parsed.path_filter, Some("documents".to_string()));
        assert_eq!(parsed.text_query, "important");
    }

    #[test]
    fn test_parse_size_operators() {
        let query = "size:>1MB document";
        let parsed = ParsedQuery::new(query);
        assert_eq!(parsed.min_size, Some(1048576));
        assert_eq!(parsed.text_query, "document");
    }

    #[test]
    fn test_multiple_operators() {
        let query = "ext:pdf path:reports annual size:<10MB";
        let parsed = ParsedQuery::new(query);
        assert_eq!(parsed.extension, Some("pdf".to_string()));
        assert_eq!(parsed.path_filter, Some("reports".to_string()));
        assert_eq!(parsed.max_size, Some(10485760));
        assert_eq!(parsed.text_query, "annual");
    }
}
