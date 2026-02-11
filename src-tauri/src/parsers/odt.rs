use crate::error::{FlashError, Result};
use crate::parsers::ParsedDocument;
use memmap2::Mmap;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

/// Parse ODT file using memory mapping and streaming XML parsing
/// OpenDocument files are ZIP archives with content in content.xml
pub fn parse_odt(path: &Path) -> Result<ParsedDocument> {
    let file = File::open(path).map_err(|e| FlashError::Io(e))?;

    let mmap = unsafe {
        Mmap::map(&file)
            .map_err(|e| FlashError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?
    };

    let cursor = std::io::Cursor::new(&mmap[..]);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| FlashError::Parse(format!("Failed to read ZIP: {}", e)))?;

    // ODT content is stored in content.xml
    let mut content_xml = archive
        .by_name("content.xml")
        .map_err(|e| FlashError::Parse(format!("Failed to find content.xml: {}", e)))?;

    let mut xml_content = String::new();
    content_xml
        .read_to_string(&mut xml_content)
        .map_err(|e| FlashError::Io(e))?;

    let mut reader = Reader::from_str(&xml_content);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut text = String::new();

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
                return Err(FlashError::Parse(format!("XML parsing error: {}", e)));
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: text.trim().to_string(),
        title: path.file_stem().map(|s| s.to_string_lossy().to_string()),
    })
}
