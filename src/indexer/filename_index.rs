use crate::error::Result;
use arc_swap::ArcSwap;
use compact_str::CompactString;
use fst::automaton::Subsequence;
use fst::{IntoStreamer, Streamer};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

#[derive(
    Serialize, Deserialize, Debug, Clone, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize,
)]
pub struct FilenameEntry {
    pub path: String,
    pub name: CompactString,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FilenameSearchResult {
    pub file_path: String,
    pub file_name: CompactString,
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
    committed: ArcSwap<Vec<FilenameEntry>>,
    data_path: std::path::PathBuf,
    fst_map: ArcSwap<Arc<[u8]>>,
    staging: parking_lot::Mutex<Vec<FilenameEntry>>,
}

impl FilenameIndex {
    pub fn open(data_path: &Path) -> Result<Self> {
        let data_path = data_path.to_path_buf();

        let entries = if data_path.exists() {
            // Try rkyv first, then fall back to legacy JSON
            let bin_path = data_path.join(INDEX_FILENAME);
            let json_path = data_path.join(LEGACY_INDEX_FILENAME);

            if bin_path.exists() {
                match std::fs::read(&bin_path) {
                    Ok(bytes) => {
                        // Ensure byte alignment for rkyv
                        let mut aligned_bytes = rkyv::util::AlignedVec::<16>::new();
                        aligned_bytes.extend_from_slice(&bytes);

                        match rkyv::access::<rkyv::Archived<Vec<FilenameEntry>>, rkyv::rancor::Error>(
                            &aligned_bytes,
                        ) {
                            Ok(archived) => {
                                let entries: Vec<FilenameEntry> =
                                    rkyv::deserialize::<Vec<FilenameEntry>, rkyv::rancor::Error>(
                                        archived,
                                    )
                                    .unwrap_or_default();
                                tracing::info!(
                                    "Loaded {} filenames from rkyv index",
                                    entries.len()
                                );
                                entries
                            }
                            Err(e) => {
                                tracing::warn!("Failed to parse rkyv filename index: {}", e);
                                Vec::new()
                            }
                        }
                    }
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
                            // Save as rkyv immediately and remove JSON
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

        let fst_map = ArcSwap::from_pointee(Arc::from(Self::build_fst(&entries)));

        Ok(Self {
            committed: ArcSwap::from_pointee(entries),
            data_path,
            fst_map,
            staging: parking_lot::Mutex::new(Vec::new()),
        })
    }

    pub fn add_file(&self, path: &str, name: &str) -> Result<()> {
        let entry = FilenameEntry {
            path: path.to_string(),
            name: CompactString::from(name),
        };

        self.staging.lock().push(entry);
        Ok(())
    }

    pub fn commit(&self) -> Result<()> {
        let mut staging = self.staging.lock();
        if staging.is_empty() {
            return Ok(());
        }

        let new_items = std::mem::take(&mut *staging);

        // Update committed list
        let mut current = self.committed.load().as_ref().clone();
        current.extend(new_items);

        // Rebuild FST map
        self.fst_map
            .store(Arc::new(Arc::from(Self::build_fst(&current))));

        let data_path = self.data_path.clone();
        let data_to_save = current.clone();

        self.committed.store(Arc::new(current));

        std::thread::spawn(move || {
            Self::save_to_disk_sync(&data_to_save, &data_path);
        });

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

    /// Save entries to disk using rkyv (consolidated from bincode)
    fn save_to_disk_sync(entries: &Vec<FilenameEntry>, data_path: &std::path::Path) {
        match rkyv::to_bytes::<rkyv::rancor::Error>(entries) {
            Ok(bytes) => {
                let _ = std::fs::write(data_path.join(INDEX_FILENAME), bytes.as_slice());
            }
            Err(e) => {
                tracing::warn!("Failed to serialize filename index: {}", e);
            }
        }
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<FilenameSearchResult>> {
        let fst_guard = self.fst_map.load();
        if fst_guard.is_empty() {
            return Ok(Vec::new());
        }

        // FST Map - use reference borrow from the guard
        let map = match fst::Map::new(&**fst_guard) {
            Ok(m) => m,
            Err(_) => return Ok(Vec::new()),
        };

        // Fuzzy / Subsequence matching
        let query_lower = query.to_lowercase();
        let aut = Subsequence::new(&query_lower);

        let mut stream = map.search(aut).into_stream();

        let entries_lock = self.committed.load();

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
        self.committed.store(Arc::new(Vec::new()));
        self.fst_map
            .store(Arc::new(Arc::from(Vec::new().into_boxed_slice())));
        self.staging.lock().clear();

        let data_path = self.data_path.clone();
        std::thread::spawn(move || {
            let _ = std::fs::remove_file(data_path.join(INDEX_FILENAME));
            let _ = std::fs::remove_file(data_path.join(LEGACY_INDEX_FILENAME));
        });

        Ok(())
    }

    pub fn get_stats(&self) -> Result<FilenameIndexStats> {
        let entries = self.committed.load();

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
        let data_path = self.data_path.clone();
        let new_entries: Vec<FilenameEntry> = paths
            .into_iter()
            .map(|(path, name)| FilenameEntry {
                path,
                name: CompactString::from(name),
            })
            .collect();

        let data = new_entries.clone();

        self.fst_map
            .store(Arc::new(Arc::from(Self::build_fst(&new_entries))));
        self.committed.store(Arc::new(new_entries));
        self.staging.lock().clear();

        std::thread::spawn(move || {
            Self::save_to_disk_sync(&data, &data_path);
        });

        Ok(())
    }
}
