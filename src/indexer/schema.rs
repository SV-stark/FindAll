use tantivy::schema::{
    FAST, INDEXED, IndexRecordOption, STORED, STRING, Schema, TEXT, TextFieldIndexing, TextOptions,
};

/// Version of the on-disk layout.
///
/// Bump this whenever `create_schema` changes in a way that makes an existing
/// index unreadable or inconsistent. `IndexManager` compares it against the
/// stored value and rotates the old index aside for a clean rebuild rather than
/// trying to migrate documents in place.
///
/// 3.0.0: `content` is no longer STORED. Snippets are re-extracted from disk.
pub const SCHEMA_LAYOUT_VERSION: &str = "3.0.0";

/// Create Tantivy schema optimized for file search with minimal posting list overhead
#[must_use]
pub fn create_schema() -> Schema {
    let mut schema_builder = Schema::builder();

    // File path - stored for retrieval, indexed as string for fast term deletes
    schema_builder.add_text_field("file_path", STRING | STORED);

    // Content - indexed full-text stream.
    //
    // NOT stored. The field previously carried `.set_stored()`, which wrote
    // every extracted document body into the index's doc store. That made the
    // on-disk index grow with the total corpus size and forced a full
    // decompress of each hit's entire body just to produce a one-line snippet.
    // At the default `max_results` of 10_000 that is a serious I/O cliff.
    //
    // Snippets are now re-extracted from the file on demand (see
    // `crate::snippet`), and the indexed stream carries positions
    // (`WithFreqsAndPositions`) precisely so that on-disk extraction can still
    // locate and highlight the match.
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

    // File extension - keyword indexed for exact ext filter.
    // STORED so results can display the correct file-type badge/icon; without it
    // `retrieve_result_with_doc` always reads back `None` and the UI falls back
    // to a generic "FILE" badge for every result.
    schema_builder.add_text_field("extension", STRING | STORED);

    schema_builder.build()
}
