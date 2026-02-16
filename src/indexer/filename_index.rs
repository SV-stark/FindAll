use crate::error::Result;
use nucleo_matcher::pattern::{CaseMatching, Pattern};
use nucleo_matcher::Config;
use nucleo_matcher::Matcher;
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

pub struct FilenameIndex {
    entries: Arc<RwLock<Vec<FilenameEntry>>>,
    data_path: std::path::PathBuf,
}

impl FilenameIndex {
    pub fn open(data_path: &Path) -> Result<Self> {
        let data_path = data_path.to_path_buf();

        let entries = if data_path.exists() {
            match std::fs::read_to_string(data_path.join("filenames.json")) {
                Ok(content) => match serde_json::from_str::<Vec<FilenameEntry>>(&content) {
                    Ok(entries) => {
                        tracing::info!("Loaded {} filenames from disk", entries.len());
                        entries
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse filename index: {}", e);
                        Vec::new()
                    }
                },
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };

        Ok(Self {
            entries: Arc::new(RwLock::new(entries)),
            data_path,
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
                    Self::save_to_disk(&data, &data_path);
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
            std::thread::spawn(move || {
                Self::save_to_disk(&data, &data_path);
            });
        }

        Ok(())
    }

    fn save_to_disk(entries: &[FilenameEntry], data_path: &std::path::PathBuf) {
        if let Ok(json) = serde_json::to_string(entries) {
            let _ = std::fs::write(data_path.join("filenames.json"), json);
        }
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<FilenameSearchResult>> {
        let entries = self.entries.clone();

        let entries = match entries.read() {
            Ok(guard) => guard.clone(),
            Err(_) => return Ok(Vec::new()),
        };

        if entries.is_empty() {
            return Ok(Vec::new());
        }

        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();

        let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
        let pattern = Pattern::parse(query, CaseMatching::Ignore);

        let matches: Vec<_> = pattern.match_list(&names, &mut matcher);

        let mut results = Vec::with_capacity(matches.len().min(limit));

        for (idx, (_score, _matched)) in matches.into_iter().enumerate() {
            if idx >= limit {
                break;
            }

            if let Some(entry) = entries.get(idx) {
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
                let _ = std::fs::remove_file(data_path.join("filenames.json"));
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
            std::thread::spawn(move || {
                Self::save_to_disk(&data, &data_path);
            });
        }

        Ok(())
    }
}
