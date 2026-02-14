use crate::error::{FlashError, Result};
use std::ffi::OsStr;
use std::path::Path;

pub mod docx;
pub mod epub;
pub mod excel;
pub mod extended;
pub mod odt;
pub mod pdf;
pub mod pptx;
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
    if extension_matches(extension, "docx") || extension_matches(extension, "doc") {
        return docx::parse_docx(path);
    }

    // Check PowerPoint formats
    if extension_matches(extension, "pptx") || extension_matches(extension, "ppt") {
        return pptx::parse_pptx(path);
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
    
    // Check Excel formats
    if extension_matches(extension, "xlsx")
        || extension_matches(extension, "xls")
        || extension_matches(extension, "xlsb") {
        return excel::parse_excel(path);
    }

    // Check RTF format
    if extension_matches(extension, "rtf") {
        return extended::parse_rtf(path);
    }

    // Check email formats
    if extension_matches(extension, "eml") {
        return extended::parse_eml(path);
    }
    if extension_matches(extension, "msg") {
        return extended::parse_msg(path);
    }

    // Check CHM format
    if extension_matches(extension, "chm") {
        return extended::parse_chm(path);
    }

    // Check Kindle/AZW formats
    if extension_matches(extension, "azw") 
        || extension_matches(extension, "azw3")
        || extension_matches(extension, "mobi") {
        return extended::parse_azw(path);
    }

    // Check archive formats
    if extension_matches(extension, "zip") {
        return extended::parse_zip_content(path);
    }
    if extension_matches(extension, "7z") {
        return extended::parse_7z_content(path);
    }
    if extension_matches(extension, "rar") {
        return extended::parse_rar_content(path);
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
        // More code files
        b"cs",
        b"jsx",
        b"tsx",
        b"vue",
        b"jsx",
        b"sx",
        b"asm",
        b"s",
        b"m",
        b"pl",
        b"lua",
        b"ex",
        b"exs",
        b"erl",
        b"clj",
        b"fs",
        b"fsx",
        b"vb",
        b"pas",
        b"d",
        b"zig",
        b"nim",
        b"hlsl",
        b"glsl",
        b"cmake",
        b"makefile",
        // Data formats
        b"csv",
        b"tsv",
        b"dat",
        b"msgpack",
        b"cbor",
        b"toml",
        // Documents
        b"tex",
        b"latex",
        b"rst",
        b"adoc",
        b"asciidoc",
        // Config
        b"gitignore",
        b"gitattributes",
        b"editorconfig",
        b"prettierrc",
        b"eslintrc",
        b"babelrc",
        b"webpack",
        b"nginx",
        b"apache",
        b"htaccess",
        // Shell/other
        b"fish",
        b"zsh",
        b"csh",
        b"awk",
        b"sed",
        b"vim",
        b"vimrc",
        b"gitconfig",
        b"env",
        b"properties",
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
