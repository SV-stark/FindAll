use criterion::{black_box, criterion_group, criterion_main, Criterion};
use flash_search::{indexer::IndexManager, metadata::MetadataDb, parsers::ParsedDocument};
use std::sync::Arc;
use tempfile::tempdir;

fn perform_search(indexer: &IndexManager, query: &str) {
    let _ = indexer.search(black_box(query), black_box(50));
}

fn bench_search(c: &mut Criterion) {
    let temp = tempdir().unwrap();
    let indexer = IndexManager::open(temp.path(), 100).unwrap();

    // Create a mock dataset to index
    let mut docs = Vec::new();
    for i in 0..1000 {
        docs.push((
            ParsedDocument {
                path: format!("/fake/path/doc_{}.txt", i),
                title: Some(format!("Document Title {}", i)),
                content: format!("This is some mocked content for benchmarking. Finding needle {} in the haystack.", i),
            },
            1000 + i as u64,
            100 + i as u64,
        ));
    }

    indexer.add_documents_batch(&docs).unwrap();
    indexer.commit().unwrap();

    let mut group = c.benchmark_group("Search Index");
    group.bench_function("search_common_word", |b| {
        b.iter(|| perform_search(&indexer, "content"))
    });
    group.bench_function("search_rare_word", |b| {
        b.iter(|| perform_search(&indexer, "needle 500"))
    });
    group.bench_function("search_fuzzy", |b| {
        b.iter(|| perform_search(&indexer, "benxmarking"))
    });
    group.finish();
}

criterion_group!(benches, bench_search);
criterion_main!(benches);
