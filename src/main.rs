#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mimalloc::MiMalloc;

use std::sync::atomic::Ordering;
use tracing::{error, info};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
use tracing_appender::rolling;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

static LOG_GUARD: std::sync::OnceLock<tracing_appender::non_blocking::WorkerGuard> =
    std::sync::OnceLock::new();

fn init_logging(app_data_dir: &std::path::Path) {
    let log_dir = app_data_dir.join("logs");
    std::fs::create_dir_all(&log_dir).ok();

    let file_appender = rolling::daily(&log_dir, "flash-search.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Keep the guard alive for the lifetime of the program
    let _ = LOG_GUARD.set(guard);

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
    let thirty_days_ago = std::time::SystemTime::now() - std::time::Duration::from_hours(720);

    if let Ok(entries) = std::fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata()
                && let Ok(modified) = metadata.modified()
                && modified < thirty_days_ago
            {
                let _ = std::fs::remove_file(entry.path());
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
    let args: Vec<String> = std::env::args().collect();
    let is_cli = args.iter().any(|arg| arg == "--cli" || arg == "-c");
    if is_cli {
        let is_json = args.iter().any(|arg| arg == "--json" || arg == "-j");
        // Find the query
        let mut query = None;
        for i in 1..args.len() {
            if (args[i] == "--cli" || args[i] == "-c") && i + 1 < args.len() {
                query = Some(args[i + 1].clone());
                break;
            }
        }

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");

        let run_result = rt.block_on(async { flash_search::run_cli(query, is_json, None).await });

        if let Err(e) = run_result {
            eprintln!("CLI Error: {e}");
            std::process::exit(1);
        }
        std::process::exit(0);
    }

    let mut initial_dir = None;
    if args.len() > 1 {
        let first_arg = &args[1];
        if std::path::Path::new(first_arg).is_dir() {
            initial_dir = Some(first_arg.clone());
        }
    }

    let app_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("com.flashsearch");
    std::fs::create_dir_all(&app_dir).ok();
    let lock_path = app_dir.join("app.lock");

    let lock_file = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path)
    {
        Ok(file) => file,
        Err(e) => {
            eprintln!(
                "Error: Failed to open lock file at {}.",
                lock_path.display()
            );
            eprintln!("Details: {e}");
            std::process::exit(1);
        }
    };

    let mut lock = fd_lock::RwLock::new(lock_file);

    // We must keep the guard alive for the entire program to hold the OS lock.
    // If it fails, another instance holds it.
    let _guard_lock = lock.try_write().map_or_else(
        |_| {
            // App might already be running. Check for a stale PID.
            if let Ok(pid_str) = std::fs::read_to_string(&lock_path)
                && let Ok(pid) = pid_str.trim().parse::<u32>()
            {
                use sysinfo::System;
                let mut sys = System::new_all();
                sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
                if let Some(process) = sys.process(sysinfo::Pid::from_u32(pid))
                    && process.name().to_string_lossy().contains("flash-search")
                {
                    // Alive and is flash-search - just exit
                    std::process::exit(0);
                }
            }
            // If we reach here, it's either a stale lock or unreadable.
            tracing::warn!("Lock is blocked but PID appears stale. Continuing anyway...");
            None
        },
        |mut guard| {
            use std::io::{Seek, SeekFrom, Write};
            let _ = guard.seek(SeekFrom::Start(0));
            let _ = guard.set_len(0);
            let _ = write!(&mut *guard, "{}", std::process::id());
            let _ = guard.flush();
            Some(guard)
        },
    );

    init_logging(&app_dir);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");

    let _guard = rt.enter();

    spawn_update_checker();

    // Set up graceful shutdown
    ctrlc::set_handler(|| {
        info!("Shutdown signal received, committing index...");
        flash_search::SHUTDOWN_FLAG.store(true, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    // Run the UI
    if let Err(e) = flash_search::run_ui(initial_dir) {
        error!("Application error: {}", e);
        std::process::exit(1);
    }
}
