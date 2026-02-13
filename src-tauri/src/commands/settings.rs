use crate::commands::AppState;
use crate::models::SearchHistoryItem;
use crate::settings::AppSettings;
use std::sync::Arc;
use tauri::State;

/// Get current settings
#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> Result<AppSettings, String> {
    state.settings_manager.load().map_err(|e| e.to_string())
}

/// Save settings
#[tauri::command]
pub fn save_settings(settings: AppSettings, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state
        .settings_manager
        .save_settings(&settings)
        .map_err(|e| e.to_string())?;

    let mut watcher = state.watcher.lock().unwrap();
    watcher
        .update_watch_list(settings.index_dirs)
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Get recent searches
#[tauri::command]
pub fn get_recent_searches(state: State<'_, Arc<AppState>>) -> Result<Vec<String>, String> {
    let settings = state.settings_manager.load().map_err(|e| e.to_string())?;
    Ok(settings.recent_searches.unwrap_or_default())
}

/// Add a search to recent searches
#[tauri::command]
pub fn add_recent_search(query: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut settings = state.settings_manager.load().map_err(|e| e.to_string())?;

    let mut recent = settings.recent_searches.unwrap_or_default();
    recent.retain(|q| q != &query);
    recent.insert(0, query);
    recent.truncate(10);

    settings.recent_searches = Some(recent);
    state
        .settings_manager
        .save_settings(&settings)
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Clear recent searches
#[tauri::command]
pub fn clear_recent_searches(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut settings = state.settings_manager.load().map_err(|e| e.to_string())?;
    settings.recent_searches = Some(vec![]);
    state
        .settings_manager
        .save_settings(&settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Add to search history with frequency tracking
#[tauri::command]
pub fn add_search_history(query: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut settings = state.settings_manager.load().map_err(|e| e.to_string())?;

    let mut history = settings.search_history.unwrap_or_default();

    let mut found = false;
    for item in &mut history {
        if item.query == query {
            item.frequency += 1;
            item.last_used = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            found = true;
            break;
        }
    }

    if !found {
        history.insert(
            0,
            crate::settings::SearchHistoryItem {
                query,
                frequency: 1,
                last_used: std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            },
        );
    }

    history.sort_by(|a, b| b.frequency.cmp(&a.frequency));
    history.truncate(50);

    settings.search_history = Some(history);
    state
        .settings_manager
        .save_settings(&settings)
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Get search history sorted by frequency
#[tauri::command]
pub fn get_search_history(
    limit: usize,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<SearchHistoryItem>, String> {
    let settings = state.settings_manager.load().map_err(|e| e.to_string())?;
    let history = settings.search_history.unwrap_or_default();

    let mut sorted = history;
    sorted.sort_by(|a, b| b.frequency.cmp(&a.frequency));
    sorted.truncate(limit);

    Ok(sorted
        .into_iter()
        .map(|item| SearchHistoryItem {
            query: item.query,
            frequency: item.frequency,
            last_used: item.last_used,
        })
        .collect())
}

/// Pin a file for quick access
#[tauri::command]
pub fn pin_file(path: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut settings = state.settings_manager.load().map_err(|e| e.to_string())?;

    if !settings.pinned_files.contains(&path) {
        settings.pinned_files.push(path);
        state
            .settings_manager
            .save_settings(&settings)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Unpin a file
#[tauri::command]
pub fn unpin_file(path: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut settings = state.settings_manager.load().map_err(|e| e.to_string())?;
    settings.pinned_files.retain(|p| p != &path);
    state
        .settings_manager
        .save_settings(&settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Get pinned files
#[tauri::command]
pub fn get_pinned_files(state: State<'_, Arc<AppState>>) -> Result<Vec<String>, String> {
    let settings = state.settings_manager.load().map_err(|e| e.to_string())?;
    Ok(settings.pinned_files)
}
