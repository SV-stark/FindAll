use crate::commands::AppState;
use crate::settings::AppSettings;
use crate::settings::SearchHistoryItem;
use std::sync::Arc;

pub fn get_settings_internal(state: &Arc<AppState>) -> Result<AppSettings, String> {
    Ok(state.settings_cache.load().as_ref().clone())
}

pub fn save_settings_internal(settings: &AppSettings, state: &Arc<AppState>) -> Result<(), String> {
    state.settings_cache.store(Arc::new(settings.clone()));

    state
        .settings_manager
        .save(settings)
        .map_err(|e| e.to_string())?;

    let mut watcher = state.watcher.lock();

    watcher
        .update_watch_list(&settings.index_dirs)
        .map_err(|e| e.to_string())?;

    drop(watcher);
    Ok(())
}

pub fn get_recent_searches_internal(state: &Arc<AppState>) -> Result<Vec<String>, String> {
    Ok(state.settings_cache.load().recent_searches.clone())
}

pub fn add_recent_search_internal(query: String, state: &Arc<AppState>) -> Result<(), String> {
    let mut cache = state.settings_cache.load().as_ref().clone();

    let mut recent = cache.recent_searches.clone();
    recent.retain(|q| q != &query);
    recent.insert(0, query);
    recent.truncate(10);

    cache.recent_searches = recent;
    state
        .settings_manager
        .save(&cache)
        .map_err(|e| e.to_string())?;

    state.settings_cache.store(Arc::new(cache));

    Ok(())
}

pub fn clear_recent_searches_internal(state: &Arc<AppState>) -> Result<(), String> {
    let mut cache = state.settings_cache.load().as_ref().clone();

    cache.recent_searches = vec![];
    state
        .settings_manager
        .save(&cache)
        .map_err(|e| e.to_string())?;

    state.settings_cache.store(Arc::new(cache));
    Ok(())
}

pub fn add_search_history_internal(query: String, state: &Arc<AppState>) -> Result<(), String> {
    let mut cache = state.settings_cache.load().as_ref().clone();

    let mut history = cache.search_history.clone();

    let mut found = false;
    for item in &mut history {
        if item.query == query {
            item.frequency += 1;
            item.last_used = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
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
                    .unwrap_or_default()
                    .as_secs(),
            },
        );
    }

    history.sort_by(|a, b| b.frequency.cmp(&a.frequency));
    history.truncate(50);

    cache.search_history = history;
    state
        .settings_manager
        .save(&cache)
        .map_err(|e| e.to_string())?;

    state.settings_cache.store(Arc::new(cache));
    Ok(())
}

pub fn get_search_history_internal(
    limit: usize,
    state: &Arc<AppState>,
) -> Result<Vec<SearchHistoryItem>, String> {
    let mut history = state.settings_cache.load().search_history.clone();
    history.truncate(limit);

    Ok(history)
}

pub fn pin_file_internal(path: String, state: &Arc<AppState>) -> Result<(), String> {
    let mut cache = state.settings_cache.load().as_ref().clone();

    if !cache.pinned_files.contains(&path) {
        cache.pinned_files.push(path);
        state
            .settings_manager
            .save(&cache)
            .map_err(|e| e.to_string())?;
        state.settings_cache.store(Arc::new(cache));
    }
    Ok(())
}

pub fn unpin_file_internal(path: &str, state: &Arc<AppState>) -> Result<(), String> {
    let mut cache = state.settings_cache.load().as_ref().clone();

    cache.pinned_files.retain(|p| p != path);
    state
        .settings_manager
        .save(&cache)
        .map_err(|e| e.to_string())?;
    state.settings_cache.store(Arc::new(cache));
    Ok(())
}

pub fn get_pinned_files_internal(state: &Arc<AppState>) -> Result<Vec<String>, String> {
    Ok(state.settings_cache.load().pinned_files.clone())
}
