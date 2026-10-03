use crate::commands::AppState;
use crate::settings::AppSettings;
use std::sync::Arc;

pub fn get_settings_internal(state: &Arc<AppState>) -> Result<AppSettings, String> {
    Ok(state.settings_cache.load().as_ref().clone())
}

/// Persists `settings` and pushes everything the background workers care about
/// into them.
///
/// The watcher previously only learned about new/removed directories. Editing
/// `custom_extensions` or `exclude_patterns` and saving left the watcher using
/// the values captured at construction, so it re-indexed files the scanner
/// skipped and skipped files the scanner indexed.
///
/// Search-history and pinned-file mutations go through here too rather than
/// through dedicated commands: they are ordinary settings fields, and a second
/// write path would race this one for the whole settings document.
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
    watcher.update_exclude_patterns(&settings.exclude_patterns);
    watcher.update_allowed_extensions(settings.get_allowed_extensions());
    watcher.set_enable_ocr(settings.enable_ocr);

    drop(watcher);
    Ok(())
}
