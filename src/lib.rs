#![allow(clippy::missing_errors_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::similar_names)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::large_futures)]

pub mod commands;
pub mod error;
pub mod iced_ui;
pub mod indexer;
pub mod metadata;
pub mod models;
pub mod parsers;
pub mod scanner;
pub mod settings;
pub mod system;
pub mod watcher;
pub use iced_ui::{app_theme, app_title, subscription, update, view};

pub static SHUTDOWN_FLAG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[must_use]
pub fn is_shutting_down() -> bool {
    SHUTDOWN_FLAG.load(std::sync::atomic::Ordering::SeqCst)
}

/// Signals background threads to wind down.
pub fn request_shutdown() {
    SHUTDOWN_FLAG.store(true, std::sync::atomic::Ordering::SeqCst);
}

use crate::error::FlashError;
use crate::indexer::searcher::SearchParams;
use commands::AppState;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info, warn};

pub fn get_app_data_dir() -> std::result::Result<PathBuf, FlashError> {
    #[cfg(target_os = "windows")]
    let path = dirs::config_dir()
        .ok_or_else(|| FlashError::config("config_dir", "Could not find config directory"))?;

    #[cfg(not(target_os = "windows"))]
    let path = dirs::data_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".flash-search")))
        .ok_or_else(|| FlashError::config("data_dir", "Could not find data directory"))?;

    let mut path = path;
    path.push("com.flashsearch");
    Ok(path)
}

pub fn setup_app() -> std::result::Result<
    (
        Arc<AppState>,
        flume::Receiver<crate::scanner::ProgressEvent>,
    ),
    FlashError,
> {
    let app_data_dir = get_app_data_dir()?;

    if !app_data_dir.exists() {
        std::fs::create_dir_all(&app_data_dir)
            .map_err(|e| FlashError::config("create_dir", e.to_string()))?;
    }

    info!("App data directory: {:?}", app_data_dir);

    // Pre-warm Xberg extractors and registry
    parsers::ensure_initialized();

    // Create the local IPC token before anything can bind the IPC port.
    if let Err(e) = ensure_ipc_token(&app_data_dir) {
        warn!("Could not create local search IPC token: {e}");
    }

    let settings_manager = settings::SettingsManager::new(&app_data_dir);
    let settings = settings_manager.load().unwrap_or_else(|e| {
        warn!("Failed to load settings (using defaults): {e}");
        settings::AppSettings::default()
    });
    let index_path = app_data_dir.join("index");
    let indexer =
        indexer::IndexManager::open(&index_path, settings.memory_limit_mb).map_err(|e| {
            FlashError::Index {
                msg: format!("Failed to open search index: {e}"),
                field: None,
            }
        })?;
    let db_path = app_data_dir.join("metadata.redb");
    let (metadata_db, db_corrupted) = metadata::MetadataDb::open(&db_path)
        .map_err(|e| FlashError::database("open", "metadata.redb", e.to_string()))?;

    let metadata_db_shared = Arc::new(metadata_db);
    let indexer_shared = Arc::new(indexer);

    // The filename index is only built when the setting asks for it; otherwise the
    // FST is never written and never opened, so the on-disk artifact is not even
    // created for users who never switch to Filename mode.
    let filename_index = if settings.filename_index_enabled {
        match indexer::filename_index::FilenameIndex::open(&app_data_dir.join("filename_index")) {
            Ok(idx) => Some(Arc::new(idx)),
            Err(e) => {
                error!("Failed to open filename index: {}", e);
                None
            }
        }
    } else {
        info!("Filename index disabled in settings; skipping");
        None
    };

    // The watcher must see the same extension set and exclude globs the scanner
    // uses; otherwise a file the scanner skips still triggers a re-index, or vice
    // versa.
    let watcher = watcher::WatcherManager::new_with_excludes(
        Arc::clone(&indexer_shared),
        Arc::clone(&metadata_db_shared),
        settings.get_allowed_extensions().clone(),
        &settings.exclude_patterns,
        settings.enable_ocr,
    );

    let (progress_tx, progress_rx) = flume::bounded(100);

    let scanner = Arc::new(crate::scanner::Scanner::new(
        Arc::clone(&indexer_shared),
        Arc::clone(&metadata_db_shared),
        filename_index.clone(),
        Some(progress_tx.clone()),
        settings.clone(),
    ));

    let state = Arc::new(
        AppState::builder()
            .indexer(indexer_shared)
            .metadata_db(metadata_db_shared)
            .settings(settings)
            .settings_manager(settings_manager)
            .watcher(watcher)
            .maybe_filename_index(filename_index)
            .progress_tx(progress_tx)
            .scanner(scanner)
            .db_corrupted(db_corrupted)
            .build(),
    );

    Ok((state, progress_rx))
}

