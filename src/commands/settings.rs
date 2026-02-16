use crate::commands::AppState;
use crate::models::SearchHistoryItem;
use crate::settings::AppSettings;
use std::sync::Arc;

pub fn get_settings_internal(state: &Arc<AppState>) -> Result<AppSettings, String> {
    state.settings_manager.load().map_err(|e| e.to_string())
}

pub fn save_settings_internal(settings: AppSettings, state: &Arc<AppState>) -> Result<(), String> {
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

pub fn get_recent_searches_internal(state: &Arc<AppState>) -> Result<Vec<String>, String> {
    let settings = state.settings_manager.load().map_err(|e| e.to_string())?;
    Ok(settings.recent_searches.unwrap_or_default())
}

pub fn add_recent_search_internal(query: String, state: &Arc<AppState>) -> Result<(), String> {
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

pub fn clear_recent_searches_internal(state: &Arc<AppState>) -> Result<(), String> {
    let mut settings = state.settings_manager.load().map_err(|e| e.to_string())?;
    settings.recent_searches = Some(vec![]);
    state
        .settings_manager
        .save_settings(&settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn add_search_history_internal(query: String, state: &Arc<AppState>) -> Result<(), String> {
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

pub fn get_search_history_internal(
    limit: usize,
    state: &Arc<AppState>,
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

pub fn pin_file_internal(path: String, state: &Arc<AppState>) -> Result<(), String> {
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

pub fn unpin_file_internal(path: String, state: &Arc<AppState>) -> Result<(), String> {
    let mut settings = state.settings_manager.load().map_err(|e| e.to_string())?;
    settings.pinned_files.retain(|p| p != &path);
    state
        .settings_manager
        .save_settings(&settings)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_pinned_files_internal(state: &Arc<AppState>) -> Result<Vec<String>, String> {
    let settings = state.settings_manager.load().map_err(|e| e.to_string())?;
    Ok(settings.pinned_files)
}
