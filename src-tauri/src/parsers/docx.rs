use crate::error::{FlashError, Result};
use crate::parsers::ParsedDocument;
use memmap2::Mmap;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

/// Parse DOCX file using memory mapping and streaming XML parsing
/// This is a zero-copy approach that minimizes memory usage
pub fn parse_docx(path: &Path) -> Result<ParsedDocument> {
    // Memory map the file for zero-copy access
    let file = File::open(path).map_err(|e| FlashError::Io(e))?;

    let mmap = unsafe {
        Mmap::map(&file)
            .map_err(|e| FlashError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?
    };

    // Create cursor from memory mapped file
    let cursor = std::io::Cursor::new(&mmap[..]);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| FlashError::Parse(format!("Failed to read ZIP: {}", e)))?;

    let mut xml_content = String::new();
    {
        // Extract document.xml from the DOCX (which is a ZIP file)
        let mut doc_xml = archive
            .by_name("word/document.xml")
            .map_err(|e| FlashError::Parse(format!("Failed to find document.xml: {}", e)))?;

        doc_xml
            .read_to_string(&mut xml_content)
            .map_err(|e| FlashError::Io(e))?;
    }

    // Stream parse XML without loading into DOM
    let mut reader = Reader::from_str(&xml_content);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut text = String::new();
    let mut in_text_element = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                // Check if this is a text element
                if e.name().as_ref() == b"w:t" {
                    in_text_element = true;
                }
            }
            Ok(Event::Text(e)) => {
                if in_text_element {
                    if let Ok(txt) = e.unescape() {
                        text.push_str(&txt);
                        text.push(' ');
                    }
                }
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == b"w:t" {
                    in_text_element = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(FlashError::Parse(format!("XML parsing error: {}", e)));
            }
            _ => {}
        }
        buf.clear();
    }

    // Try to extract title from core.xml
    let title = extract_title(&mut archive).ok();

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: text.trim().to_string(),
        title,
    })
}

/// Extract document title from core.xml metadata
fn extract_title<R: std::io::Read + std::io::Seek>(archive: &mut ZipArchive<R>) -> Result<String> {
    let mut core_xml = archive
        .by_name("docProps/core.xml")
        .map_err(|e| FlashError::Parse(format!("Failed to find core.xml: {}", e)))?;

    let mut xml_content = String::new();
    core_xml
        .read_to_string(&mut xml_content)
        .map_err(|e| FlashError::Io(e))?;

    let mut reader = Reader::from_str(&xml_content);
    let mut buf = Vec::new();
    let mut in_title = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if e.name().as_ref() == b"dc:title" {
                    in_title = true;
                }
            }
            Ok(Event::Text(e)) => {
                if in_title {
                    if let Ok(txt) = e.unescape() {
                        return Ok(txt.to_string());
                    }
                }
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == b"dc:title" {
                    in_title = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(FlashError::Parse(format!("XML parsing error: {}", e)));
            }
            _ => {}
        }
        buf.clear();
    }

    Err(FlashError::Parse("Title not found".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docx_parsing_placeholder() {
        // Placeholder for actual test
        // Requires test fixture: tests/fixtures/sample.docx
    }
}
