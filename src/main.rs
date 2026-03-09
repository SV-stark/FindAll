#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mimalloc::MiMalloc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{error, info};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

static SHUTDOWN_FLAG: AtomicBool = AtomicBool::new(false);

pub fn is_shutting_down() -> bool {
    SHUTDOWN_FLAG.load(Ordering::SeqCst)
}

fn init_logging() -> WorkerGuard {
    let log_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("com.flashsearch")
        .join("logs");

    std::fs::create_dir_all(&log_dir).ok();

    let file_appender = RollingFileAppender::new(Rotation::DAILY, &log_dir, "flash-search.log");

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .with(fmt::layer().with_writer(std::io::stderr))
        .init();

    info!("Flash Search starting up");

    // Prune logs older than 30 days
    prune_old_logs(&log_dir);

    guard
}

fn prune_old_logs(log_dir: &std::path::Path) {
    if let Ok(entries) = std::fs::read_dir(log_dir) {
        let now = std::time::SystemTime::now();
        let max_age = std::time::Duration::from_secs(30 * 24 * 60 * 60);

        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(age) = now.duration_since(modified) {
                        if age > max_age {
                            let _ = std::fs::remove_file(entry.path());
                        }
                    }
                }
            }
        }
    }
}

fn spawn_update_checker() {
    std::thread::spawn(|| {
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
                    } else {
                        tracing::info!("Flash Search is up to date.");
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

    let lock_path = app_dir.join("flashsearch.lock");
    let lock_file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path);

    if let Ok(file) = lock_file {
        use fs2::FileExt;
        if file.try_lock_exclusive().is_err() {
            eprintln!("Flash Search is already running.");
            std::process::exit(1);
        }
        // Leak the file handle so the lock is held for the lifetime of the process
        std::mem::forget(file);
    }

    let _guard = init_logging();

    spawn_update_checker();

    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("flash-search {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    // We must create a tokio runtime for background tasks (like Watcher), but we CANNOT
    // use #[tokio::main] because Iced uses winit to hijack the main thread and blocks it.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let _rt_guard = rt.enter();

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("Flash Search - Ultrafast local full-text search\n");
        println!("Usage: flash-search [OPTIONS]");
        println!("Options:");
        println!("  -V, --version       Print version");
        println!("  -h, --help          Print help");
        println!("  -c, --cli <query>   Perform a search from the command line");
        println!("  --json              Output CLI search results as JSON");
        return;
    }

    if args.iter().any(|arg| arg == "--cli" || arg == "-c") {
        let query_index = args.iter().position(|a| a == "--cli" || a == "-c").unwrap();
        let query = args.get(query_index + 1).cloned();
        let is_json = args.iter().any(|a| a == "--json");

        rt.block_on(async {
            if let Err(e) = flash_search::run_cli(query, is_json, None).await {
                error!("CLI Error: {}", e);
            }
        });
        return;
    }

    ctrlc::set_handler(|| {
        info!("Shutdown signal received, committing index...");
        SHUTDOWN_FLAG.store(true, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    // Iced requires running on the main thread and runs its own executor
    match flash_search::run_ui() {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to start application: {}", e);
            std::process::exit(1);
        }
    }
}
