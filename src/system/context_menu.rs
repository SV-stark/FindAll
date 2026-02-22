#[cfg(target_os = "windows")]
use crate::error::{FlashError, Result};

#[cfg(not(target_os = "windows"))]
pub fn register_context_menu(_enable: bool) -> crate::error::Result<()> {
    // Operations on non-windows don't do anything for now
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn register_context_menu(enable: bool) -> Result<()> {
    use std::env;
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let path = r#"Software\Classes\Directory\shell\FlashSearch"#;

    if enable {
        let (key, _) = hkcu
            .create_subkey(path)
            .map_err(|e| FlashError::config("context_menu", e.to_string()))?;

        key.set_value("", &"Search with Flash Search")
            .map_err(|e| FlashError::config("context_menu", e.to_string()))?;

        let icon_path = env::current_exe().unwrap_or_default();
        key.set_value("Icon", &icon_path.to_str().unwrap_or_default())
            .map_err(|e| FlashError::config("context_menu", e.to_string()))?;

        let (command_key, _) = key
            .create_subkey("command")
            .map_err(|e| FlashError::config("context_menu", e.to_string()))?;

        let command = format!("\"{}\" \"%%1\"", icon_path.to_str().unwrap_or_default());
        command_key
            .set_value("", &command)
            .map_err(|e| FlashError::config("context_menu", e.to_string()))?;
    } else {
        let _ = hkcu.delete_subkey_all(path);
    }
    Ok(())
}
