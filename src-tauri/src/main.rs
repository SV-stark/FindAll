// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Use mimalloc as the global allocator for better performance
// in allocation-heavy workloads like XML parsing and indexing
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    
    let mut search_query: Option<String> = None;
    let mut index_dir: Option<String> = None;
    
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
            "-h" | "--help" => {
                println!("Flash Search - Local file search tool");
                println!();
                println!("Usage:");
                println!("  flash-search [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -s, --search QUERY    Search for files");
                println!("  -i, --index DIR       Index a directory");
                println!("  -h, --help            Show this help");
                println!();
                println!("Examples:");
                println!("  flash-search --search \"report 2024\"");
                println!("  flash-search -i C:\\Users");
                println!("  flash-search -s \"ext:pdf budget\"");
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
    
    // Pass arguments to the app
    flash_search_lib::run_with_args(search_query, index_dir);
}
