use serde::{Deserialize, Serialize};

/// Search result from index
#[derive(Serialize, Deserialize)]
pub struct SearchResult {
    pub file_path: String,
    pub title: Option<String>,
    pub score: f32,
    #[serde(default)]
    pub matched_terms: Vec<String>,
}

/// Recent file from metadata DB
#[derive(Serialize, Deserialize)]
pub struct RecentFile {
    pub path: String,
    pub title: Option<String>,
    pub modified: u64,
    pub size: u64,
}

/// Index statistics
#[derive(Serialize, Deserialize)]
pub struct IndexStatistics {
    pub total_documents: u64,
    pub total_size_bytes: u64,
    pub last_updated: Option<String>,
}

/// Filename search result
#[derive(Serialize, Deserialize)]
pub struct FilenameSearchResult {
    pub file_path: String,
    pub file_name: String,
}

/// Filename index statistics
#[derive(Serialize, Deserialize)]
pub struct FilenameIndexStats {
    pub total_files: usize,
    pub index_size_bytes: u64,
}

/// Preview result with highlighting
#[derive(Serialize, Deserialize)]
pub struct PreviewResult {
    pub content: String,
    pub matched_terms: Vec<String>,
}

/// Index status
#[derive(Serialize, Deserialize)]
pub struct IndexStatus {
    pub status: String,
    pub files_indexed: usize,
}
