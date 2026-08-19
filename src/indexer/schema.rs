use tantivy::schema::{
    FAST, INDEXED, IndexRecordOption, STORED, STRING, Schema, TEXT, TextFieldIndexing, TextOptions,
};

/// Create Tantivy schema optimized for file search with minimal posting list overhead
#[must_use]
pub fn create_schema() -> Schema {
    let mut schema_builder = Schema::builder();

    // File path - stored for retrieval, indexed as string for fast term deletes
    schema_builder.add_text_field("file_path", STRING | STORED);

    // Content - indexed full-text stream, explicitly NOT stored to save space
    let text_options = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("default")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions),
    );
    schema_builder.add_text_field("content", text_options);

    // Title - stored for display, indexed for search
    schema_builder.add_text_field("title", TEXT | STORED);

    // Modified timestamp - fast field for sorting & date range queries
    schema_builder.add_date_field("modified", FAST | INDEXED);

    // File size - fast field for range queries
    schema_builder.add_u64_field("size", FAST | INDEXED);

    // File extension - keyword indexed for exact ext filter (NOT stored in doc store)
    schema_builder.add_text_field("extension", STRING);

    schema_builder.build()
}
