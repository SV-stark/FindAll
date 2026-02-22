use tantivy::schema::*;

/// Create Tantivy schema optimized for file search
pub fn create_schema() -> Schema {
    let mut schema_builder = Schema::builder();

    // File path - stored for retrieval, indexed for exact matches
    schema_builder.add_text_field("file_path", STRING | STORED);

    // Content - indexed for search but NOT stored (to save RAM)
    // We retrieve content from disk on demand
    let text_options = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("default")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions),
    );
    schema_builder.add_text_field("content", text_options);

    // Title - stored for display, indexed for search
    schema_builder.add_text_field("title", TEXT | STORED);

    // Modified timestamp - indexed for sorting
    schema_builder.add_date_field("modified", FAST | INDEXED);

    // File size - indexed for range queries
    schema_builder.add_u64_field("size", FAST | INDEXED);
    
    // File extension - indexed as keyword for fast filtering
    schema_builder.add_text_field("extension", STRING | STORED);

    schema_builder.build()
}
