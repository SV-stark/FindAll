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

impl RecentFile {
    pub fn builder() -> RecentFileBuilder {
        RecentFileBuilder::default()
    }
}

#[derive(Default)]
pub struct RecentFileBuilder {
    path: Option<String>,
    title: Option<CompactString>,
    modified: Option<u64>,
    size: Option<u64>,
}

impl RecentFileBuilder {
    pub fn path(mut self, path: String) -> Self {
        self.path = Some(path);
        self
    }

    pub fn title(mut self, title: Option<CompactString>) -> Self {
        self.title = title;
        self
    }

    pub fn maybe_title(self, title: Option<CompactString>) -> Self {
        self.title(title)
    }

    pub fn modified(mut self, modified: u64) -> Self {
        self.modified = Some(modified);
        self
    }

    pub fn size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    pub fn build(self) -> RecentFile {
        RecentFile {
            path: self.path.expect("path is required"),
            title: self.title,
            modified: self.modified.expect("modified is required"),
            size: self.size.expect("size is required"),
        }
    }
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
    pub cached_spans: Vec<(String, Option<[f32; 4]>)>,
}

/// Index status
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct IndexStatus {
    pub status: String,
    pub files_indexed: usize,
}
