use crate::indexer::searcher::SearchResult;

pub fn get_home_dir() -> Result<String, String> {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Could not determine home directory".to_string())
}

pub fn open_folder(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("explorer")
            .arg("/select,")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let path = std::path::PathBuf::from(path);
        if let Some(parent) = path.parent() {
            opener::reveal(parent).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

pub async fn select_folder() -> Result<Option<String>, String> {
    // Placeholder - will integrate rfd or similar for native slint
    Ok(None)
}

pub fn copy_to_clipboard(text: String) -> Result<(), String> {
    use arboard::Clipboard;
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text).map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn export_results(
    results: Vec<SearchResult>,
    format: String,
) -> Result<(), String> {
    // Placeholder for save dialog
    println!("Exporting {} results in {} format", results.len(), format);
    Ok(())
}
