#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mimalloc::MiMalloc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

static SHUTDOWN_FLAG: AtomicBool = AtomicBool::new(false);

pub fn is_shutting_down() -> bool {
    SHUTDOWN_FLAG.load(Ordering::SeqCst)
}

#[tokio::main]
async fn main() {
    // Set up graceful shutdown handler
    ctrlc::set_handler(|| {
        println!("Shutdown signal received, committing index...");
        SHUTDOWN_FLAG.store(true, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    let args: Vec<String> = std::env::args().collect();
    
    if args.iter().any(|arg| arg == "--cli" || arg == "-c") {
        let query = args.get(2).cloned();
        if let Err(e) = flash_search::run_cli(query, None).await {
            eprintln!("CLI Error: {}", e);
        }
    } else {
        flash_search::run_ui();
    }
}
