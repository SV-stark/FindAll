use slint::{ComponentHandle, ModelRc, VecModel, Model};
use std::sync::Arc;
use std::rc::Rc;
use crate::commands::AppState;
use tokio::sync::mpsc;
use crate::scanner::{ProgressEvent, ProgressType};

use tracing::info;

slint::include_modules!();



pub fn run_slint_ui(state: Arc<AppState>, mut progress_rx: mpsc::Receiver<ProgressEvent>) {
    let ui = AppWindow::new().unwrap();

    // Load initial settings and cache them
    let initial_settings = state.settings_manager.load().unwrap_or_default();
    let cached_settings = Arc::new(std::sync::Mutex::new(initial_settings.clone()));
    let current_settings = initial_settings;
    let slint_settings = AppSettings {
        theme: current_settings.theme.to_string().into(),
        index_dirs: ModelRc::from(Rc::new(VecModel::from(
            current_settings.index_dirs.iter().map(|s| s.into()).collect::<Vec<slint::SharedString>>()
        ))),
        exclude_patterns: ModelRc::from(Rc::new(VecModel::from(
            current_settings.exclude_patterns.iter().map(|s| s.into()).collect::<Vec<slint::SharedString>>()
        ))),
        auto_start: current_settings.auto_start_on_boot,
        minimize_to_tray: current_settings.minimize_to_tray,
    };
    ui.set_settings(slint_settings);
    
    // Initial stats
    let stats = state.indexer.get_statistics().unwrap_or_default();
    ui.set_files_indexed(stats.total_documents as i32);
    ui.set_index_size(format!("{:.1} MB", (stats.total_size_bytes as f64) / 1_048_576.0).into());
    
    // Progress Listener
    let ui_weak = ui.as_weak();
    tokio::spawn(async move {
        while let Some(event) = progress_rx.recv().await {
            let ui_weak_inner = ui_weak.clone();
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak_inner.upgrade() {
                    ui.set_show_progress(true);
                    let progress = if event.total > 0 {
                        event.processed as f32 / event.total as f32
                    } else {
                        0.0
                    };
                    
                    let status = event.status.clone(); // Restore this

                    // U5: Normalize status for UI
                    let ui_status = if status.contains("Scanning") {
                        "Scanning"
                    } else if status.contains("Indexing") {
                        "Indexing"
                    } else if status == "Idle" || status == "All files indexed" {
                        "Idle"
                    } else {
                        "Idle"
                    };
                    ui.set_content_status(ui_status.into());

                    match event.ptype {
                        ProgressType::Content => {
                            ui.set_content_progress(progress);
                            // ui.set_content_status(status.clone().into()); // Replaced by normalized
                            ui.set_current_file(event.current_file.clone().into());
                            ui.set_current_folder(event.current_folder.clone().into());
                            ui.set_files_per_second(format!("{:.1}", event.files_per_second).into());
                            let eta = if event.eta_seconds > 0 {
                                if event.eta_seconds < 60 {
                                    format!("{}s", event.eta_seconds)
                                } else if event.eta_seconds < 3600 {
                                    format!("{}m {}s", event.eta_seconds / 60, event.eta_seconds % 60)
                                } else {
                                    format!("{}h {}m", event.eta_seconds / 3600, (event.eta_seconds % 3600) / 60)
                                }
                            } else {
                                "...".to_string()
                            };
                            ui.set_eta_seconds(eta.into());
                        }
                        ProgressType::Filename => {
                            ui.set_filename_progress(progress);
                            ui.set_filename_status(ui_status.into());
                        }
                    }
                    
                    // Auto-hide if finished (U4)
                    if status == "Idle" || status == "All files indexed" {
                         // Delay hiding slightly? For now just hide.
                         ui.set_show_progress(false);
                    }
                }
            }).unwrap();
        }
    });

    // Set up search callback
    let ui_weak_search = ui.as_weak();
    let state_search = state.clone();
    let cached_settings_for_search = cached_settings.clone();
    
    // Track latest query to prevent race conditions (U2/P8)
    let current_search_query = Arc::new(std::sync::Mutex::new(String::new()));
    
    ui.on_perform_search(move |query, filter_type, filter_size| {
        let Some(ui_handle) = ui_weak_search.upgrade() else { return };
        // Cancel previous search by updating the "current" query
        let query = query.to_string();
        {
            let mut guard = current_search_query.lock().unwrap();
            *guard = query.clone();
        }

        let state = state_search.clone();
        let filter_type = filter_type.to_string();
        let filter_size = filter_size.to_string();
        let current_query_ref = current_search_query.clone(); // Clone Arc
        
        // Parse filters (same as before)
        let (min_size, max_size) = match filter_size.as_str() {
             "Small (< 1MB)" => (None, Some(1024 * 1024)),
             "Medium (1MB - 100MB)" => (Some(1024 * 1024), Some(100 * 1024 * 1024)),
             "Large (> 100MB)" => (Some(100 * 1024 * 1024), None),
             _ => (None, None),
        };

        // Extensions map (same as before + extras)
        let extensions: Option<Vec<String>> = match filter_type.as_str() {
             "Text" => Some(vec!["txt", "md", "rs", "toml", "json", "js", "ts", "html", "css", "c", "cpp", "h", "java", "py", "sh", "bat", "ps1", "log", "ini", "yaml", "xml", "slint", "sql"].iter().map(|s| s.to_string()).collect()),
             "Image" => Some(vec!["png", "jpg", "jpeg", "gif", "bmp", "svg", "ico", "tiff", "webp"].iter().map(|s| s.to_string()).collect()),
             "Audio" => Some(vec!["mp3", "wav", "ogg", "flac", "m4a", "aac"].iter().map(|s| s.to_string()).collect()),
             "Video" => Some(vec!["mp4", "mkv", "avi", "mov", "webm", "flv", "wmv"].iter().map(|s| s.to_string()).collect()),
             "Document" => Some(vec!["pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "rtf", "tex"].iter().map(|s| s.to_string()).collect()),
             "Archive" => Some(vec!["zip", "tar", "gz", "7z", "rar", "bz2", "xz"].iter().map(|s| s.to_string()).collect()),
             _ => None,
        };

        if query.is_empty() && extensions.is_none() && min_size.is_none() && max_size.is_none() {
             ui_handle.set_results(ModelRc::from(Rc::new(VecModel::default())));
             return;
        }

        ui_handle.set_is_searching(true);
        // Don't clear results immediately to avoid flicker? 
        // ui_handle.set_results(ModelRc::from(Rc::new(VecModel::default()))); 
        
        let ui_weak_for_task = ui_weak_search.clone();
        let settings_clone = cached_settings_for_search.clone();
        tokio::spawn(async move {
            let max_results = settings_clone.lock().unwrap().max_results;
            
            let results = if filter_type == "Filename Only" {
                 if let Some(f_index) = &state.filename_index {
                     f_index.search(&query, max_results).unwrap_or_default()
                        .into_iter()
                        .map(|r| crate::indexer::searcher::SearchResult {
                            file_path: r.file_path,
                            title: Some(r.file_name),
                            score: 1.0,
                            matched_terms: vec![], // Nucleo doesn't expose terms easily here
                            snippet: None,
                        })
                        .collect()
                 } else {
                     vec![]
                 }
            } else {
                 state.indexer.search(&query, max_results, min_size, max_size, extensions.as_deref()).await.unwrap_or_default()
            };
            
            // Check if this result is still relevant
            {
                 let guard = current_query_ref.lock().unwrap();
                 if *guard != query {
                     return; // Discard stale results
                 }
            }

            let slint_results: Vec<FileItem> = results.into_iter().map(|r| {
                let path = std::path::Path::new(&r.file_path);
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                let icon = match ext.as_str() {
                    "rs" => "rust",
                    "slint" => "code",
                    "toml" | "ini" | "cfg" | "conf" => "settings",
                    "json" | "xml" | "yaml" | "yml" => "code",
                    "md" | "txt" | "log" => "file-text",
                    "png" | "jpg" | "jpeg" | "gif" | "svg" | "bmp" | "ico" => "image",
                    "mp3" | "wav" | "ogg" | "flac" => "audio",
                    "mp4" | "mkv" | "avi" | "mov" | "webm" => "video",
                    "zip" | "tar" | "gz" | "7z" | "rar" => "archive",
                    "exe" | "msi" | "bat" | "cmd" | "sh" | "ps1" => "terminal",
                    "pdf" => "book",
                    "doc" | "docx" | "rtf" => "file-text",
                    "xls" | "xlsx" | "csv" => "file-text",
                    "ppt" | "pptx" => "file-text",
                    "js" | "ts" | "jsx" | "tsx" | "html" | "css" | "scss" => "code",
                    "c" | "cpp" | "h" | "hpp" | "cs" | "java" | "py" | "go" => "code",
                    _ => "file", 
                };
                
                FileItem {
                    title: r.file_path.split(['\\', '/']).last().unwrap_or("Unknown").into(),
                    path: r.file_path.into(),
                    score: r.score,
                    icon: icon.into(),
                    snippet: r.snippet.unwrap_or_default().into(),
                }
            }).collect();
            
            // Re-check one last time before UI update
            {
                 let guard = current_query_ref.lock().unwrap();
                 if *guard != query {
                     return; 
                 }
            }

            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak_for_task.upgrade() {
                    let model = Rc::new(VecModel::from(slint_results));
                    ui.set_results(ModelRc::from(model));
                    ui.set_is_searching(false);
                }
            }).unwrap();
        });
    });

    // U3: Rebuild Index
    let ui_weak_rebuild = ui.as_weak();
    let state_rebuild = state.clone();
    ui.on_rebuild_index(move || {
        let state = state_rebuild.clone();
        tokio::spawn(async move {
            info!("Rebuilding index...");
            // Clear existing index and metadata to force full rebuild
            if let Err(e) = state.indexer.clear() {
                eprintln!("Failed to clear index: {}", e);
            }
            if let Err(e) = state.indexer.commit() {
                eprintln!("Failed to commit cleared index: {}", e);
            }
            if let Err(e) = state.metadata_db.clear() {
                eprintln!("Failed to clear metadata: {}", e);
            }

            let settings = state.settings_manager.load().unwrap_or_default();
            for dir_str in settings.index_dirs {
                let dir = std::path::PathBuf::from(dir_str);
                if let Err(e) = state.scanner.scan_directory(dir, settings.exclude_patterns.clone()).await {
                     eprintln!("Failed to trigger scan: {}", e);
                }
            }
        });
    });

    ui.on_open_file(move |path| {
        let path_buf = std::path::PathBuf::from(path.as_str());
        if let Err(e) = opener::open(path_buf) {
            eprintln!("Failed to open file: {}", e);
        }
    });
    
    // Settings Callbacks
    let ui_weak_settings = ui.as_weak();
    let state_settings = state.clone();

    let cached_settings_for_save = cached_settings.clone();
    ui.on_save_settings(move |slint_settings| {
        let Some(ui) = ui_weak_settings.upgrade() else { return };
        // Update cache
        let mut current = cached_settings_for_save.lock().unwrap().clone();
        
        // Update fields
        current.theme = match slint_settings.theme.as_str() {
            "light" => crate::settings::Theme::Light,
            "dark" => crate::settings::Theme::Dark,
            _ => crate::settings::Theme::Auto,
        };
        
        current.index_dirs = slint_settings.index_dirs.iter().map(|s| s.to_string()).collect();
        current.exclude_patterns = slint_settings.exclude_patterns.iter().map(|s| s.to_string()).collect();
        current.auto_start_on_boot = slint_settings.auto_start;
        current.minimize_to_tray = slint_settings.minimize_to_tray;
        
        if let Err(e) = state_settings.settings_manager.save(&current) {
            eprintln!("Failed to save settings: {}", e);
        } else {
             // Update the cache if save succeeded
             *cached_settings_for_save.lock().unwrap() = current;
        }
    });

    let ui_weak_pick = ui.as_weak();
    ui.on_pick_new_folder(move || {
        let Some(ui) = ui_weak_pick.upgrade() else { return };
        let ui_handle = ui.as_weak();
        std::thread::spawn(move || {
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                let folder_str = folder.to_string_lossy().to_string();
                slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_handle.upgrade() {
                        let mut settings = ui.get_settings();
                        let mut current_dirs: Vec<slint::SharedString> = settings.index_dirs.iter().collect();
                        if !current_dirs.iter().any(|d| d.as_str() == folder_str) {
                            current_dirs.push(folder_str.into());
                            settings.index_dirs = ModelRc::from(Rc::new(VecModel::from(current_dirs)));
                            ui.set_settings(settings);
                        }
                    }
                }).unwrap();
            }
        });
    });

    let ui_weak_remove = ui.as_weak();
    ui.on_remove_folder(move |index| {
         let Some(ui) = ui_weak_remove.upgrade() else { return };
         let mut settings = ui.get_settings();
         let mut current_dirs: Vec<slint::SharedString> = settings.index_dirs.iter().collect();
         if index >= 0 && (index as usize) < current_dirs.len() {
             current_dirs.remove(index as usize);
             settings.index_dirs = ModelRc::from(Rc::new(VecModel::from(current_dirs)));
             ui.set_settings(settings);
         }
    });
    

    
    // Preview Callbacks
    let ui_weak_preview = ui.as_weak();
    ui.on_request_preview(move |path| {
        let Some(ui) = ui_weak_preview.upgrade() else { return };
        let path_str = path.to_string();
        let path_buf = std::path::PathBuf::from(&path_str);
        let weak = ui_weak_preview.clone();

        // calc title
        let file_name = path_buf.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Selected File".to_string());
        
        // Update title immediately
        let _ = weak.upgrade_in_event_loop(move |ui| {
            ui.set_preview_title(file_name.into());
            ui.set_is_preview_loading(true);
            ui.set_preview_type("none".into());
            ui.set_preview_content("Loading...".into());
        });
        
        // Reset other preview fields
        ui.set_preview_file_size("".into());
        ui.set_preview_modified("".into());
        
        if !path_buf.exists() {
             return;
        }

        // Metadata
        if let Ok(metadata) = std::fs::metadata(&path_buf) {
             let size = metadata.len();
             let size_str = if size < 1024 {
                 format!("{} B", size)
             } else if size < 1024 * 1024 {
                 format!("{:.1} KB", size as f64 / 1024.0)
             } else {
                 format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
             };
             ui.set_preview_file_size(size_str.into());
             
             // Modified time (simplistic)
             if let Ok(modified) = metadata.modified() {
                 let datetime: chrono::DateTime<chrono::Local> = modified.into();
                 ui.set_preview_modified(datetime.format("%Y-%m-%d %H:%M:%S").to_string().into());
             }
        }

        let extension = path_buf.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        
        // Image preview
        if ["png", "jpg", "jpeg", "gif", "bmp", "ico", "svg"].contains(&extension.as_str()) {
             ui.set_preview_type("image".into());
             if let Ok(image) = slint::Image::load_from_path(&path_buf) {
                 ui.set_preview_image(image);
             }
             return;
        }
        
        // Text preview (limit 10KB)
        let is_text = ["txt", "rs", "toml", "json", "md", "js", "ts", "html", "css", "slint", "py", "c", "cpp", "h", "java", "xml", "yaml", "yml", "ini", "log", "bat", "sh", "ps1"].contains(&extension.as_str());
        
        if is_text {
             ui.set_preview_type("text".into());
             // Spawn reading task
             let ui_handle = ui.as_weak();
             let p = path_buf.clone();
             std::thread::spawn(move || {
                 use std::io::Read;
                 if let Ok(file) = std::fs::File::open(&p) {
                     let mut reader = std::io::BufReader::new(file);
                     let mut buffer = [0; 10240]; // 10KB
                     if let Ok(n) = reader.read(&mut buffer) {
                         let content = String::from_utf8_lossy(&buffer[..n]);
                         let content_str = content.to_string();
                         slint::invoke_from_event_loop(move || {
                             if let Some(ui) = ui_handle.upgrade() {
                                 ui.set_preview_content(content_str.into());
                             }
                         }).unwrap();
                     }
                 }
             });
        } else {
             ui.set_preview_type("binary".into());
        }
    });

    let ui_weak_actions = ui.as_weak();
    ui.on_copy_path(move |path| {
        let path_str = path.to_string();
        std::thread::spawn(move || {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                let _ = clipboard.set_text(path_str);
            }
        });
    });

    ui.on_open_folder(move |path| {
        let path_str = path.to_string();
        let path_buf = std::path::PathBuf::from(&path_str);
        // Fix B10: Open parent folder, not the file itself
        if let Err(e) = opener::open(path_buf.parent().unwrap_or(&path_buf)) {
            eprintln!("Failed to open folder: {}", e);
        }
    });

    ui.run().unwrap();
}
