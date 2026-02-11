use std::path::Path;
use std::sync::Arc;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyError};

/// Create Tantivy schema optimized for file search
pub fn create_schema() -> Schema {
    let mut schema_builder = Schema::builder();

    // File path - stored for retrieval, indexed for exact matches
    schema_builder.add_text_field("file_path", STRING | STORED);

    // Content - indexed for search but NOT stored (to save RAM)
    // We retrieve content from disk on demand
    let text_options = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("default")
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        )
        .set_stored();
    schema_builder.add_text_field("content", text_options);

    // Title - stored for display, indexed for search
    schema_builder.add_text_field("title", TEXT | STORED);

    // Modified timestamp - indexed for sorting
    schema_builder.add_date_field("modified", FAST | INDEXED);

    schema_builder.build()
}
