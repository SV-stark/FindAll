use thiserror::Error;

#[derive(Error, Debug)]
pub enum FlashError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Index error: {0}")]
    Index(#[from] tantivy::TantivyError),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Unsupported file format: {0}")]
    UnsupportedFormat(String),

    #[error("File corrupted or encrypted")]
    CorruptedFile,

    #[error("Search error: {0}")]
    Search(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Zip error: {0}")]
    Zip(#[from] zip::ZipError),
}

pub type Result<T> = std::result::Result<T, FlashError>;
