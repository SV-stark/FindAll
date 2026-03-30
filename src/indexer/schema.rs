use tantivy::schema::*;

/// Create Tantivy schema optimized for file search
pub fn create_schema() -> Schema {
    let mut schema_builder = Schema::builder();

    // File path - stored for retrieval, indexed for exact matches
    schema_builder.add_text_field("file_path", STRING | STORED);

    // Content - indexed for search AND stored for fast preview retrieval
    // Storing content allows fast previews without re-parsing files
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

    // Language code - indexed as keyword for filtering (e.g., lang:eng)
    schema_builder.add_text_field("language", STRING | STORED);

    // Keywords - indexed and tokenized for search visibility
    let keywords_options = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("default")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions),
    ).set_stored();
    schema_builder.add_text_field("keywords", keywords_options);

    schema_builder.build()
}
