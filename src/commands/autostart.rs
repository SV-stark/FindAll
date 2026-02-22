#[cfg(target_os = "windows")]
pub fn set_auto_start(enable: bool, app_name: &str, exe_path: &str) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu
        .open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            KEY_ALL_ACCESS,
        )
        .map_err(|e| e.to_string())?;

    if enable {
        run_key
            .set_value(app_name, &exe_path)
            .map_err(|e| e.to_string())?;
    } else {
        let _ = run_key.delete_value(app_name);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn is_auto_start_enabled(app_name: &str) -> Result<bool, String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
        .map_err(|e| e.to_string())?;

    match run_key.get_value::<String, _>(app_name) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[cfg(not(target_os = "windows"))]
pub fn set_auto_start(_enable: bool, _app_name: &str, _exe_path: &str) -> Result<(), String> {
    Err("Auto-start not implemented for this platform".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn is_auto_start_enabled(_app_name: &str) -> Result<bool, String> {
    Ok(false)
}
