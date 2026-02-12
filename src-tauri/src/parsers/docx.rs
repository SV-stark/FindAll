use crate::error::{FlashError, Result};
use crate::parsers::ParsedDocument;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use zip::ZipArchive;

/// Parse DOCX file using buffered I/O and streaming XML parsing
/// Optimized for memory efficiency and proper error handling
pub fn parse_docx(path: &Path) -> Result<ParsedDocument> {
    // Open file with buffered reader instead of memory mapping
    // This provides better error handling and doesn't require unsafe
    let file = File::open(path).map_err(|e| {
        FlashError::Parse(format!("Failed to open file {}: {}", path.display(), e))
    })?;

    let reader = BufReader::new(file);
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| FlashError::Parse(format!("Failed to read ZIP archive: {}", e)))?;

    // Extract document.xml with size limit to prevent OOM
    const MAX_XML_SIZE: usize = 100 * 1024 * 1024; // 100MB limit
    let mut xml_content = String::new();
    {
        let mut doc_xml = archive
            .by_name("word/document.xml")
            .map_err(|e| FlashError::Parse(format!("Failed to find document.xml: {}", e)))?;

        // Check size before reading
        if doc_xml.size() > MAX_XML_SIZE as u64 {
            return Err(FlashError::Parse(format!(
                "document.xml too large: {} bytes (max: {})",
                doc_xml.size(),
                MAX_XML_SIZE
            )));
        }

        doc_xml
            .read_to_string(&mut xml_content)
            .map_err(|e| FlashError::Io(e))?;
    }

    // Stream parse XML without loading into DOM
    let mut reader = Reader::from_str(&xml_content);
    reader.trim_text(true);

    let mut buf = Vec::with_capacity(1024); // Pre-allocate buffer
    let mut text = String::with_capacity(xml_content.len() / 2); // Estimate capacity
    let mut in_text_element = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
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
                return Err(FlashError::Parse(format!(
                    "XML parsing error in {}: {}",
                    path.display(),
                    e
                )));
            }
            _ => {}
        }
        buf.clear();
    }

    // Try to extract title from core.xml (optional)
    let title = extract_title(&mut archive).ok();

    // Explicitly drop archive to release file handle
    drop(archive);

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: text.trim().to_string(),
        title,
    })
}

/// Extract document title from core.xml metadata
fn extract_title<R: std::io::Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<String> {
    let mut core_xml = archive.by_name("docProps/core.xml").map_err(|e| {
        FlashError::Parse(format!("Failed to find core.xml metadata: {}", e))
    })?;

    let mut xml_content = String::new();
    core_xml
        .read_to_string(&mut xml_content)
        .map_err(|e| FlashError::Io(e))?;

    // Limit search to prevent excessive memory usage
    const MAX_CORE_XML_SIZE: usize = 10 * 1024 * 1024; // 10MB
    if xml_content.len() > MAX_CORE_XML_SIZE {
        return Err(FlashError::Parse(
            "core.xml metadata too large".to_string(),
        ));
    }

    let mut reader = Reader::from_str(&xml_content);
    let mut buf = Vec::with_capacity(512);
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
                        let title = txt.to_string();
                        if !title.trim().is_empty() {
                            return Ok(title);
                        }
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

    Err(FlashError::Parse("Title not found in metadata".to_string()))
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
