use flash_search::{
    indexer::IndexManager,
    metadata::MetadataDb,
    settings::{AppSettings, SettingsManager},
};
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;
use flash_search::error::Result;
use notify::Watcher;

#[tokio::test]
async fn test_end_to_end_search() -> Result<()> {
    // 1. Setup temporary workspace
    let temp_workspace = tempdir()?;
    let index_dir = temp_workspace.path().join("index");
    let data_dir = temp_workspace.path().join("data");
    let settings_dir = temp_workspace.path().join("settings");

    fs::create_dir_all(&index_dir)?;
    fs::create_dir_all(&data_dir)?;
    fs::create_dir_all(&settings_dir)?;

    // 2. Create fixture files
    let txt_path = data_dir.join("hello.txt");
    fs::write(&txt_path, "This is a unique test string for searching.")?;

    let md_path = data_dir.join("notes.md");
    fs::write(&md_path, "# Notes\n\nSome markdown content with unique keyword: flashsearchintegrationtest")?;

    // 3. Initialize components
    let indexer = Arc::new(IndexManager::open(&index_dir, 100)?);
    let metadata_db = Arc::new(MetadataDb::new(&index_dir)?);

    // 4. Index files
    let txt_doc = flash_search::parsers::parse_file(&txt_path)?;
    let md_doc = flash_search::parsers::parse_file(&md_path)?;
    
    indexer.add_documents_batch(&[
        (txt_doc, 0, 100),
        (md_doc, 0, 200),
    ])?;
    indexer.commit()?;

    // 5. Search for the text file
    let results = indexer.search("unique test string", 10)?;
    assert_eq!(results.len(), 1);
    assert!(results[0].path.contains("hello.txt"));

    // 6. Search for the markdown file
    let results = indexer.search("flashsearchintegrationtest", 10)?;
    assert_eq!(results.len(), 1);
    assert!(results[0].path.contains("notes.md"));

    Ok(())
}
