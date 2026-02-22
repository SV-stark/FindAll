#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mimalloc::MiMalloc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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

    let file_appender = RollingFileAppender::new(
        Rotation::DAILY,
        log_dir,
        "flash-search.log",
    );

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .with(fmt::layer().with_writer(std::io::stderr))
        .init();

    info!("Flash Search starting up");
    
    guard
}

#[tokio::main]
async fn main() {
    let _guard = init_logging();

    let args: Vec<String> = std::env::args().collect();
    
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("flash-search {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    
    if args.iter().any(|arg| arg == "--cli" || arg == "-c") {
        let query = args.get(2).cloned();
        if let Err(e) = flash_search::run_cli(query, None).await {
            error!("CLI Error: {}", e);
        }
        return;
    }

    ctrlc::set_handler(|| {
        info!("Shutdown signal received, committing index...");
        SHUTDOWN_FLAG.store(true, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    match flash_search::run_ui() {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to start application: {}", e);
            std::process::exit(1);
        }
    }
}
