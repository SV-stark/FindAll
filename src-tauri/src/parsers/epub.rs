use crate::error::{FlashError, Result};
use crate::parsers::ParsedDocument;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

/// Parse EPUB file by extracting text from all HTML/XHTML components within the ZIP
pub fn parse_epub(path: &Path) -> Result<ParsedDocument> {
    let file = File::open(path).map_err(|e| FlashError::Io(e))?;

    let mut archive = ZipArchive::new(file)
        .map_err(|e| FlashError::Parse(format!("Failed to read ZIP: {}", e)))?;

    let mut combined_text = String::new();

    // Collect all filenames first to avoid borrowing issues while iterating
    let file_names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();

    for name in file_names {
        if name.ends_with(".xhtml") || name.ends_with(".html") {
            if let Ok(mut inner_file) = archive.by_name(&name) {
                let mut content = String::new();
                if inner_file.read_to_string(&mut content).is_ok() {
                    let mut reader = Reader::from_str(&content);
                    reader.trim_text(true);
                    let mut buf = Vec::new();

                    loop {
                        match reader.read_event_into(&mut buf) {
                            Ok(Event::Text(e)) => {
                                if let Ok(txt) = e.unescape() {
                                    combined_text.push_str(&txt);
                                    combined_text.push(' ');
                                }
                            }
                            Ok(Event::Eof) => break,
                            _ => {}
                        }
                        buf.clear();
                    }
                }
            }
        }
    }

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: combined_text.trim().to_string(),
        title: path.file_stem().map(|s| s.to_string_lossy().to_string()),
    })
}
