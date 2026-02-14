use crate::error::{FlashError, Result};
use crate::parsers::ParsedDocument;
use memmap2::Mmap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

/// Threshold for using memory mapping (files larger than this will be memory-mapped)
const MMAP_THRESHOLD: u64 = 1024 * 1024; // 1MB
/// Maximum file size to parse (prevent DOS)
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024; // 100MB

/// Parse plain text files (TXT, MD, code files)
/// Uses memory mapping for large files to reduce memory usage
pub fn parse_text(path: &Path) -> Result<ParsedDocument> {
    let metadata = std::fs::metadata(path).map_err(|e| FlashError::Io(e))?;

    let file_size = metadata.len();

    // Security: skip extremely large files
    if file_size > MAX_FILE_SIZE {
        return Err(FlashError::Parse(format!(
            "File too large: {} bytes (max: {})",
            file_size, MAX_FILE_SIZE
        )));
    }

    // Choose parsing strategy based on file size
    let content = if file_size > MMAP_THRESHOLD {
        parse_with_mmap(path)?
    } else {
        parse_with_buffer(path)?
    };

    // Extract title from first non-empty line
    let title = extract_title(&content);

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content,
        title,
    })
}

/// Parse small files using buffered reader (faster for small files)
fn parse_with_buffer(path: &Path) -> Result<String> {
    let file = File::open(path).map_err(|e| FlashError::Io(e))?;

    let mut content = String::new();
    BufReader::new(file)
        .read_to_string(&mut content)
        .map_err(|e| FlashError::Io(e))?;

    Ok(content)
}

/// Parse large files using memory mapping (zero-copy, better for large files)
fn parse_with_mmap(path: &Path) -> Result<String> {
    let file = File::open(path)
        .map_err(|e| FlashError::Parse(format!("Failed to open file {}: {}", path.display(), e)))?;

    // Memory map the file
    let mmap = unsafe {
        Mmap::map(&file)
            .map_err(|e| FlashError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?
    };

    // Convert to string (this will allocate, but only once)
    // For text files, we assume valid UTF-8
    String::from_utf8(mmap.to_vec())
        .map_err(|e| FlashError::Parse(format!("Invalid UTF-8 in file {}: {}", path.display(), e)))
}

/// Extract title from first non-empty line
fn extract_title(content: &str) -> Option<String> {
    content
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| {
            // Remove markdown heading markers if present
            let trimmed = line.trim();
            if let Some(stripped) = trimmed.strip_prefix("# ") {
                stripped.to_string()
            } else if let Some(stripped) = trimmed.strip_prefix("## ") {
                stripped.to_string()
            } else if let Some(stripped) = trimmed.strip_prefix("### ") {
                stripped.to_string()
            } else {
                trimmed.to_string()
            }
        })
        .filter(|line| line.len() < 200) // Sanity check: don't use entire paragraphs
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_text_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "# My Title").unwrap();
        writeln!(temp_file, "Some content here").unwrap();

        let result = parse_text(temp_file.path()).unwrap();

        assert!(result.content.contains("My Title"));
        assert!(result.content.contains("Some content"));
        assert_eq!(result.title, Some("My Title".to_string()));
    }

    #[test]
    fn test_parse_text_no_heading() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Just a title").unwrap();
        writeln!(temp_file, "Content here").unwrap();

        let result = parse_text(temp_file.path()).unwrap();

        assert_eq!(result.title, Some("Just a title".to_string()));
    }
}
