use crate::indexer::searcher::SearchResult;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_export_results_csv() {
    let temp_dir = TempDir::new().unwrap();
    let csv_path = temp_dir.path().join("results.csv");

    let results = vec![
        SearchResult {
            file_path: "/path/to/file1.txt".to_string(),
            title: Some("File 1".to_string()),
            score: 1.5,
            matched_terms: vec!["test".to_string()],
            snippet: Some("content...".to_string()),
        },
        SearchResult {
            file_path: "/path/to/file2.txt".to_string(),
            title: Some("File 2".to_string()),
            score: 1.2,
            matched_terms: vec![],
            snippet: None,
        },
    ];

    crate::commands::export_results_csv(&results, csv_path.to_str().unwrap()).unwrap();

    let content = std::fs::read_to_string(csv_path).unwrap();
    assert!(content.contains("Score,File Path,Title"));
    assert!(content.contains("1.5,/path/to/file1.txt,File 1"));
    assert!(content.contains("1.2,/path/to/file2.txt,File 2"));
}

#[test]
fn test_export_results_json() {
    let temp_dir = TempDir::new().unwrap();
    let json_path = temp_dir.path().join("results.json");

    let results = vec![SearchResult {
        file_path: "/path/to/file1.txt".to_string(),
        title: Some("File 1".to_string()),
        score: 1.5,
        matched_terms: vec!["test".to_string()],
        snippet: Some("content...".to_string()),
    }];

    crate::commands::export_results_json(&results, json_path.to_str().unwrap()).unwrap();

    let content = std::fs::read_to_string(json_path).unwrap();
    assert!(content.contains("file_path"));
    assert!(content.contains("1.5"));
}
