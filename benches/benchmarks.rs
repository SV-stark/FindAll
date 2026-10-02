use divan::black_box;
use flash_search::indexer::query_parser::ParsedQuery;
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
    let data = black_box([0x42u8; 65536]);
    let _ = blake3::hash(&data);
}
