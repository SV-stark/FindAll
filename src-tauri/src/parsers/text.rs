use std::fs::File;
use std::io::Read;
use std::path::Path;
use crate::error::{FlashError, Result};
use crate::parsers::ParsedDocument;

/// Parse plain text files (TXT, MD, code files)
pub fn parse_text(path: &Path) -> Result<ParsedDocument> {
    let mut file = File::open(path)
        .map_err(|e| FlashError::Io(e))?;
    
    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(|e| FlashError::Io(e))?;
    
    // Extract title from first line (for markdown/text files)
    let title = content
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| {
            // Remove markdown heading markers if present
            line.trim()
                .trim_start_matches("# ")
                .trim_start_matches("## ")
                .trim_start_matches("### ")
                .to_string()
        })
        .filter(|line| line.len() < 200);
    
    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content,
        title,
    })
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
}
