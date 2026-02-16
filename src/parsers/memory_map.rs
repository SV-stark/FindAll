use crate::error::{FlashError, Result};
use memmap2::Mmap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

const MMAP_THRESHOLD: u64 = 1024 * 1024; // 1MB
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024; // 100MB

pub fn read_file(path: &Path) -> Result<Vec<u8>> {
    let metadata = std::fs::metadata(path).map_err(|e| FlashError::Io(e))?;

    let file_size = metadata.len();

    if file_size > MAX_FILE_SIZE {
        return Err(FlashError::parse(
            path,
            format!(
                "File too large: {} bytes (max: {})",
                file_size, MAX_FILE_SIZE
            ),
        ));
    }

    if file_size > MMAP_THRESHOLD {
        read_with_mmap(path)
    } else {
        read_with_buffer(path)
    }
}

pub fn read_file_as_string(path: &Path) -> Result<String> {
    let bytes = read_file(path)?;
    String::from_utf8(bytes).map_err(|e| FlashError::parse(path, format!("Invalid UTF-8: {}", e)))
}

fn read_with_buffer(path: &Path) -> Result<Vec<u8>> {
    let mut file = File::open(path).map_err(|e| FlashError::Io(e))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|e| FlashError::Io(e))?;
    Ok(bytes)
}

fn read_with_mmap(path: &Path) -> Result<Vec<u8>> {
    let file = File::open(path)
        .map_err(|e| FlashError::parse(path, format!("Failed to open file: {}", e)))?;

    let mmap = unsafe {
        Mmap::map(&file)
            .map_err(|e| FlashError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?
    };

    Ok(mmap.to_vec())
}

pub fn is_mmap_applicable(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.len() > MMAP_THRESHOLD)
        .unwrap_or(false)
}

pub fn get_file_size(path: &Path) -> Result<u64> {
    std::fs::metadata(path)
        .map(|m| m.len())
        .map_err(|e| FlashError::Io(e))
}
