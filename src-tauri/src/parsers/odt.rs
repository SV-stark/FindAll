use crate::error::{FlashError, Result};
use crate::parsers::ParsedDocument;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use zip::ZipArchive;

/// Maximum XML content size to prevent OOM attacks
const MAX_XML_SIZE: usize = 100 * 1024 * 1024; // 100MB

/// Parse ODT file using buffered I/O and streaming XML parsing
/// OpenDocument files are ZIP archives with content in content.xml
pub fn parse_odt(path: &Path) -> Result<ParsedDocument> {
    let file = File::open(path).map_err(|e| {
        FlashError::Parse(format!("Failed to open file {}: {}", path.display(), e))
    })?;

    let reader = BufReader::new(file);
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| FlashError::Parse(format!("Failed to read ZIP archive: {}", e)))?;

    // ODT content is stored in content.xml
    let mut content_xml = archive
        .by_name("content.xml")
        .map_err(|e| FlashError::Parse(format!("Failed to find content.xml: {}", e)))?;

    // Check size before reading to prevent OOM
    if content_xml.size() > MAX_XML_SIZE as u64 {
        return Err(FlashError::Parse(format!(
            "content.xml too large: {} bytes (max: {})",
            content_xml.size(),
            MAX_XML_SIZE
        )));
    }

    let mut xml_content = String::with_capacity(content_xml.size() as usize);
    content_xml
        .read_to_string(&mut xml_content)
        .map_err(|e| FlashError::Io(e))?;

    // Explicitly drop the zip entry to release resources
    drop(content_xml);
    drop(archive);

    let mut reader = Reader::from_str(&xml_content);
    reader.trim_text(true);

    let mut buf = Vec::with_capacity(1024);
    let mut text = String::with_capacity(xml_content.len() / 2);

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(e)) => {
                if let Ok(txt) = e.unescape() {
                    text.push_str(&txt);
                    text.push(' ');
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

    // Try to extract title from meta.xml
    let title = extract_odt_title(path).ok();

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: text.trim().to_string(),
        title: title.or_else(|| path.file_stem().map(|s| s.to_string_lossy().to_string())),
    })
}

/// Extract title from ODT meta.xml if available
fn extract_odt_title(path: &Path) -> Result<String> {
    let file = File::open(path).map_err(|e| FlashError::Io(e))?;
    let reader = BufReader::new(file);
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| FlashError::Parse(format!("Failed to read ZIP: {}", e)))?;

    let mut meta_xml = archive
        .by_name("meta.xml")
        .map_err(|e| FlashError::Parse(format!("Failed to find meta.xml: {}", e)))?;

    if meta_xml.size() > 10 * 1024 * 1024 {
        // 10MB limit for meta.xml
        return Err(FlashError::Parse("meta.xml too large".to_string()));
    }

    let mut xml_content = String::new();
    meta_xml
        .read_to_string(&mut xml_content)
        .map_err(|e| FlashError::Io(e))?;

    drop(meta_xml);
    drop(archive);

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
            Err(_) => break, // Non-fatal: title extraction is optional
            _ => {}
        }
        buf.clear();
    }

    Err(FlashError::Parse("Title not found".to_string()))
}
