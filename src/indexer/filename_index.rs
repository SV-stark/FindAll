use crate::error::Result;
use fst::automaton::Subsequence;
use fst::{IntoStreamer, Streamer};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FilenameEntry {
    pub path: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FilenameSearchResult {
    pub file_path: String,
    pub file_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FilenameIndexStats {
    pub total_files: usize,
    pub index_size_bytes: u64,
}

/// File extension for the binary index file
const INDEX_FILENAME: &str = "filenames.bin";
/// Legacy JSON filename for migration
const LEGACY_INDEX_FILENAME: &str = "filenames.json";

pub struct FilenameIndex {
    entries: Arc<RwLock<Vec<FilenameEntry>>>,
    data_path: std::path::PathBuf,
    fst_map: Arc<RwLock<Vec<u8>>>,
}

impl FilenameIndex {
    pub fn open(data_path: &Path) -> Result<Self> {
        let data_path = data_path.to_path_buf();

        let entries = if data_path.exists() {
            // Try bincode first, then fall back to legacy JSON
            let bin_path = data_path.join(INDEX_FILENAME);
            let json_path = data_path.join(LEGACY_INDEX_FILENAME);

            if bin_path.exists() {
                match std::fs::read(&bin_path) {
                    Ok(bytes) => match bincode::deserialize::<Vec<FilenameEntry>>(&bytes) {
                        Ok(entries) => {
                            tracing::info!("Loaded {} filenames from bincode index", entries.len());
                            entries
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse bincode filename index: {}", e);
                            Vec::new()
                        }
                    },
                    Err(_) => Vec::new(),
                }
            } else if json_path.exists() {
                // Migrate from legacy JSON
                match std::fs::read_to_string(&json_path) {
                    Ok(content) => match serde_json::from_str::<Vec<FilenameEntry>>(&content) {
                        Ok(entries) => {
                            tracing::info!(
                                "Migrated {} filenames from legacy JSON index",
                                entries.len()
                            );
                            // Save as bincode immediately and remove JSON
                            Self::save_to_disk_sync(&entries, &data_path);
                            let _ = std::fs::remove_file(&json_path);
                            entries
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse legacy JSON filename index: {}", e);
                            Vec::new()
                        }
                    },
                    Err(_) => Vec::new(),
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let fst_map = Arc::new(RwLock::new(Self::build_fst(&entries)));

        Ok(Self {
            entries: Arc::new(RwLock::new(entries)),
            data_path,
            fst_map,
        })
    }

    pub fn add_file(&self, path: &str, name: &str) -> Result<()> {
        let entry = FilenameEntry {
            path: path.to_string(),
            name: name.to_string(),
        };

        let entries = self.entries.clone();

        if let Ok(mut guard) = entries.write() {
            guard.push(entry);

            if guard.len() % 1000 == 0 {
                let data = guard.clone();
                let data_path = self.data_path.clone();
                std::thread::spawn(move || {
                    Self::save_to_disk_sync(&data, &data_path);
                });
            }
        }

        Ok(())
    }

    pub fn commit(&self) -> Result<()> {
        let entries = self.entries.clone();

        if let Ok(guard) = entries.read() {
            let data_path = self.data_path.clone();
            let data: Vec<_> = guard
                .iter()
                .map(|e| FilenameEntry {
                    path: e.path.clone(),
                    name: e.name.clone(),
                })
                .collect();

            if let Ok(mut fst_guard) = self.fst_map.write() {
                *fst_guard = Self::build_fst(&data);
            }

            std::thread::spawn(move || {
                Self::save_to_disk_sync(&data, &data_path);
            });
        }

        Ok(())
    }

    fn build_fst(entries: &[FilenameEntry]) -> Vec<u8> {
        let mut items: Vec<(String, u64)> = entries
            .iter()
            .enumerate()
            .map(|(i, e)| (format!("{}\0{}", e.name.to_lowercase(), i), i as u64))
            .collect();
        items.sort_by(|a, b| a.0.cmp(&b.0));

        let mut build = fst::MapBuilder::memory();
        for (k, v) in items {
            let _ = build.insert(k, v);
        }
        build.into_inner().unwrap_or_default()
    }

    /// Save entries to disk using bincode (P3: replaces JSON for ~10x smaller + faster)
    fn save_to_disk_sync(entries: &[FilenameEntry], data_path: &std::path::Path) {
        match bincode::serialize(entries) {
            Ok(bytes) => {
                let _ = std::fs::write(data_path.join(INDEX_FILENAME), bytes);
            }
            Err(e) => {
                tracing::warn!("Failed to serialize filename index: {}", e);
            }
        }
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<FilenameSearchResult>> {
        let fst_bytes = self.fst_map.read().unwrap();
        if fst_bytes.is_empty() {
            return Ok(Vec::new());
        }

        // FST Map
        let map = match fst::Map::new(fst_bytes.clone()) {
            Ok(m) => m,
            Err(_) => return Ok(Vec::new()),
        };

        // Fuzzy / Subsequence matching
        let query_lower = query.to_lowercase();
        let aut = Subsequence::new(&query_lower);

        let mut stream = map.search(aut).into_stream();

        let entries_lock = match self.entries.read() {
            Ok(g) => g,
            Err(_) => return Ok(Vec::new()),
        };

        let mut results = Vec::new();
        while let Some((_, v)) = stream.next() {
            if results.len() >= limit {
                break;
            }
            if let Some(entry) = entries_lock.get(v as usize) {
                results.push(FilenameSearchResult {
                    file_path: entry.path.clone(),
                    file_name: entry.name.clone(),
                });
            }
        }

        Ok(results)
    }

    pub fn clear(&self) -> Result<()> {
        let entries = self.entries.clone();

        if let Ok(mut guard) = entries.write() {
            guard.clear();
            let data_path = self.data_path.clone();
            std::thread::spawn(move || {
                let _ = std::fs::remove_file(data_path.join(INDEX_FILENAME));
                let _ = std::fs::remove_file(data_path.join(LEGACY_INDEX_FILENAME));
            });
        }

        Ok(())
    }

    pub fn get_stats(&self) -> Result<FilenameIndexStats> {
        let entries = self.entries.clone();

        let entries = match entries.read() {
            Ok(guard) => guard,
            Err(_) => {
                return Ok(FilenameIndexStats {
                    total_files: 0,
                    index_size_bytes: 0,
                })
            }
        };

        let size: u64 = entries
            .iter()
            .map(|e| e.path.len() as u64 + e.name.len() as u64 + 32)
            .sum();

        Ok(FilenameIndexStats {
            total_files: entries.len(),
            index_size_bytes: size,
        })
    }

    pub fn rebuild_index(&self, paths: Vec<(String, String)>) -> Result<()> {
        let entries = self.entries.clone();
        let data_path = self.data_path.clone();

        if let Ok(mut guard) = entries.write() {
            *guard = paths
                .into_iter()
                .map(|(path, name)| FilenameEntry { path, name })
                .collect();

            let data: Vec<_> = guard
                .iter()
                .map(|e| FilenameEntry {
                    path: e.path.clone(),
                    name: e.name.clone(),
                })
                .collect();

            if let Ok(mut fst_guard) = self.fst_map.write() {
                *fst_guard = Self::build_fst(&data);
            }

            std::thread::spawn(move || {
                Self::save_to_disk_sync(&data, &data_path);
            });
        }

        Ok(())
    }
}
