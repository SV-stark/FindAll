use crate::error::{FlashError, Result};
use crate::parsers::ParsedDocument;
use std::io::Read;
use std::path::Path;

pub fn parse_rtf(path: &Path) -> Result<ParsedDocument> {
    let content = std::fs::read_to_string(path)?;

    let mut text = String::new();
    let _in_control = false;
    let mut brace_depth = 0;
    let mut skip_until_brace = false;

    let bytes = content.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];

        if skip_until_brace {
            if b == b'{' {
                brace_depth += 1;
            } else if b == b'}' {
                brace_depth -= 1;
                if brace_depth == 0 {
                    skip_until_brace = false;
                }
            }
            i += 1;
            continue;
        }

        match b {
            b'\\' => {
                if i + 1 < bytes.len() {
                    let next = bytes[i + 1];
                    match next {
                        b'\'' => {
                            // RTF hex escape
                            if i + 3 < bytes.len() {
                                if let Ok(hex_str) = std::str::from_utf8(&bytes[i + 2..i + 4]) {
                                    if let Ok(byte) = u8::from_str_radix(hex_str, 16) {
                                        text.push(byte as char);
                                    }
                                }
                                i += 4;
                                continue;
                            }
                        }
                        b'{' | b'}' | b'\\' => {
                            text.push(next as char);
                            i += 2;
                            continue;
                        }
                        b'\n' | b'\r' => {
                            text.push(' ');
                            i += 2;
                            continue;
                        }
                        _ => {
                            // Control word - skip it
                            let mut j = i + 1;
                            while j < bytes.len() && bytes[j].is_ascii_alphabetic() {
                                j += 1;
                            }
                            if j > i + 1 {
                                let control = std::str::from_utf8(&bytes[i + 1..j]).unwrap_or("");

                                // Handle special control words
                                match control {
                                    "par" | "line" => text.push(' '),
                                    "tab" => text.push('\t'),
                                    "emph" | "b" | "i" | "u" | "strike" | "fs" => {
                                        // Skip content until next control word or brace
                                        skip_until_brace = true;
                                        brace_depth = 0;
                                    }
                                    _ => {}
                                }
                                i = j;
                                if i < bytes.len() && bytes[i] == b' ' {
                                    i += 1;
                                }
                                continue;
                            }
                        }
                    }
                }
                i += 1;
            }
            b'{' => {
                brace_depth += 1;
                i += 1;
            }
            b'}' => {
                brace_depth -= 1;
                i += 1;
            }
            _ => {
                if b.is_ascii() && b != b'\r' && b != b'\n' {
                    text.push(b as char);
                }
                i += 1;
            }
        }
    }

    // Clean up whitespace
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: text,
        title: None,
    })
}

pub fn parse_eml(path: &Path) -> Result<ParsedDocument> {
    let content = std::fs::read_to_string(path)?;

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
    // MSG files are compound files - try to extract text
    // For now, fall back to basic text extraction
    // Full MSG parsing would require the msg crate
    let content = std::fs::read(path)?;

    // Try to extract printable ASCII strings
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
    // CHM files are MS Compiled HTML Help
    // For now, return a placeholder - full CHM parsing requires the chm crate
    let content = std::fs::read(path)?;

    // Extract strings from the binary
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
    // AZW is Amazon's Kindle format - similar to MOBI
    // Try to extract text from the raw data
    let content = std::fs::read(path)?;

    let mut text = String::new();
    let mut current = Vec::new();
    let mut _in_palmdoc = false;

    // Check for PALM DOC signature
    if content.len() > 68 {
        if &content[0..4] == b"TPZ" || &content[60..68] == b"BOOKMOBI" {
            _in_palmdoc = true;
        }
    }

    // Extract strings
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
    use std::io::BufReader;
    use zip::ZipArchive;

    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| FlashError::archive("ZIP", "open_archive", e.to_string()))?;

    let mut all_text = String::new();

    for i in 0..archive.len() {
        if let Ok(mut file) = archive.by_index(i) {
            if !file.is_dir() {
                let name = file.name().to_lowercase();

                // Only extract text-like files
                if name.ends_with(".txt")
                    || name.ends_with(".md")
                    || name.ends_with(".json")
                    || name.ends_with(".xml")
                    || name.ends_with(".html")
                    || name.ends_with(".htm")
                    || name.ends_with(".js")
                    || name.ends_with(".ts")
                    || name.ends_with(".rs")
                    || name.ends_with(".py")
                    || name.ends_with(".java")
                    || name.ends_with(".c")
                    || name.ends_with(".cpp")
                    || name.ends_with(".h")
                    || name.ends_with(".hpp")
                    || name.ends_with(".cs")
                    || name.ends_with(".go")
                    || name.ends_with(".rb")
                    || name.ends_with(".php")
                    || name.ends_with(".sql")
                    || name.ends_with(".yaml")
                    || name.ends_with(".yml")
                    || name.ends_with(".toml")
                    || name.ends_with(".ini")
                    || name.ends_with(".cfg")
                    || name.ends_with(".conf")
                {
                    let mut content = String::new();
                    if file.read_to_string(&mut content).is_ok() {
                        all_text.push_str(&content);
                        all_text.push_str("\n\n");
                    }
                }
            }
        }
    }

    if all_text.is_empty() {
        return Err(FlashError::unsupported_format(
            "Archive",
            path.extension().and_then(|e| e.to_str()).unwrap_or("zip"),
        ));
    }

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: all_text,
        title: None,
    })
}

pub fn parse_7z_content(path: &Path) -> Result<ParsedDocument> {
    // 7z parsing requires the sevenz-rust crate
    // For now, return basic info
    let metadata = std::fs::metadata(path)?;

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: format!("7z archive: {} bytes", metadata.len()),
        title: None,
    })
}

pub fn parse_rar_content(path: &Path) -> Result<ParsedDocument> {
    // RAR parsing requires the unrar crate
    // For now, return basic info
    let metadata = std::fs::metadata(path)?;

    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: format!("RAR archive: {} bytes", metadata.len()),
        title: None,
    })
}
