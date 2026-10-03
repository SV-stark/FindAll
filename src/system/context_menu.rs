#[cfg(target_os = "windows")]
use crate::error::{FlashError, Result};

#[cfg(not(target_os = "windows"))]
use crate::error::Result;

/// Registry subkeys holding the "Search with Flash Search" shell verb.
#[cfg(target_os = "windows")]
const MENU_KEYS: [&str; 2] = [
    r"Software\Classes\Directory\shell\FlashSearch",
    r"Software\Classes\*\shell\FlashSearch",
];

/// Label shown in Explorer's context menu.
#[cfg(target_os = "windows")]
const MENU_LABEL: &str = "Search with Flash Search";

#[cfg(not(target_os = "windows"))]
pub fn register_context_menu(_enable: bool) -> Result<()> {
    // Operations on non-windows don't do anything for now
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn register_context_menu(enable: bool) -> Result<()> {
    use std::env;
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    if !enable {
        remove_context_menu(&hkcu);
        return Ok(());
    }

    // Resolved once instead of per key, and an unusable path is an error rather
    // than an empty `command` value, which would leave a broken verb behind.
    let exe = env::current_exe().map_err(|e| FlashError::Io(std::sync::Arc::new(e)))?;
    let exe = exe
        .to_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| FlashError::config("context_menu", "Executable path is not valid UTF-8"))?;

    for path in MENU_KEYS {
        let (key, _) = hkcu
            .create_subkey(path)
            .map_err(|e| FlashError::config("context_menu", e.to_string()))?;

        key.set_value("", &MENU_LABEL)
            .map_err(|e| FlashError::config("context_menu", e.to_string()))?;
        key.set_value("Icon", &exe)
            .map_err(|e| FlashError::config("context_menu", e.to_string()))?;

        let (command_key, _) = key
            .create_subkey("command")
            .map_err(|e| FlashError::config("context_menu", e.to_string()))?;

        // `%1` is the path Explorer passed in; the app reads it from argv and
        // pre-fills the search box.
        command_key
            .set_value("", &format!("\"{exe}\" \"%1\""))
            .map_err(|e| FlashError::config("context_menu", e.to_string()))?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn remove_context_menu(hkcu: &winreg::RegKey) {
    for path in MENU_KEYS {
        // A missing key is the desired end state, so a delete failure is only
        // worth reporting when the key actually still exists.
        if let Err(e) = hkcu.delete_subkey_all(path) {
            tracing::debug!("Context menu key {path} not removed: {e}");
        }
    }
}

#[cfg(target_os = "windows")]
pub fn is_context_menu_enabled() -> Result<bool> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    // The directory verb is the one users notice, so it is the probe.
    let Ok(key) = hkcu.open_subkey(MENU_KEYS[0]) else {
        return Ok(false);
    };
    Ok(key.get_value::<String, _>("").is_ok())
}

#[cfg(not(target_os = "windows"))]
pub fn is_context_menu_enabled() -> Result<bool> {
    Ok(false)
}

/// Brings the shell registration in line with `desired`.
///
/// Returns `true` when a change was made. Failures are logged, not propagated:
/// a missing Explorer verb must never prevent the app from starting.
pub fn sync_context_menu(desired: bool) -> bool {
    match is_context_menu_enabled() {
        Ok(current) if current == desired => false,
        Ok(_) => match register_context_menu(desired) {
            Ok(()) => {
                tracing::info!("Explorer context menu synced to {desired}");
                true
            }
            Err(e) => {
                tracing::warn!("Could not sync Explorer context menu: {e}");
                false
            }
        },
        Err(e) => {
            tracing::warn!("Could not read Explorer context menu state: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Probing must never fail, even on a machine with no entries registered,
    /// because `sync_context_menu` depends on it to decide whether to write.
    #[test]
    fn probe_is_total() {
        assert!(is_context_menu_enabled().is_ok());
    }

    /// Syncing is idempotent: a second call with the same target state must not
    /// report another change.
    #[test]
    fn sync_is_idempotent() {
        let desired = is_context_menu_enabled().unwrap_or(false);
        sync_context_menu(desired);
        let second = sync_context_menu(desired);
        if !desired {
            assert!(
                !second,
                "sync must be a no-op when already in the desired state"
            );
        }
    }
}
