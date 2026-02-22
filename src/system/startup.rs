use crate::error::{FlashError, Result};
use auto_launch::AutoLaunchBuilder;
use std::env;

pub fn set_auto_start(enable: bool) -> Result<()> {
    let app_path = env::current_exe().map_err(|e| FlashError::Io(e))?;
    let app_name = "Flash Search";

    let auto = AutoLaunchBuilder::new()
        .set_app_name(app_name)
        .set_app_path(app_path.to_str().unwrap_or_default())
        .set_use_launch_agent(true)
        .build()
        .map_err(|e| FlashError::config("auto_start", e.to_string()))?;

    if enable {
        auto.enable()
            .map_err(|e| FlashError::config("auto_start_enable", e.to_string()))?;
    } else {
        if auto.is_enabled().unwrap_or(false) {
            auto.disable()
                .map_err(|e| FlashError::config("auto_start_disable", e.to_string()))?;
        }
    }
    
    Ok(())
}
