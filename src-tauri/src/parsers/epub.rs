use crate::error::{FlashError, Result};
use crate::parsers::ParsedDocument;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use zip::ZipArchive;

/// Maximum size for individual HTML files within EPUB (prevent OOM)
const MAX_HTML_SIZE: usize = 50 * 1024 * 1024; // 50MB
/// Maximum total extracted text size (prevent unbounded growth)
const MAX_TOTAL_TEXT_SIZE: usize = 200 * 1024 * 1024; // 200MB

/// Parse EPUB file by extracting text from all HTML/XHTML components within the ZIP
pub fn parse_epub(path: &Path) -> Result<ParsedDocument> {
    let file = File::open(path).map_err(|e| {
        FlashError::Parse(format!("Failed to open EPUB file {}: {}", path.display(), e))
    })?;

    let reader = BufReader::new(file);
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| FlashError::Parse(format!("Failed to read EPUB archive: {}", e)))?;

    let mut combined_text = String::with_capacity(1024 * 1024); // Start with 1MB capacity
    let mut total_extracted_size: usize = 0;

    // Collect all filenames first to avoid borrowing issues while iterating
    let file_names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();

    for name in file_names {
        // Only process HTML/XHTML files
        if name.ends_with(".xhtml") || name.ends_with(".html") {
            match archive.by_name(&name) {
                Ok(mut inner_file) => {
                    // Check file size before reading
                    if inner_file.size() > MAX_HTML_SIZE as u64 {
                        eprintln!(
                            "Warning: Skipping large HTML file {} in EPUB ({} bytes)",
                            name,
                            inner_file.size()
                        );
                        continue;
                    }

                    // Pre-allocate string with known capacity
                    let mut content = String::with_capacity(inner_file.size() as usize);
                    
                    match inner_file.read_to_string(&mut content) {
                        Ok(_) => {
                            // Check total size limit
                            if total_extracted_size + content.len() > MAX_TOTAL_TEXT_SIZE {
                                eprintln!(
                                    "Warning: EPUB {} has exceeded maximum text size limit",
                                    path.display()
                                );
                                break;
                            }

                            // Extract text content
                            extract_text_from_html(&content, &mut combined_text);
                            total_extracted_size += content.len();
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to read {} from EPUB: {}", name, e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to access {} in EPUB: {}", name, e);
                }
            }
        }
    }

    // Drop archive explicitly to release resources
    drop(archive);

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: combined_text.trim().to_string(),
        title: extract_epub_title(path).ok(),
    })
}

/// Extract text content from HTML string using streaming XML parser
fn extract_text_from_html(html: &str, output: &mut String) {
    let mut reader = Reader::from_str(html);
    reader.trim_text(true);
    
    let mut buf = Vec::with_capacity(1024);

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(e)) => {
                if let Ok(txt) = e.unescape() {
                    output.push_str(&txt);
                    output.push(' ');
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                eprintln!("Warning: HTML parsing error: {}", e);
                break;
            }
            _ => {}
        }
        buf.clear();
    }
}

/// Try to extract title from EPUB metadata
fn extract_epub_title(path: &Path) -> Result<String> {
    let file = File::open(path).map_err(|e| FlashError::Io(e))?;
    let reader = BufReader::new(file);
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| FlashError::Parse(format!("Failed to read EPUB: {}", e)))?;

    // Try to read OPF file for metadata
    if let Ok(mut container_xml) = archive.by_name("META-INF/container.xml") {
        let mut content = String::new();
        if container_xml.read_to_string(&mut content).is_ok() {
            // Parse container.xml to find OPF path
            if let Some(opf_path) = extract_opf_path(&content) {
                drop(container_xml);
                
                if let Ok(mut opf_file) = archive.by_name(&opf_path) {
                    let mut opf_content = String::new();
                    if opf_file.read_to_string(&mut opf_content).is_ok() {
                        if let Some(title) = extract_title_from_opf(&opf_content) {
                            return Ok(title);
                        }
                    }
                }
            }
        }
    }

    // Fallback to filename
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .ok_or_else(|| FlashError::Parse("Could not extract title".to_string()))
}

/// Extract OPF file path from container.xml
fn extract_opf_path(container_xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(container_xml);
    let mut buf = Vec::with_capacity(512);
    let mut in_rootfile = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) => {
                if e.name().as_ref() == b"rootfile" {
                    // Extract full-path attribute
                    for attr in e.attributes() {
                        if let Ok(attr) = attr {
                            if attr.key.as_ref() == b"full-path" {
                                if let Ok(path) = std::str::from_utf8(&attr.value) {
                                    return Some(path.to_string());
                                }
                            }
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    // Default fallback
    Some("OEBPS/content.opf".to_string())
}

/// Extract title from OPF content
fn extract_title_from_opf(opf_content: &str) -> Option<String> {
    let mut reader = Reader::from_str(opf_content);
    let mut buf = Vec::with_capacity(1024);
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
                            return Some(title);
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
            _ => {}
        }
        buf.clear();
    }

    None
}
