use crate::error::{FlashError, Result};
use auto_launch::{AutoLaunch, AutoLaunchBuilder};
use std::env;

/// Identifier recorded with the OS auto-start registration.
const APP_ID: &str = "com.flashsearch";

/// Builds the auto-start registrar for the running executable.
///
/// The path is resolved lazily so a failure here is reported against the
/// `auto_start` operation rather than being swallowed by a `None` default.
fn auto_launch() -> Result<AutoLaunch> {
    let app_path = env::current_exe().map_err(|e| FlashError::Io(std::sync::Arc::new(e)))?;

    AutoLaunchBuilder::new()
        .set_app_name(APP_ID)
        .set_app_path(app_path.to_str().unwrap_or_default())
        .set_macos_launch_mode(auto_launch::MacOSLaunchMode::LaunchAgent)
        .build()
        .map_err(|e| FlashError::config("auto_start", e.to_string()))
}

/// Registers or removes the OS "launch on login" entry.
pub fn set_auto_start(enable: bool) -> Result<()> {
    let auto = auto_launch()?;

    if enable {
        auto.enable()
            .map_err(|e| FlashError::config("auto_start_enable", e.to_string()))?;
    } else if auto.is_enabled().unwrap_or(false) {
        auto.disable()
            .map_err(|e| FlashError::config("auto_start_disable", e.to_string()))?;
    }

    Ok(())
}

/// Reports whether the OS currently has a launch-on-login entry registered.
///
/// Used at startup to reconcile [`AppSettings::auto_start_on_boot`] with the
/// real OS state, so a checkbox the user never touched does not claim the app
/// is registered when it is not.
pub fn is_auto_start_enabled() -> Result<bool> {
    Ok(auto_launch()?.is_enabled().unwrap_or(false))
}

/// Brings the OS registration in line with `desired`.
///
/// Returns `true` when a change was made. Failures are logged rather than
/// propagated: a user with no autostart right must still be able to run the app.
pub fn sync_auto_start(desired: bool) -> bool {
    match is_auto_start_enabled() {
        Ok(current) if current == desired => false,
        Ok(_) => match set_auto_start(desired) {
            Ok(()) => {
                tracing::info!("Auto-start registration synced to {desired}");
                true
            }
            Err(e) => {
                tracing::warn!("Could not sync auto-start registration: {e}");
                false
            }
        },
        Err(e) => {
            tracing::warn!("Could not read auto-start registration: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The builder must produce a registrar, not `None`, on every supported
    /// platform. A `None` here would silently turn every autostart toggle into
    /// a no-op, which is exactly the bug this module replaced.
    #[test]
    fn builder_resolves_for_current_executable() {
        assert!(
            auto_launch().is_ok(),
            "auto-launch builder must resolve for the running executable"
        );
    }

    /// Reconciliation must be idempotent: syncing twice to the same value is a
    /// no-op, and syncing never panics regardless of the registry's state.
    #[test]
    fn sync_is_idempotent_and_total() {
        let desired = is_auto_start_enabled().unwrap_or(false);
        let first = sync_auto_start(desired);
        let second = sync_auto_start(desired);
        assert!(
            !second || first,
            "repeated sync must not keep reporting a change"
        );
    }
}
