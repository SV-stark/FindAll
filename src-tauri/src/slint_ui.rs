use slint::{ComponentHandle, ModelRc, VecModel};
use std::sync::Arc;
use std::rc::Rc;
use crate::commands::AppState;
use tokio::sync::mpsc;
use crate::scanner::{ProgressEvent, ProgressType};

slint::include_modules!();

pub fn run_slint_ui(state: Arc<AppState>, mut progress_rx: mpsc::Receiver<ProgressEvent>) {
    let ui = AppWindow::new().unwrap();
    
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
                    
                    let status = event.status.clone();
                    
                    match event.ptype {
                        ProgressType::Content => {
                            ui.set_content_progress(progress);
                            ui.set_content_status(status.clone().into());
                        }
                        ProgressType::Filename => {
                            ui.set_filename_progress(progress);
                            ui.set_filename_status(status.clone().into());
                        }
                    }
                    
                    // Auto-hide if finished
                    if status == "Idle" || status == "All files indexed" {
                         // Maybe keep visible for a bit or hide
                         // ui.set_show_progress(false);
                    }
                }
            }).unwrap();
        }
    });

    // Set up search callback
    let ui_weak_search = ui.as_weak();
    let state_search = state.clone();
    
    ui.on_perform_search(move |query| {
        let Some(ui_handle) = ui_weak_search.upgrade() else { return };
        let state = state_search.clone();
        let query = query.to_string();
        
        if query.is_empty() {
             ui_handle.set_results(ModelRc::from(Rc::new(VecModel::default())));
             return;
        }

        ui_handle.set_is_searching(true);
        
        let ui_weak_for_task = ui_weak_search.clone();
        tokio::spawn(async move {
            let results = state.indexer.search(&query, 50, None, None, None).await.unwrap_or_default();
            
            let slint_results: Vec<FileItem> = results.into_iter().map(|r| {
                FileItem {
                    title: r.file_path.split(['\\', '/']).last().unwrap_or("Unknown").into(),
                    path: r.file_path.into(),
                    score: r.score,
                }
            }).collect();
            
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak_for_task.upgrade() {
                    let model = Rc::new(VecModel::from(slint_results));
                    ui.set_results(ModelRc::from(model));
                    ui.set_is_searching(false);
                }
            }).unwrap();
        });
    });

    ui.on_open_file(move |path| {
        let path_buf = std::path::PathBuf::from(path.as_str());
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer")
                .arg("/select,")
                .arg(path_buf)
                .spawn()
                .ok();
        }
    });
    
    ui.run().unwrap();
}
