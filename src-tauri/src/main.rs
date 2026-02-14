// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Use mimalloc as the global allocator for better performance
// in allocation-heavy workloads like XML parsing and indexing
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[cfg(target_os = "windows")]
extern "system" {
    fn AttachConsole(dwProcessId: u32) -> i32;
    fn FreeConsole() -> i32;
}

#[tokio::main]
async fn main() {
    #[cfg(target_os = "windows")]
    unsafe {
        // Attempt to attach to parent process console (CMD/PowerShell)
        AttachConsole(0xFFFFFFFF);
    }

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    
    let mut search_query: Option<String> = None;
    let mut index_dir: Option<String> = None;
    let mut is_cli = false;
    
    // Parse arguments
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-s" | "--search" => {
                if i + 1 < args.len() {
                    search_query = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "-i" | "--index" => {
                if i + 1 < args.len() {
                    index_dir = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "-c" | "--cli" => {
                is_cli = true;
                i += 1;
            }
            "-h" | "--help" => {
                println!("Flash Search - Local file search tool");
                println!();
                println!("Usage:");
                println!("  flash-search [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -s, --search QUERY    Search for files");
                println!("  -i, --index DIR       Index a directory");
                println!("  -c, --cli             Run in CLI mode (output to terminal)");
                println!("  -h, --help            Show this help");
                println!();
                println!("Examples:");
                println!("  flash-search --cli --search \"report 2024\"");
                println!("  flash-search -c \"ext:pdf budget\"");
                println!("  flash-search -i C:\\Users");
                std::process::exit(0);
            }
            _ => {
                // Assume it's a search query if no flag
                if !args[i].starts_with('-') {
                    search_query = Some(args[i].clone());
                }
                i += 1;
            }
        }
    }
    
    if is_cli {
        if let Err(e) = flash_search_lib::run_cli(search_query, index_dir).await {
            eprintln!("CLI Error: {}", e);
            std::process::exit(1);
        }
    } else {
        // Pass arguments to the app
        flash_search_lib::run_with_args(search_query, index_dir);
    }
}
