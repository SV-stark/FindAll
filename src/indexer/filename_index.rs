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
    fst_map: Arc<ArcSwap<Arc<[u8]>>>,
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
                std::fs::read(&bin_path).map_or_else(
                    |_| Vec::new(),
                    |bytes| {
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
                    },
                )
            } else if json_path.exists() {
                // Migrate from legacy JSON
                std::fs::read_to_string(&json_path).map_or_else(
                    |_| Vec::new(),
                    |content| match serde_json::from_str::<Vec<FilenameEntry>>(&content) {
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
                )
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let fst_map = Arc::new(ArcSwap::from_pointee(Arc::from(Self::build_fst(&entries))));

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

        let mut staging = self.staging.lock();
        staging.push(entry);
        if staging.len() >= 10000 {
            drop(staging);
            let _ = self.commit();
        }
        Ok(())
    }

    /// Add multiple files to the staging buffer in a single lock acquisition
    pub fn add_files_batch(&self, entries: Vec<FilenameEntry>) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let mut staging = self.staging.lock();
        staging.extend(entries);
        if staging.len() >= 10000 {
            drop(staging);
            let _ = self.commit();
        }
        Ok(())
    }

    pub fn commit(&self) -> Result<()> {
        let mut staging = self.staging.lock();
        if staging.is_empty() {
            return Ok(());
        }

        let new_items = std::mem::take(&mut *staging);
        drop(staging);

        // Update committed list
        let mut current = self.committed.load().as_ref().clone();
        current.extend(new_items);

        let data_path = self.data_path.clone();
        let data_to_save = current.clone();

        let fst_map_clone = Arc::clone(&self.fst_map);

        self.committed.store(Arc::new(current));

        let task = move || {
            let fst_bytes = Self::build_fst(&data_to_save);
            fst_map_clone.store(Arc::new(Arc::from(fst_bytes)));
            Self::save_to_disk_sync(&data_to_save, &data_path);
        };

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn_blocking(task);
        } else {
            std::thread::spawn(task);
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
        let Ok(map) = fst::Map::new(&**fst_guard) else {
            return Ok(Vec::new());
        };

        // Fuzzy / Subsequence matching
        let query_lower = query.to_lowercase();
        let aut = Subsequence::new(&query_lower);

        let mut stream = map.search(aut).into_stream();

        let entries_lock = self.committed.load();

        // Collect matching candidates to sort them later
        let mut candidates = Vec::new();
        while let Some((_, v)) = stream.next() {
            if let Some(entry) = entries_lock.get(usize::try_from(v).unwrap_or(usize::MAX)) {
                let score = calculate_match_score(&entry.name, &query_lower);
                candidates.push((entry, score));
            }
        }

        // Sort by score ascending (lower score means better match)
        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let results = candidates
            .into_iter()
            .take(limit)
            .map(|(entry, _)| FilenameSearchResult {
                file_path: entry.path.clone(),
                file_name: entry.name.clone(),
            })
            .collect();

        Ok(results)
    }

    pub fn clear(&self) -> Result<()> {
        self.committed.store(Arc::new(Vec::new()));
        self.fst_map
            .store(Arc::new(Arc::from(Vec::new().into_boxed_slice())));
        self.staging.lock().clear();

        let data_path = self.data_path.clone();
        let task = move || {
            let _ = std::fs::remove_file(data_path.join(INDEX_FILENAME));
            let _ = std::fs::remove_file(data_path.join(LEGACY_INDEX_FILENAME));
        };

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn_blocking(task);
        } else {
            std::thread::spawn(task);
        }

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

        let task = move || {
            Self::save_to_disk_sync(&data, &data_path);
        };

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn_blocking(task);
        } else {
            std::thread::spawn(task);
        }

        Ok(())
    }
}

fn find_subsequence_span(name: &str, query: &str) -> Option<(usize, usize)> {
    let mut query_chars = query.chars().peekable();
    let mut first_match = None;
    let mut last_match = 0;

    for (i, c) in name.char_indices() {
        if query_chars.peek() == Some(&c) {
            if first_match.is_none() {
                first_match = Some(i);
            }
            last_match = i;
            let _ = query_chars.next();
        }
    }

    if query_chars.peek().is_none() {
        Some((first_match.unwrap_or(0), last_match))
    } else {
        None
    }
}

#[allow(clippy::suboptimal_flops)]
fn calculate_match_score(name: &str, query: &str) -> f32 {
    let name_lower = name.to_lowercase();
    let query_lower = query.to_lowercase();

    if name_lower == query_lower {
        return 0.0;
    }
    if name_lower.starts_with(&query_lower) {
        return 1.0 + (name_lower.len() - query_lower.len()) as f32 * 0.001;
    }
    if let Some(idx) = name_lower.find(&query_lower) {
        return 2.0 + idx as f32 * 0.01 + (name_lower.len() - query_lower.len()) as f32 * 0.001;
    }

    if let Some((start, end)) = find_subsequence_span(&name_lower, &query_lower) {
        let span = end - start + 1;
        let gap_penalty = (span - query_lower.len()) as f32;
        return 3.0 + gap_penalty * 0.1 + start as f32 * 0.01 + name_lower.len() as f32 * 0.001;
    }

    100.0
}
