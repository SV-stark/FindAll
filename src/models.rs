pub use crate::indexer::searcher::{IndexStatistics, SearchResult};
use serde::{Deserialize, Serialize};

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
