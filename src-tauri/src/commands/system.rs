use crate::indexer::searcher::SearchResult;

/// Get user's home directory
#[tauri::command]
pub fn get_home_dir() -> Result<String, String> {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Could not determine home directory".to_string())
}

/// Open folder and select file
#[tauri::command]
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

/// Pick a folder using native dialog
#[tauri::command]
pub async fn select_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    
    let (tx, rx) = tokio::sync::oneshot::channel();
    
    app.dialog().file().pick_folder(move |folder| {
        let _ = tx.send(folder.map(|f| f.to_string()));
    });
    
    rx.await.map_err(|e| e.to_string())
}

/// Copy text to clipboard
#[tauri::command]
pub fn copy_to_clipboard(text: String) -> Result<(), String> {
    use arboard::Clipboard;
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text).map_err(|e| e.to_string())?;
    Ok(())
}

/// Export search results to CSV
#[tauri::command]
pub async fn export_results(
    results: Vec<SearchResult>,
    format: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    use tauri_plugin_dialog::DialogExt;
    
    let (tx, rx) = tokio::sync::oneshot::channel();
    
    let extension = match format.as_str() {
        "csv" => "csv",
        "json" => "json",
        _ => "txt",
    };
    
    app.dialog().file()
        .add_filter(format.to_uppercase(), &[extension])
        .save_file(move |file_path| {
            let _ = tx.send(file_path.map(|f| f.to_string()));
        });
    
    let file_path = rx.await.map_err(|e| e.to_string())?;
    
    if let Some(path) = file_path {
        let content = match format.as_str() {
            "csv" => {
                let mut csv = String::from("File Path,Title,Score\n");
                for result in results {
                    let title = result.title.unwrap_or_default().replace('"', "\"");
                    csv.push_str(&format!("\"{}\",\"{}\",{}\n", 
                        result.file_path.replace('"', "\""),
                        title,
                        result.score
                    ));
                }
                csv
            }
            "json" => serde_json::to_string_pretty(&results).map_err(|e| e.to_string())?,
            _ => {
                let mut text = String::new();
                for result in results {
                    text.push_str(&format!("{}\t{}\t{}\n", 
                        result.file_path,
                        result.title.unwrap_or_default(),
                        result.score
                    ));
                }
                text
            }
        };
        
        tokio::fs::write(path, content).await.map_err(|e| e.to_string())?;
    }
    
    Ok(())
}