/// Main entry point for the Iced GUI
///
/// # Errors
///
/// Returns a `FlashError` if the GUI fails to initialize or run.
pub fn run_ui(initial_dir: Option<String>) -> std::result::Result<(), FlashError> {
    let (state_res, rx) = match setup_app() {
        Ok((state, rx)) => {
            tokio::spawn(start_ipc_server(state.clone()));
            (Ok(state), rx)
        }
        Err(e) => (Err(e.to_string()), flume::bounded(1).1),
    };

    iced_ui::run_ui(&state_res, rx, initial_dir)
}

pub async fn run_cli(
    query: Option<String>,
    is_json: bool,
    _index_path: Option<String>,
) -> crate::error::Result<()> {
    if let Some(query_str) = query {
        let (state, _) = setup_app()?;
        let results = state
            .indexer
            .search(
                SearchParams::builder()
                    .query(&query_str)
                    .limit(20)
                    .case_sensitive(false)
                    .build(),
            )
            .await?;

        if is_json {
            let json_results: Vec<serde_json::Value> = results
                .into_iter()
                .map(|res| {
                    serde_json::json!({
                        "score": res.score,
                        "path": res.file_path,
                        "title": res.title
                    })
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&json_results).unwrap_or_default()
            );
        } else {
            for res in results {
                println!("{} | {}", res.score, res.file_path);
            }
        }
    } else {
        println!("Usage: flash-search --cli <query> [--json]");
    }
    Ok(())
}

/// Runs the local search IPC server.
///
/// # Security
///
/// The listener is loopback-only and guarded by a per-user token stored in the
/// app data directory with owner-only permissions. Without the token, any local
/// process (including other users' sandboxed apps and any web page able to reach
/// `localhost`) could query the index and dump every indexed path. The previous
/// implementation was an open, unauthenticated port with no input length cap.
#[allow(clippy::too_many_lines)]
async fn start_ipc_server(state: Arc<AppState>) {
    const IPC_ADDR: &str = "127.0.0.1:9095";
    /// Maximum accepted query length in bytes.
    const MAX_QUERY_BYTES: usize = 4096;

    let app_data_dir = match get_app_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            tracing::error!("Cannot start IPC server: {e}");
            return;
        }
    };

    let token = match std::fs::read_to_string(app_data_dir.join("ipc_token")) {
        Ok(token) if !token.trim().is_empty() => token,
        _ => {
            tracing::warn!("IPC token missing; local search IPC is disabled");
            return;
        }
    };
    let token = token.trim().to_string();

    let listener = match tokio::net::TcpListener::bind(IPC_ADDR).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind IPC TCP listener at {IPC_ADDR}: {e}");
            return;
        }
    };

    tracing::info!("Local search IPC listening on {IPC_ADDR} (token required)");

    loop {
        if is_shutting_down() {
            break;
        }

        // `accept` returns an error for both transient conditions (EMFILE, ECONNABORTED)
        // and permanent ones. Sleeping briefly avoids a hot spin loop while still
        // recovering promptly.
        let accepted = match listener.accept().await {
            Ok(accepted) => accepted,
            Err(e) => {
                tracing::warn!("IPC accept failed: {e}");
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                continue;
            }
        };
        let (mut socket, _peer) = accepted;

        let state_clone = state.clone();
        let token = token.clone();
        tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

            let (reader, mut writer) = socket.split();
            let mut reader = BufReader::new(reader);

            // First line is the shared token, second line is the query.
            let mut auth_line = Vec::new();
            let read = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                reader.read_until(b'\n', &mut auth_line),
            )
            .await;

            let auth_ok = matches!(read, Ok(Ok(_)))
                && String::from_utf8_lossy(&auth_line)
                    .trim()
                    .eq_ignore_ascii_case(token.as_str());

            if !auth_ok {
                tracing::warn!("Rejected unauthenticated local search request");
                let _ = writer.write_all(b"{\"error\":\"unauthorized\"}\n").await;
                return;
            }

            // Cap the query length so a client cannot make the server buffer
            // without bound. `read_until` stops at the cap, and the leftover
            // bytes are simply never read from this short-lived connection.
            let mut line = Vec::new();
            match reader
                .take(MAX_QUERY_BYTES as u64 + 1)
                .read_until(b'\n', &mut line)
                .await
            {
                Ok(n) if n <= MAX_QUERY_BYTES => {}
                Ok(_) => {
                    let _ = writer.write_all(b"{\"error\":\"query too long\"}\n").await;
                    return;
                }
                Err(e) => {
                    tracing::warn!("IPC read failed: {e}");
                    return;
                }
            }

            let query = String::from_utf8_lossy(&line);
            let query = query.trim();
            if query.is_empty() {
                return;
            }

            let search_params = SearchParams::builder()
                .query(query)
                .limit(50)
                .case_sensitive(false)
                .build();

            let payload = match state_clone.indexer.search(search_params).await {
                Ok(results) => {
                    let json_results: Vec<serde_json::Value> = results
                        .into_iter()
                        .map(|res| {
                            serde_json::json!({
                                "score": res.score,
                                "path": res.file_path,
                                "title": res.title
                            })
                        })
                        .collect();
                    serde_json::to_string(&json_results)
                        .unwrap_or_else(|e| format!(r#"{{"error":"{e}"}}"#))
                }
                Err(e) => serde_json::json!({ "error": e.to_string() }).to_string(),
            };

            let _ = writer.write_all(payload.as_bytes()).await;
            let _ = writer.write_all(b"\n").await;
        });
    }
}

