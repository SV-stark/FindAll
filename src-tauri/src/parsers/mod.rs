use crate::error::{FlashError, Result};
use std::ffi::OsStr;
use std::path::Path;

pub mod docx;
pub mod epub;
pub mod odt;
pub mod pdf;
pub mod text;

#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub path: String,
    pub content: String,
    pub title: Option<String>,
}

/// Parse file without allocating - uses byte comparison
fn extension_matches(ext: &OsStr, target: &str) -> bool {
    // Case-insensitive comparison without allocation
    if let Some(ext_bytes) = ext.to_str().map(|s| s.as_bytes()) {
        if ext_bytes.len() != target.len() {
            return false;
        }
        ext_bytes
            .iter()
            .zip(target.bytes())
            .all(|(a, b)| a.eq_ignore_ascii_case(&b))
    } else {
        false
    }
}

/// Detect file type and route to appropriate parser
/// Optimized to avoid string allocations
pub fn parse_file(path: &Path) -> Result<ParsedDocument> {
    let extension = path.extension().unwrap_or_default();

    // Check DOCX first (most common office format)
    if extension_matches(extension, "docx") {
        return docx::parse_docx(path);
    }

    // Check other office formats
    if extension_matches(extension, "odt") {
        return odt::parse_odt(path);
    }
    if extension_matches(extension, "epub") {
        return epub::parse_epub(path);
    }
    if extension_matches(extension, "pdf") {
        return pdf::parse_pdf(path);
    }

    // Check text-based formats using a static lookup
    if is_text_format(extension) {
        return text::parse_text(path);
    }

    // If we got here, the format is not supported
    let ext_str = extension.to_string_lossy().to_string();
    Err(FlashError::UnsupportedFormat(ext_str))
}

/// Check if extension is a supported text format
/// Uses a static array for O(1) lookup with minimal comparisons
#[inline]
fn is_text_format(ext: &OsStr) -> bool {
    // Common text extensions - grouped by frequency for cache efficiency
    const TEXT_EXTENSIONS: &[&[u8]] = &[
        b"txt",
        b"md",
        // Code files - web
        b"js",
        b"ts",
        b"json",
        b"html",
        b"css",
        b"xml",
        // Rust ecosystem
        b"rs",
        b"toml",
        // Python
        b"py",
        // Java ecosystem
        b"java",
        b"kt",
        // C/C++
        b"c",
        b"cpp",
        b"h",
        b"hpp",
        // Go
        b"go",
        // Ruby/PHP
        b"rb",
        b"php",
        // Swift/Dart
        b"swift",
        b"dart",
        // Config/data
        b"yaml",
        b"yml",
        b"ini",
        b"conf",
        b"env",
        // Shell scripts
        b"sh",
        b"bat",
        b"ps1",
        // SQL and data
        b"sql",
        b"r",
        b"log",
        // Web frameworks
        b"svelte",
        b"vue",
        // Stylesheets
        b"scss",
        b"less",
        // Other formats
        b"svg",
        b"ics",
        b"vcf",
        b"cmake",
        b"gradle",
        b"properties",
        b"proto",
        b"dockerfile",
    ];

    if let Some(ext_bytes) = ext.to_str().map(|s| s.as_bytes()) {
        // Linear search with case-insensitive comparison
        // This is fast enough for ~40 extensions and doesn't require sorting
        for target in TEXT_EXTENSIONS {
            if ext_bytes.len() == target.len() {
                let matches = ext_bytes
                    .iter()
                    .zip(target.iter())
                    .all(|(a, b)| a.eq_ignore_ascii_case(b));
                if matches {
                    return true;
                }
            }
        }
        false
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_extension_matches() {
        assert!(extension_matches(OsStr::new("docx"), "docx"));
        assert!(extension_matches(OsStr::new("DOCX"), "docx"));
        assert!(extension_matches(OsStr::new("Docx"), "docx"));
        assert!(!extension_matches(OsStr::new("pdf"), "docx"));
    }

    #[test]
    fn test_is_text_format() {
        assert!(is_text_format(OsStr::new("txt")));
        assert!(is_text_format(OsStr::new("TXT")));
        assert!(is_text_format(OsStr::new("rs")));
        assert!(is_text_format(OsStr::new("js")));
        assert!(!is_text_format(OsStr::new("exe")));
        assert!(!is_text_format(OsStr::new("docx")));
    }

    #[test]
    fn test_parse_file_txt() {
        // This will be implemented with test fixtures
        let path = PathBuf::from("tests/fixtures/sample.txt");
        // Test implementation here
    }
}
