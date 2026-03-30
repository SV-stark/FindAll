use crate::indexer::searcher::SearchResult;

pub fn get_home_dir_internal() -> Result<String, String> {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Could not determine home directory".to_string())
}

pub fn open_folder_internal(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("explorer")
            .arg(format!("/select,{path}"))
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

pub async fn select_folder_internal() -> Result<Option<String>, String> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Select Folder to Index")
        .pick_folder()
        .await;
    Ok(handle.map(|h| h.path().to_string_lossy().to_string()))
}

pub fn copy_to_clipboard_internal(text: String) -> Result<(), String> {
    use arboard::Clipboard;
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text).map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn export_results_internal(
    results: Vec<SearchResult>,
    format: String,
) -> Result<(), String> {
    let mut dialog = rfd::AsyncFileDialog::new()
        .set_title("Export Search Results")
        .set_file_name(format!("flash_search_results.{format}"));

    if format == "csv" {
        dialog = dialog.add_filter("CSV File", &["csv"]);
    } else if format == "json" {
        dialog = dialog.add_filter("JSON File", &["json"]);
    }

    if let Some(handle) = dialog.save_file().await {
        let path = handle.path().to_string_lossy().to_string();
        if format == "csv" {
            crate::commands::export_results_csv(&results, &path)?;
        } else if format == "json" {
            crate::commands::export_results_json(&results, &path)?;
        }
    }

    Ok(())
}
