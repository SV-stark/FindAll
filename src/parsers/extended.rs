use crate::error::{FlashError, Result};
use crate::parsers::memory_map;
use crate::parsers::ParsedDocument;
use phf::phf_set;
use std::io::Read;
use std::path::Path;

static TEXT_EXTENSIONS: phf::Set<&'static str> = phf_set![
    "txt", "md", "json", "xml", "html", "htm", "js", "ts", "rs", "py", "java", "c", "cpp", "h",
    "hpp", "cs", "go", "rb", "php", "sql", "yaml", "yml", "toml", "ini", "cfg", "conf",
];

pub fn parse_rtf(path: &Path) -> Result<ParsedDocument> {
    // Litchi integration is currently disabled due to crate API incompatibility
    /*
    match litchi::extract(path) {
        Ok(text) => return Ok(ParsedDocument {
            path: path.to_string_lossy().to_string(),
            content: text,
            title: None,
        }),
        Err(_) => {}
    }
    */

    let metadata = memory_map::get_file_size(path)?;
    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: format!("RTF Document (Litchi disabled): {} bytes.", metadata),
        title: None,
    })
}

pub fn parse_eml(path: &Path) -> Result<ParsedDocument> {
    let content = memory_map::read_file_as_string(path)?;

    let mut title = String::new();
    let mut text = String::new();
    let mut in_body = false;

    for line in content.lines() {
        let line_lower = line.to_lowercase();

        if line_lower.starts_with("subject:") {
            title = line[8..].trim().to_string();
        } else if line_lower.starts_with("from:")
            || line_lower.starts_with("to:")
            || line_lower.starts_with("date:")
        {
            text.push_str(line);
            text.push(' ');
        } else if line_lower.is_empty() {
            in_body = true;
        } else if in_body {
            text.push_str(line);
            text.push(' ');
        }
    }

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: text.trim().to_string(),
        title: if title.is_empty() { None } else { Some(title) },
    })
}

pub fn parse_msg(path: &Path) -> Result<ParsedDocument> {
    let content = memory_map::read_file(path)?;

    let mut text = String::new();
    let mut in_string = false;
    let mut current = String::new();

    for byte in content {
        if byte.is_ascii_graphic() || byte == b' ' || byte == b'\n' {
            current.push(byte as char);
            in_string = true;
        } else if in_string && current.len() > 3 {
            text.push_str(&current);
            text.push(' ');
            current.clear();
            in_string = false;
        } else {
            current.clear();
            in_string = false;
        }
    }

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: text
            .split_whitespace()
            .take(5000)
            .collect::<Vec<_>>()
            .join(" "),
        title: None,
    })
}

pub fn parse_chm(path: &Path) -> Result<ParsedDocument> {
    let content = memory_map::read_file(path)?;

    let mut text = String::new();
    let mut current = Vec::new();

    for byte in content {
        if byte.is_ascii_graphic() || byte == b' ' {
            current.push(byte);
        } else if current.len() > 4 {
            if let Ok(s) = String::from_utf8(current.clone()) {
                if s.chars().all(|c| c.is_alphanumeric() || c.is_whitespace()) {
                    text.push_str(&s);
                    text.push(' ');
                }
            }
            current.clear();
        } else {
            current.clear();
        }
    }

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: text
            .split_whitespace()
            .take(5000)
            .collect::<Vec<_>>()
            .join(" "),
        title: None,
    })
}

pub fn parse_azw(path: &Path) -> Result<ParsedDocument> {
    let content = memory_map::read_file(path)?;

    let mut text = String::new();
    let mut current = Vec::new();

    if content.len() > 68 {
        if &content[0..4] == b"TPZ" || &content[60..68] == b"BOOKMOBI" {
            // Kindle format detected
        }
    }

    for byte in content {
        if byte.is_ascii_graphic() || byte == b' ' || byte == b'\n' {
            current.push(byte);
        } else if current.len() > 5 {
            if let Ok(s) = String::from_utf8(current.clone()) {
                if s.len() > 5
                    && s.chars()
                        .all(|c| c.is_alphanumeric() || c.is_whitespace() || c == '.' || c == ',')
                {
                    text.push_str(&s);
                    text.push(' ');
                }
            }
            current.clear();
        } else {
            current.clear();
        }
    }

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: text
            .split_whitespace()
            .take(10000)
            .collect::<Vec<_>>()
            .join(" "),
        title: None,
    })
}

pub fn parse_zip_content(path: &Path) -> Result<ParsedDocument> {
    use zip::ZipArchive;

    // Use File::open instead of reading entire file into memory (P5)
    let file = std::fs::File::open(path)
        .map_err(|e: std::io::Error| FlashError::parse("open_zip", e.to_string()))?;
    let mut archive = ZipArchive::new(std::io::BufReader::new(file))
        .map_err(|e: zip::result::ZipError| FlashError::parse("open_archive", e.to_string()))?;

    let mut all_text = String::new();
    let max_size = 10 * 1024 * 1024; // 10MB limit

    for i in 0..archive.len() {
        if let Ok(file) = archive.by_index(i) {
            if !file.is_dir() {
                let name = file.name().to_lowercase();

                if let Some(ext) = name.rsplit('.').next() {
                    if TEXT_EXTENSIONS.contains(ext) {
                        // Limit read size per file and total
                        let mut content = String::new();
                        // Put a cap on individual file read to avoid safe-bomb/DoS
                        let mut take = file.take(1 * 1024 * 1024); // 1MB per file
                        if take.read_to_string(&mut content).is_ok() {
                            all_text.push_str(&content);
                            all_text.push_str("\n\n");
                        }
                    }
                }
            }
        }
        if all_text.len() >= max_size {
            break; 
        }
    }

    if all_text.is_empty() {
        // Just return metadata if no text extracted? Or error?
        // Original returned error, but maybe return empty doc is better?
        // Let's stick to original behavior or just return empty content.
        // Returning error might be filtered out as failure.
        // Let's return empty content if valid zip but no text.
        // But if it was empty because no text files found, that's fine.
        // The original error "unsupported_format" is a bit misleading if it IS a zip.
        // Let's return valid doc with empty content.
    }

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: all_text,
        title: None,
    })
}

pub fn parse_7z_content(path: &Path) -> Result<ParsedDocument> {
    let metadata = memory_map::get_file_size(path)?;

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: format!("7z archive: {} bytes", metadata),
        title: None,
    })
}

pub fn parse_rar_content(path: &Path) -> Result<ParsedDocument> {
    let metadata = memory_map::get_file_size(path)?;

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: format!("RAR archive: {} bytes", metadata),
        title: None,
    })
}

pub fn parse_legacy_office(path: &Path) -> Result<ParsedDocument> {
    // Litchi integration is currently disabled due to crate API incompatibility
    /*
    match litchi::extract(path) {
         Ok(text) => return Ok(ParsedDocument {
            path: path.to_string_lossy().to_string(),
            content: text,
            title: None,
        }),
        Err(_) => {}
    }
    */

    let metadata = memory_map::get_file_size(path)?;
    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: format!("Legacy Office Document (Litchi disabled): {} bytes.", metadata),
        title: None,
    })
}
