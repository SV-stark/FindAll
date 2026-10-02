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
        .unwrap_or_else(|_| EnvFilter::new("flash_search=info,xberg=info"));

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
    let thirty_days_ago = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_hours(720))
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

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
        match flash_search::system::updater::check_for_updates() {
            Ok(check) => {
                if check.update_available {
                    tracing::info!(
                        "Update available: current={}, latest={}",
                        check.current_version,
                        check.latest_version
                    );
                } else {
                    tracing::info!("Running latest version ({})", check.current_version);
                }
            }
            Err(e) => tracing::warn!("Update check failed: {}", e),
        }
    });
}

fn handle_cli(args: &[String]) {
    let is_json = args.iter().any(|arg| arg == "--json" || arg == "-j");
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

fn try_lock_app<'a>(
    lock: &'a mut fd_lock::RwLock<std::fs::File>,
    lock_path: &std::path::Path,
) -> Option<fd_lock::RwLockWriteGuard<'a, std::fs::File>> {
    lock.try_write().map_or_else(
        |_| {
            if let Ok(pid_str) = std::fs::read_to_string(lock_path)
                && let Ok(pid) = pid_str.trim().parse::<u32>()
            {
                #[cfg(windows)]
                {
                    use windows::Win32::Foundation::CloseHandle;
                    use windows::Win32::System::Threading::{
                        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
                    };
                    if let Ok(handle) =
                        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
                        && !handle.is_invalid()
                    {
                        unsafe {
                            let _ = CloseHandle(handle);
                        };
                        std::process::exit(0);
                    }
                }
                #[cfg(unix)]
                {
                    if std::path::Path::new(&format!("/proc/{pid}")).exists() {
                        std::process::exit(0);
                    }
                }
            }
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
    )
}

fn main() {
    flash_search::parsers::ensure_initialized();

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--cli" || arg == "-c") {
        handle_cli(&args);
    }

    let mut initial_dir = None;
    if args.len() > 1 {
        let first_arg = &args[1];
        if std::path::Path::new(first_arg).is_dir() {
            initial_dir = Some(first_arg.clone());
        }
    }

    let app_dir =
        flash_search::get_app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    std::fs::create_dir_all(&app_dir).ok();
    let lock_path = app_dir.join("app.lock");

    let lock_file = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
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

    // Guard kept alive for lifetime of program to hold OS lock
    let _guard_lock = try_lock_app(&mut lock, &lock_path);

    init_logging(&app_dir);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");

    let _guard = rt.enter();

    spawn_update_checker();

    // Set up graceful shutdown via Tokio signal
    rt.spawn(async {
        if tokio::signal::ctrl_c().await.is_ok() {
            info!("Shutdown signal received, committing index...");
            flash_search::SHUTDOWN_FLAG.store(true, Ordering::SeqCst);
        }
    });

    // Run the UI
    if let Err(e) = flash_search::run_ui(initial_dir) {
        error!("Application error: {}", e);
        std::process::exit(1);
    }
}
