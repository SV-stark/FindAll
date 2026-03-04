use crate::commands::AppState;
use crate::settings::AppSettings;
use crate::settings::SearchHistoryItem;
use std::sync::Arc;

pub fn get_settings_internal(state: &Arc<AppState>) -> Result<AppSettings, String> {
    Ok(state.settings_cache.read().unwrap().clone())
}

pub fn save_settings_internal(settings: AppSettings, state: &Arc<AppState>) -> Result<(), String> {
    *state.settings_cache.write().unwrap() = settings.clone();
    state
        .settings_manager
        .save(&settings)
        .map_err(|e| e.to_string())?;

    let mut watcher = state.watcher.lock().unwrap();
    watcher
        .update_watch_list(settings.index_dirs)
        .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn get_recent_searches_internal(state: &Arc<AppState>) -> Result<Vec<String>, String> {
    Ok(state
        .settings_cache
        .read()
        .unwrap()
        .recent_searches
        .clone()
        .unwrap_or_default())
}

pub fn add_recent_search_internal(query: String, state: &Arc<AppState>) -> Result<(), String> {
    let mut cache = state.settings_cache.write().unwrap();

    let mut recent = cache.recent_searches.clone().unwrap_or_default();
    recent.retain(|q| q != &query);
    recent.insert(0, query);
    recent.truncate(10);

    cache.recent_searches = Some(recent);
    state
        .settings_manager
        .save(&cache)
        .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn clear_recent_searches_internal(state: &Arc<AppState>) -> Result<(), String> {
    let mut cache = state.settings_cache.write().unwrap();
    cache.recent_searches = Some(vec![]);
    state
        .settings_manager
        .save(&cache)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn add_search_history_internal(query: String, state: &Arc<AppState>) -> Result<(), String> {
    let mut cache = state.settings_cache.write().unwrap();
    let mut history = cache.search_history.clone().unwrap_or_default();

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

    cache.search_history = Some(history);
    state
        .settings_manager
        .save(&cache)
        .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn get_search_history_internal(
    limit: usize,
    state: &Arc<AppState>,
) -> Result<Vec<SearchHistoryItem>, String> {
    let mut history = state
        .settings_cache
        .read()
        .unwrap()
        .search_history
        .clone()
        .unwrap_or_default();
    history.truncate(limit);

    Ok(history)
}

pub fn pin_file_internal(path: String, state: &Arc<AppState>) -> Result<(), String> {
    let mut cache = state.settings_cache.write().unwrap();
    if !cache.pinned_files.contains(&path) {
        cache.pinned_files.push(path);
        state
            .settings_manager
            .save(&cache)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn unpin_file_internal(path: String, state: &Arc<AppState>) -> Result<(), String> {
    let mut cache = state.settings_cache.write().unwrap();
    cache.pinned_files.retain(|p| p != &path);
    state
        .settings_manager
        .save(&cache)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_pinned_files_internal(state: &Arc<AppState>) -> Result<Vec<String>, String> {
    Ok(state.settings_cache.read().unwrap().pinned_files.clone())
}
