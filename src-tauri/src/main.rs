#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    if args.iter().any(|arg| arg == "--cli" || arg == "-c") {
        let query = args.get(2).cloned();
        if let Err(e) = flash_search_lib::run_cli(query, None).await {
            eprintln!("CLI Error: {}", e);
        }
    } else {
        flash_search_lib::run_slint();
    }
}
