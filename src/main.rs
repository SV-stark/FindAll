#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mimalloc::MiMalloc;

use std::sync::atomic::Ordering;
use tracing::{error, info};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
use tracing_appender::rolling;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

static LOG_GUARD: std::sync::OnceLock<tracing_appender::non_blocking::WorkerGuard> =
    std::sync::OnceLock::new();

fn init_logging(app_data_dir: &std::path::Path) {
    let log_dir = app_data_dir.join("logs");
    std::fs::create_dir_all(&log_dir).ok();

    let file_appender = rolling::daily(&log_dir, "flash-search.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Keep the guard alive for the lifetime of the program
    let _ = LOG_GUARD.set(_guard);

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("flash_search=info,kreuzberg=info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .with(fmt::layer().with_writer(std::io::stderr))
        .init();

    log_panics::init();

    info!("Flash Search starting up");

    // Prune logs older than 30 days
    prune_old_logs(&log_dir);
}

fn prune_old_logs(log_dir: &std::path::Path) {
    let thirty_days_ago =
        std::time::SystemTime::now() - std::time::Duration::from_secs(30 * 24 * 3600);

    if let Ok(entries) = std::fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if modified < thirty_days_ago {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }
        }
    }
}

fn spawn_update_checker() {
    tokio::task::spawn_blocking(|| {
        tracing::info!("Checking for updates...");
        // Placeholder repo config for self_update
        let result = self_update::backends::github::Update::configure()
            .repo_owner("SV-stark")
            .repo_name("findall")
            .bin_name("flash-search")
            .show_download_progress(true)
            .current_version(env!("CARGO_PKG_VERSION"))
            .build();

        if let Ok(updater) = result {
            match updater.update() {
                Ok(status) => {
                    if status.updated() {
                        tracing::info!("Updated to version: {}", status.version());
                    }
                }
                Err(e) => tracing::warn!("Update check failed: {}", e),
            }
        }
    });
}

fn main() {
    let app_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("com.flashsearch");
    std::fs::create_dir_all(&app_dir).ok();

    let lock_file = match std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path)
    {
        Ok(file) => file,
        Err(e) => {
            eprintln!("Error: Failed to open lock file at {:?}.", lock_path);
            eprintln!("Details: {}", e);
            std::process::exit(1);
        }
    };

    let mut lock = fd_lock::RwLock::new(lock_file);
    if lock.try_write().is_err() {
        // App is already running - just exit
        // In a real app, we might want to signal the existing instance to show itself
        std::process::exit(0);
    }

    init_logging(&app_dir);
    spawn_update_checker();

    // Set up graceful shutdown
    ctrlc::set_handler(|| {
        info!("Shutdown signal received, committing index...");
        flash_search::SHUTDOWN_FLAG.store(true, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    // Run the UI
    if let Err(e) = flash_search::run_ui() {
        error!("Application error: {}", e);
        std::process::exit(1);
    }
}
