pub use crate::indexer::searcher::{IndexStatistics, SearchResult};
use compact_str::CompactString;
use serde::{Deserialize, Serialize};

/// Recent file from metadata DB
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct RecentFile {
    pub path: String,
    pub title: Option<CompactString>,
    pub modified: u64,
    pub size: u64,
}

/// Filename search result
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct FilenameSearchResult {
    pub file_path: String,
    pub file_name: CompactString,
}

/// Filename index statistics
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct FilenameIndexStats {
    pub total_files: usize,
    pub index_size_bytes: u64,
}

/// Preview result with highlighting
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PreviewResult {
    pub content: String,
    pub matched_terms: Vec<String>,
}

/// Index status
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct IndexStatus {
    pub status: String,
    pub files_indexed: usize,
}
