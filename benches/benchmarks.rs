use divan::black_box;
use flash_search::indexer::query_parser::{ParsedQuery, extract_highlight_terms};
use flash_search::parsers::is_plaintext_fast_path;
use std::path::Path;

fn main() {
    divan::main();
}

#[divan::bench]
fn bench_query_parsing() {
    let queries = [
        "hello world",
        "ext:pdf report",
        "size:>10mb",
        "path:docs important size:<100MB",
        "\"exact phrase match\" ext:rs modified:>2026-01-01",
    ];
    for query in queries {
        let _ = ParsedQuery::new(black_box(query), black_box(false));
    }
}

#[divan::bench]
fn bench_plaintext_fast_path_check() {
    let paths = [
        "file.txt",
        "deep/path/to/main.rs",
        "document.pdf",
        "style.module.css",
        "archive.tar.gz",
    ];
    for p in paths {
        let _ = is_plaintext_fast_path(black_box(Path::new(p)));
    }
}

#[divan::bench]
fn bench_blake3_throughput() {
    // Heap-allocated so the 64 KiB buffer does not sit on the stack.
    let data = black_box(vec![0x42u8; 65_536]);
    let _ = blake3::hash(&data);
}

/// Benches the highlighting path a keystroke takes after parsing.
#[divan::bench]
fn bench_highlight_terms() {
    let queries = [
        "quarterly revenue",
        "ext:pdf annual report",
        "path:docs size:>1MB modified:week",
        "title:\"Q3 Report\" ext:xlsx docx",
    ];
    for q in queries {
        black_box(extract_highlight_terms(black_box(q), false));
    }
}
