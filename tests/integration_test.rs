use flash_search::error::Result;
use flash_search::{indexer::IndexManager, metadata::MetadataDb};
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_end_to_end_search() -> Result<()> {
    let temp_workspace = tempdir()?;
    let index_dir = temp_workspace.path().join("index");
    let data_dir = temp_workspace.path().join("data");
    let settings_dir = temp_workspace.path().join("settings");

    fs::create_dir_all(&index_dir)?;
    fs::create_dir_all(&data_dir)?;
    fs::create_dir_all(&settings_dir)?;

    let txt_path = data_dir.join("hello.txt");
    fs::write(&txt_path, "This is a unique test string for searching.")?;

    let md_path = data_dir.join("notes.md");
    fs::write(
        &md_path,
        "# Notes\n\nSome markdown content with unique keyword: flashsearchintegrationtest",
    )?;

    let indexer = Arc::new(IndexManager::open(&index_dir, 100)?);
    let metadata_db_path = index_dir.join("metadata.redb");
    let _metadata_db = Arc::new(MetadataDb::open(&metadata_db_path)?);

    let txt_doc = flash_search::parsers::parse_file(&txt_path)?;
    let md_doc = flash_search::parsers::parse_file(&md_path)?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    indexer.add_document(&txt_doc, now, 100)?;
    indexer.add_document(&md_doc, now, 200)?;
    indexer.commit()?;

    std::thread::sleep(std::time::Duration::from_millis(500));

    let results = indexer
        .search("unique test string", 10, None, None, None)
        .await?;
    assert_eq!(results.len(), 1);
    assert!(results[0].file_path.contains("hello.txt"));

    let results = indexer
        .search("flashsearchintegrationtest", 10, None, None, None)
        .await?;
    assert_eq!(results.len(), 1);
    assert!(results[0].file_path.contains("notes.md"));

    Ok(())
}
