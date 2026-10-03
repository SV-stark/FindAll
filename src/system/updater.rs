use self_update::backends::github::Update;

/// Summary result of checking GitHub releases for new versions.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
}

/// Checks GitHub releases for a newer version without performing installation.
pub fn check_for_updates() -> Result<UpdateCheckResult, String> {
    let current = env!("CARGO_PKG_VERSION");
    let updater = Update::configure()
        .repo_owner("SV-stark")
        .repo_name("findall")
        .bin_name("flash-search")
        .current_version(current)
        .build()
        .map_err(|e| e.to_string())?;

    let latest_release = updater.get_latest_release().map_err(|e| e.to_string())?;
    let latest_version = latest_release.version;
    let update_available =
        self_update::version::bump_is_greater(current, &latest_version).unwrap_or(false);

    Ok(UpdateCheckResult {
        current_version: current.to_string(),
        latest_version,
        update_available,
    })
}
