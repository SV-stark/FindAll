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
    #[must_use]
    pub fn path(mut self, path: String) -> Self {
        self.path = Some(path);
        self
    }

    #[must_use]
    pub fn title(mut self, title: Option<CompactString>) -> Self {
        self.title = title;
        self
    }

    #[must_use]
    pub fn maybe_title(self, title: Option<CompactString>) -> Self {
        self.title(title)
    }

    #[must_use]
    pub const fn modified(mut self, modified: u64) -> Self {
        self.modified = Some(modified);
        self
    }

    #[must_use]
    pub const fn size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    /// Builds a `RecentFile`.
    ///
    /// # Panics
    ///
    /// Panics if any required fields (path, modified, size) are missing.
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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementType {
    Title,
    Heading,
    NarrativeText,
    ListItem,
    CodeBlock,
    Table,
    Image,
    PageBreak,
    Formula,
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DocumentElementHighlight {
    pub element_type: ElementType,
    pub spans: Vec<(String, Option<[f32; 4]>)>,
}

/// Preview result with highlighting
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PreviewResult {
    pub elements: Vec<DocumentElementHighlight>,
    pub matched_terms: Vec<String>,
}

/// Index status
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct IndexStatus {
    pub status: String,
    pub files_indexed: usize,
}
