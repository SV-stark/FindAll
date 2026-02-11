use redb::{Database, TableDefinition, RedbKey, RedbValue, TypeName};
use serde::{Serialize, Deserialize};
use std::path::Path;
use std::time::SystemTime;
use crate::error::{FlashError, Result};

const FILES_TABLE: TableDefinition<&str, FileMetadata> = TableDefinition::new("files");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub path: String,
    pub modified: u64,           // Unix timestamp
    pub size: u64,               // File size in bytes
    pub content_hash: [u8; 32],  // Blake3 hash for content deduplication
    pub indexed_at: u64,         // When this file was last indexed
}

/// Manages file metadata database using redb
pub struct MetadataDb {
    db: Database,
}

impl MetadataDb {
    /// Open or create the metadata database
    pub fn open(db_path: &Path) -> Result<Self> {
        let db = Database::create(db_path)
            .map_err(|e| FlashError::Database(e.to_string()))?;
        
        // Create table if it doesn't exist
        let txn = db.begin_write()
            .map_err(|e| FlashError::Database(e.to_string()))?;
        {
            let _table = txn.open_table(FILES_TABLE)
                .map_err(|e| FlashError::Database(e.to_string()))?;
        }
        txn.commit()
            .map_err(|e| FlashError::Database(e.to_string()))?;
        
        Ok(Self { db })
    }
    
    /// Check if file needs reindexing based on modification time and hash
    pub fn needs_reindex(&self, path: &Path, modified: u64, size: u64) -> Result<bool> {
        let txn = self.db.begin_read()
            .map_err(|e| FlashError::Database(e.to_string()))?;
        
        let table = txn.open_table(FILES_TABLE)
            .map_err(|e| FlashError::Database(e.to_string()))?;
        
        let path_str = path.to_str().unwrap_or("");
        
        match table.get(path_str)
            .map_err(|e| FlashError::Database(e.to_string()))? {
            Some(metadata) => {
                let meta = metadata.value();
                // Reindex if modification time or size changed
                Ok(meta.modified != modified || meta.size != size)
            }
            None => Ok(true), // File not indexed yet
        }
    }
    
    /// Update file metadata after indexing
    pub fn update_metadata(
        &self,
        path: &Path,
        modified: u64,
        size: u64,
        content_hash: [u8; 32],
    ) -> Result<()> {
        let txn = self.db.begin_write()
            .map_err(|e| FlashError::Database(e.to_string()))?;
        
        let mut table = txn.open_table(FILES_TABLE)
            .map_err(|e| FlashError::Database(e.to_string()))?;
        
        let metadata = FileMetadata {
            path: path.to_string_lossy().to_string(),
            modified,
            size,
            content_hash,
            indexed_at: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        
        table.insert(path.to_str().unwrap_or(""), metadata)
            .map_err(|e| FlashError::Database(e.to_string()))?;
        
        txn.commit()
            .map_err(|e| FlashError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    /// Get metadata for a specific file
    pub fn get_metadata(&self, path: &Path) -> Result<Option<FileMetadata>> {
        let txn = self.db.begin_read()
            .map_err(|e| FlashError::Database(e.to_string()))?;
        
        let table = txn.open_table(FILES_TABLE)
            .map_err(|e| FlashError::Database(e.to_string()))?;
        
        match table.get(path.to_str().unwrap_or(""))
            .map_err(|e| FlashError::Database(e.to_string()))? {
            Some(metadata) => Ok(Some(metadata.value())),
            None => Ok(None),
        }
    }
}

// Implement RedbValue for FileMetadata to store in redb
impl RedbValue for FileMetadata {
    type SelfType<'a> = FileMetadata;
    type AsBytes<'a> = Vec<u8>;
    
    fn fixed_width() -> Option<usize> {
        None
    }
    
    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        bincode::deserialize(data).expect("Failed to deserialize FileMetadata")
    }
    
    fn as_bytes<'a, 'b: 'a>(value: &Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'a,
        Self: 'b,
    {
        bincode::serialize(value).expect("Failed to serialize FileMetadata")
    }
    
    fn type_name() -> TypeName {
        TypeName::new("FileMetadata")
    }
}
