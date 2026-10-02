#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mimalloc::MiMalloc;

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

    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("CLI Error: failed to create the async runtime: {e}");
            std::process::exit(1);
        }
    };

    let run_result = rt.block_on(async { flash_search::run_cli(query, is_json, None).await });

    if let Err(e) = run_result {
        eprintln!("CLI Error: {e}");
        std::process::exit(1);
    }
    std::process::exit(0);
}

/// True when a process with this PID is currently running.
fn process_is_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };

        /// `STILL_ACTIVE` from the Win32 API: the process has not exited.
        const STILL_ACTIVE_CODE: u32 = 259;

        // SAFETY: a plain query-only open of a PID we read from our own lock file.
        // `code` is a valid, initialised out-parameter.
        unsafe {
            let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
                return false;
            };
            let mut code = 0u32;
            let alive =
                GetExitCodeProcess(handle, &raw mut code).is_ok() && code == STILL_ACTIVE_CODE;
            let _ = CloseHandle(handle);
            alive
        }
    }
    #[cfg(not(windows))]
    {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
}

/// Attempts to become the single running instance.
///
/// `Ok(guard)` means the lock is ours and must be held for the process lifetime.
/// `Err(AlreadyRunning)` means another *live* process holds it.
///
/// The PID file is only a fast path. The OS lock is the source of truth: a stale
/// PID left behind by a hard kill is detected by the failed `try_write`, and a
/// live process is detected by the successful one. The previous code took the
/// opposite (and dangerous) branch, continuing to start a second instance.
fn try_lock_app<'a>(
    lock: &'a mut fd_lock::RwLock<std::fs::File>,
    lock_path: &std::path::Path,
) -> Result<fd_lock::RwLockWriteGuard<'a, std::fs::File>, LockFailure> {
    if let Ok(mut guard) = lock.try_write() {
        use std::io::{Seek, SeekFrom, Write};
        let _ = guard.seek(SeekFrom::Start(0));
        let _ = guard.set_len(0);
        let _ = write!(&mut *guard, "{}", std::process::id());
        let _ = guard.flush();
        return Ok(guard);
    }

    let recorded = std::fs::read_to_string(lock_path)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok());

    match recorded {
        Some(pid) if pid != std::process::id() && process_is_alive(pid) => {
            Err(LockFailure::AlreadyRunning)
        }
        _ => {
            // Either the PID is dead (stale lock) or the file is unreadable.
            // Refuse to start rather than risk two writers on one index.
            tracing::error!(
                "Index lock at {} is held but the owning process is gone. \
                 Close any other Flash Search instance, or delete the lock \
                 file if none is running, then try again.",
                lock_path.display()
            );
            std::process::exit(1);
        }
    }
}

/// Outcome of failing to acquire the single-instance lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LockFailure {
    /// Another live instance holds the lock; exit quietly.
    AlreadyRunning,
}

fn main() {
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
    if let Err(e) = std::fs::create_dir_all(&app_dir) {
        eprintln!(
            "Error: failed to create the data directory at {}.",
            app_dir.display()
        );
        eprintln!("Details: {e}");
        std::process::exit(1);
    }

    init_logging(&app_dir);

    let lock_path = app_dir.join("app.lock");
    // `create(true)` without `truncate`: the PID inside must survive until the
    // OS lock is held, and the lock is advisory so the file contents are only
    // meaningful while we hold it.
    let lock_file = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
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

    // Two instances must never share a Tantivy directory or a redb file. The old
    // code logged "Continuing anyway" when the lock was held but the recorded PID
    // looked stale, and then carried on into `IndexManager::open` — which is how a
    // crashed-and-restarted instance could leave a corrupt index behind.
    let lock_guard = match try_lock_app(&mut lock, &lock_path) {
        Ok(guard) => guard,
        Err(LockFailure::AlreadyRunning) => {
            info!("Another instance is already running; exiting");
            return;
        }
    };

    flash_search::parsers::ensure_initialized();

    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("flash-search-worker")
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            error!("Failed to create the async runtime: {e}");
            std::process::exit(1);
        }
    };

    let _guard = rt.enter();

    spawn_update_checker();

    // Ctrl-C previously only set a flag whose log message promised
    // "committing index..." and then did nothing. The watcher and the USN journal
    // thread never read it either, so background threads kept running until the
    // process died. Now the flag is set through `request_shutdown` and the UI
    // commits before returning.
    rt.spawn(async {
        if tokio::signal::ctrl_c().await.is_ok() {
            info!("Shutdown signal received");
            flash_search::request_shutdown();
        }
    });

    // Run the UI
    let result = flash_search::run_ui(initial_dir);

    // Signal background threads, then let the runtime drain so their shutdown
    // paths complete before we release the index lock.
    flash_search::request_shutdown();
    rt.shutdown_timeout(std::time::Duration::from_secs(3));

    // Keep the lock alive until after the UI exits and the runtime has drained.
    drop(lock_guard);

    if let Err(e) = result {
        error!("Application error: {e}");
        std::process::exit(1);
    }
}
