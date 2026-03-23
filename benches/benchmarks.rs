use divan::black_box;
use flash_search::indexer::query_parser::ParsedQuery;

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
    ];
    for query in queries {
        let _ = ParsedQuery::new(black_box(query), black_box(false));
    }
}
