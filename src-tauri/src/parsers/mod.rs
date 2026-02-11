use crate::error::{FlashError, Result};
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

/// Detect file type and route to appropriate parser
pub fn parse_file(path: &Path) -> Result<ParsedDocument> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    match extension.as_str() {
        "docx" => docx::parse_docx(path),
        "odt" => odt::parse_odt(path),
        "epub" => epub::parse_epub(path),
        "pdf" => pdf::parse_pdf(path),
        "txt" | "md" | "rs" | "js" | "ts" | "json" | "xml" | "html" | "css" | "py" | "java"
        | "cpp" | "c" | "h" | "go" | "rb" | "php" | "swift" | "kt" | "dart" | "yaml" | "yml"
        | "toml" | "ini" | "conf" | "env" | "dockerfile" | "sh" | "bat" | "ps1" | "sql" | "r"
        | "log" | "svg" | "ics" | "vcf" | "cmake" | "gradle" | "properties" | "proto" | "less"
        | "scss" | "svelte" | "vue" => text::parse_text(path),
        _ => Err(FlashError::UnsupportedFormat(extension)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_file_txt() {
        // This will be implemented with test fixtures
        let path = PathBuf::from("tests/fixtures/sample.txt");
        // Test implementation here
    }
}