/// Generates and persists the local IPC token with owner-only permissions.
///
/// Returns the token. On non-Windows platforms the file is created with mode
/// `0600`.
pub fn ensure_ipc_token(app_data_dir: &std::path::Path) -> crate::error::Result<String> {
    use std::io::Write;

    let path = app_data_dir.join("ipc_token");
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let trimmed = existing.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    let token = {
        let mut bytes = [0u8; 32];
        getrandom(&mut bytes);
        bytes.iter().fold(String::with_capacity(64), |mut acc, b| {
            use std::fmt::Write;
            let _ = write!(acc, "{b:02x}");
            acc
        })
    };

    std::fs::create_dir_all(app_data_dir)
        .map_err(|e| FlashError::config("create_dir", e.to_string()))?;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .map_err(|e| FlashError::config("create_ipc_token", e.to_string()))?;
    file.write_all(token.as_bytes())
        .map_err(|e| FlashError::config("write_ipc_token", e.to_string()))?;

    Ok(token)
}

/// Fills `buffer` with cryptographically random bytes.
fn getrandom(buffer: &mut [u8]) {
    // Uses the `getrandom` crate, which is already in the dependency graph via
    // `rand`. A hand-rolled `BCryptGenRandom` FFI declaration was tried first and
    // crashed with an access violation: the exported function takes an opaque
    // algorithm handle whose correct "use the system RNG" value is
    // `BCRYPT_USE_SYSTEM_PREFERRED_RNG`, and calling it with a plain `u32`
    // produced garbage. This wrapper does the platform dispatch properly.
    if getrandom::fill(buffer).is_ok() {
        return;
    }

    warn!("OS CSPRNG unavailable; IPC token derived from a weak seed");
    weak_random_fill(buffer);
}

/// Last-resort fallback for [`getrandom`].
///
/// Only reachable if the OS CSPRNG is unavailable, which should not happen on any
/// supported platform. A weak token is still better than no token at all, and the
/// degradation is logged.
fn weak_random_fill(buffer: &mut [u8]) {
    /// xorshift64* multiplier; the high 32 bits of each output word have the best
    /// equidistribution.
    const SCALE: u64 = 0x2545_F491_4F6C_DD1D;

    warn!("OS CSPRNG unavailable; IPC token derived from a weak seed");

    let mut seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0x9E37_79B9_7F4A_7C15, |d| {
            u64::try_from(d.as_nanos()).unwrap_or(0x9E37_79B9_7F4A_7C15)
        })
        ^ (buffer.as_ptr() as u64);

    for slot in buffer.iter_mut() {
        seed ^= seed >> 12;
        seed ^= seed << 25;
        seed ^= seed >> 27;
        let word = seed.wrapping_mul(SCALE) >> 32;
        *slot = u8::try_from(word & 0xFF).unwrap_or(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn getrandom_fills_the_whole_buffer() {
        let mut a = [0u8; 32];
        let mut b = [0u8; 32];
        getrandom(&mut a);
        getrandom(&mut b);
        assert_ne!(a, [0u8; 32], "buffer must not be left zeroed");
        assert_ne!(a, b, "two draws must differ");
    }

    #[test]
    fn getrandom_handles_empty_and_odd_lengths() {
        getrandom(&mut []);
        let mut odd = [0u8; 7];
        getrandom(&mut odd);
        assert_ne!(odd, [0u8; 7]);
    }

    #[test]
    fn weak_random_fill_varies() {
        let mut a = [0u8; 32];
        let mut b = [0u8; 32];
        weak_random_fill(&mut a);
        weak_random_fill(&mut b);
        assert_ne!(a, [0u8; 32]);
        assert_ne!(a, b);
    }

    #[test]
    fn ipc_token_is_generated_and_reused() {
        let dir = tempdir().unwrap();
        let token = ensure_ipc_token(dir.path()).unwrap();
        assert_eq!(token.len(), 64, "32 bytes hex-encoded");
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));

        // A second call must return the same token, not rotate it.
        assert_eq!(ensure_ipc_token(dir.path()).unwrap(), token);
        assert!(dir.path().join("ipc_token").exists());
    }
}
