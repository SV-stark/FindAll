use crate::error::{FlashError, Result};
use crate::parsers::memory_map;
use crate::parsers::ParsedDocument;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;
use tracing::warn;

const MAX_HTML_SIZE: usize = 50 * 1024 * 1024;
const MAX_TOTAL_TEXT_SIZE: usize = 200 * 1024 * 1024;

pub fn parse_epub(path: &Path) -> Result<ParsedDocument> {
    let bytes = memory_map::read_file(path)?;

    let cursor = std::io::Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| FlashError::parse(path, format!("Failed to read EPUB archive: {}", e)))?;

    let mut combined_text = String::with_capacity(1024 * 1024);
    let mut total_extracted_size: usize = 0;

    let file_names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();

    for name in file_names {
        if name.ends_with(".xhtml") || name.ends_with(".html") {
            match archive.by_name(&name) {
                Ok(mut inner_file) => {
                    if inner_file.size() > MAX_HTML_SIZE as u64 {
                        warn!(
                            "Skipping large HTML file {} in EPUB ({} bytes)",
                            name,
                            inner_file.size()
                        );
                        continue;
                    }

                    let mut content = String::with_capacity(inner_file.size() as usize);

                    match inner_file.read_to_string(&mut content) {
                        Ok(_) => {
                            if total_extracted_size + content.len() > MAX_TOTAL_TEXT_SIZE {
                                warn!(
                                    "EPUB {} has exceeded maximum text size limit",
                                    path.display()
                                );
                                break;
                            }

                            extract_text_from_html(&content, &mut combined_text);
                            total_extracted_size += content.len();
                        }
                        Err(e) => {
                            warn!("Failed to read {} from EPUB: {}", name, e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to access {} in EPUB: {}", name, e);
                }
            }
        }
    }

    drop(archive);

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: combined_text.trim().to_string(),
        title: extract_epub_title(path).ok(),
    })
}

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
                warn!("HTML parsing error: {}", e);
                break;
            }
            _ => {}
        }
        buf.clear();
    }
}

fn extract_epub_title(path: &Path) -> Result<String> {
    let bytes = memory_map::read_file(path)?;
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| FlashError::parse(path, format!("Failed to read EPUB: {}", e)))?;

    let op_path = if let Ok(mut container_xml) = archive.by_name("META-INF/container.xml") {
        let mut content = String::new();
        if container_xml.read_to_string(&mut content).is_ok() {
            extract_opf_path(&content)
        } else {
            None
        }
    } else {
        None
    };

    if let Some(opf_path) = op_path {
        if let Ok(mut opf_file) = archive.by_name(&opf_path) {
            let mut opf_content = String::new();
            if opf_file.read_to_string(&mut opf_content).is_ok() {
                if let Some(title) = extract_title_from_opf(&opf_content) {
                    return Ok(title);
                }
            }
        }
    }

    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .ok_or_else(|| FlashError::parse(path, "Could not extract title"))
}

fn extract_opf_path(container_xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(container_xml);
    let mut buf = Vec::with_capacity(512);

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) => {
                if e.name().as_ref() == b"rootfile" {
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

    Some("OEBPS/content.opf".to_string())
}

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
