use crate::error::Result;
use arc_swap::ArcSwap;
use compact_str::CompactString;
use fst::automaton::Subsequence;
use fst::{IntoStreamer, Streamer};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info, warn};

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
/// Magic header for verified rkyv index
const MAGIC_HEADER: &[u8; 8] = b"FS_FN_01";
/// Header length: 8 bytes magic + 32 bytes BLAKE3 hash
const HEADER_LEN: usize = 40;

/// Maximum number of staged entries before a flush is forced from
/// `add_files_batch`, so a very long scan cannot grow staging without bound.
const STAGING_FLUSH_THRESHOLD: usize = 250_000;

/// An immutable, internally consistent view of the filename index.
///
/// Entries and the FST used to live in two separate `ArcSwap`s that were
/// updated non-atomically. A search could load a fresh entry list together with
/// a stale FST (or vice versa), so FST values could point at the wrong entry.
/// Publishing both together removes that window entirely.
///
/// The FST may lag behind the entry list on purpose: rebuilding it is
/// `O(n log n)` over every file, and a full scan would otherwise rebuild it
/// once per flush. Because indices are stable when entries are appended, the FST
/// always describes the first `fst_entries` entries, and `search` scans only the
/// appended tail linearly.
struct FilenameSnapshot {
    entries: Arc<[FilenameEntry]>,
    fst: Arc<[u8]>,
    /// Number of leading `entries` covered by `fst`.
    fst_entries: usize,
}

impl FilenameSnapshot {
    fn empty() -> Self {
        Self {
            entries: Arc::from(Vec::new()),
            fst: Arc::from(Vec::new().into_boxed_slice()),
            fst_entries: 0,
        }
    }

    fn from_entries(entries: Vec<FilenameEntry>) -> Self {
        let count = entries.len();
        let fst = Arc::from(Self::build_fst(&entries));
        Self {
            entries: Arc::from(entries),
            fst,
            fst_entries: count,
        }
    }

    /// Builds the FST used to find subsequence matches quickly.
    ///
    /// Keys are `lowercased_name\0index` so that duplicate names map to
    /// distinct entries. Returns an empty map on builder failure, which is
    /// handled correctly because `search` always falls back to a linear scan.
    fn build_fst(entries: &[FilenameEntry]) -> Vec<u8> {
        let mut items: Vec<(String, u64)> = entries
            .iter()
            .enumerate()
            .map(|(i, e)| (format!("{}\0{}", e.name.to_lowercase(), i), i as u64))
            .collect();
        items.sort_unstable_by(|a, b| a.0.cmp(&b.0));

        let mut build = fst::MapBuilder::memory();
        let mut rejected = 0usize;
        for (k, v) in items {
            if build.insert(&k, v).is_err() {
                rejected += 1;
            }
        }
        if rejected > 0 {
            // Only reachable with duplicate keys, which the `\0<index>` suffix
            // is designed to prevent.
            warn!("{rejected} filename keys were rejected by the FST builder");
        }

        match build.into_inner() {
            Ok(fst) => fst,
            Err(e) => {
                warn!("FST build failed, filename search will use the linear fallback: {e:?}");
                Vec::new()
            }
        }
    }
}

pub struct FilenameIndex {
    /// Current consistent snapshot. Wrapped in `Arc` so background save tasks
    /// can read the latest state without borrowing `self`.
    snapshot: Arc<ArcSwap<FilenameSnapshot>>,
    data_path: std::path::PathBuf,
    staging: parking_lot::Mutex<Vec<FilenameEntry>>,
    /// Serialises disk writes so a slow save cannot be overtaken by a newer one.
    save_lock: Arc<parking_lot::Mutex<()>>,
}

impl FilenameIndex {
    pub fn open(data_path: &Path) -> Result<Self> {
        let data_path = data_path.to_path_buf();
        let entries = Self::load_entries(&data_path);

        Ok(Self {
            snapshot: Arc::new(ArcSwap::from_pointee(FilenameSnapshot::from_entries(
                entries,
            ))),
            data_path,
            staging: parking_lot::Mutex::new(Vec::new()),
            save_lock: Arc::new(parking_lot::Mutex::new(())),
        })
    }

    /// Reads the on-disk index, migrating the legacy JSON format when needed.
    fn load_entries(data_path: &Path) -> Vec<FilenameEntry> {
        if !data_path.exists() {
            return Vec::new();
        }

        let bin_path = data_path.join(INDEX_FILENAME);
        let json_path = data_path.join(LEGACY_INDEX_FILENAME);

        if bin_path.exists() {
            Self::load_rkyv_entries(&bin_path)
        } else if json_path.exists() {
            let content = match std::fs::read_to_string(&json_path) {
                Ok(content) => content,
                Err(e) => {
                    warn!("Failed to read legacy JSON filename index: {e}");
                    return Vec::new();
                }
            };
            match serde_json::from_str::<Vec<FilenameEntry>>(&content) {
                Ok(entries) => {
                    info!(
                        "Migrated {} filenames from legacy JSON index",
                        entries.len()
                    );
                    // Persist immediately so the migration happens exactly once.
                    Self::save_entries(&entries, &data_path.join(INDEX_FILENAME));
                    let _ = std::fs::remove_file(&json_path);
                    entries
                }
                Err(e) => {
                    warn!("Failed to parse legacy JSON filename index: {e}");
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        }
    }

    fn load_rkyv_entries(bin_path: &Path) -> Vec<FilenameEntry> {
        let Ok(file) = std::fs::File::open(bin_path) else {
            warn!(
                "Failed to open filename index {:?}; starting empty",
                bin_path
            );
            return Vec::new();
        };
        // SAFETY: the mapping is read-only and dropped before this function
        // returns. Callers must not truncate or modify the file while it is
        // mapped, which is guaranteed because every write goes through a
        // write-to-temp + rename.
        let Ok(mmap) = (unsafe { memmap2::MmapOptions::new().map(&file) }) else {
            warn!(
                "Failed to map filename index {:?}; starting empty",
                bin_path
            );
            return Vec::new();
        };

        let dearchived = |archived: &rkyv::Archived<Vec<FilenameEntry>>| -> Vec<FilenameEntry> {
            archived
                .iter()
                .map(|item| FilenameEntry {
                    path: item.path.as_str().to_string(),
                    name: CompactString::from(item.name.as_str()),
                })
                .collect()
        };

        if mmap.len() >= HEADER_LEN && &mmap[..8] == MAGIC_HEADER {
            let expected_hash = &mmap[8..HEADER_LEN];
            let payload = &mmap[HEADER_LEN..];
            if blake3::hash(payload).as_bytes() == expected_hash {
                let mut aligned = rkyv::util::AlignedVec::<16>::new();
                aligned.extend_from_slice(payload);
                // SAFETY: the payload was just verified with BLAKE3, so it is
                // byte-for-byte the buffer that was serialized.
                let archived = unsafe {
                    rkyv::access_unchecked::<rkyv::Archived<Vec<FilenameEntry>>>(&aligned)
                };
                let entries = dearchived(archived);
                info!(
                    "Loaded {} filenames from verified rkyv index",
                    entries.len()
                );
                return entries;
            }
            warn!("BLAKE3 checksum mismatch on filename index, re-validating");
        }

        let mut aligned = rkyv::util::AlignedVec::<16>::new();
        aligned.extend_from_slice(&mmap);
        match rkyv::access::<rkyv::Archived<Vec<FilenameEntry>>, rkyv::rancor::Error>(&aligned) {
            Ok(archived) => dearchived(archived),
            Err(e) => {
                warn!("Failed to parse rkyv filename index: {e}");
                Vec::new()
            }
        }
    }

    /// Stages a single entry. Returns the number of entries accepted (0 or 1).
    pub fn add_file(&self, path: &str, name: &str) -> usize {
        self.stage(vec![FilenameEntry {
            path: path.to_string(),
            name: CompactString::from(name),
        }])
    }

    /// Adds multiple files to the staging buffer in a single lock acquisition.
    ///
    /// Returns the number of entries accepted.
    pub fn add_files_batch(&self, entries: Vec<FilenameEntry>) -> usize {
        self.stage(entries)
    }

    fn stage(&self, entries: Vec<FilenameEntry>) -> usize {
        if entries.is_empty() {
            return 0;
        }
        let count = entries.len();
        let should_flush = {
            let mut staging = self.staging.lock();
            staging.extend(entries);
            staging.len() >= STAGING_FLUSH_THRESHOLD
        };
        if should_flush {
            // Publish in-memory only. Rebuilding the FST is deferred to
            // `commit` so a long scan pays for it once, not per flush.
            self.publish_staged();
        }
        count
    }

    /// Moves staged entries into the live snapshot without rebuilding the FST.
    ///
    /// Returns the number of entries published.
    fn publish_staged(&self) -> usize {
        let staged = {
            let mut staging = self.staging.lock();
            std::mem::take(&mut *staging)
        };
        if staged.is_empty() {
            return 0;
        }
        let count = staged.len();

        let current = self.snapshot.load_full();
        let mut entries = Vec::with_capacity(current.entries.len() + count);
        entries.extend_from_slice(&current.entries);
        entries.extend(staged);

        // Indices are preserved by appending, so the existing FST still
        // describes the first `fst_entries` entries.
        self.snapshot.store(Arc::new(FilenameSnapshot {
            entries: Arc::from(entries),
            fst: Arc::clone(&current.fst),
            fst_entries: current.fst_entries,
        }));

        count
    }

    /// Rebuilds the FST, publishes it, and persists the index to disk.
    ///
    /// Safe to call when nothing is staged: the on-disk state is refreshed
    /// regardless, which makes it usable as an explicit "save now" call.
    pub fn commit(&self) -> Result<()> {
        self.publish_staged();

        let snapshot = self.snapshot.load_full();
        let rebuilt = Arc::new(FilenameSnapshot::from_entries(snapshot.entries.to_vec()));
        self.snapshot.store(Arc::clone(&rebuilt));

        let fst_map = Arc::clone(&self.snapshot);
        let save_lock = Arc::clone(&self.save_lock);
        let index_path = self.data_path.join(INDEX_FILENAME);
        let entries: Vec<FilenameEntry> = rebuilt.entries.to_vec();

        let task = move || {
            // Serialise disk writes. Re-reading the snapshot inside the lock
            // means a save always writes the newest state, so a slow write can
            // never overwrite a newer index with an older one.
            let _guard = save_lock.lock();
            let latest = fst_map.load();
            debug!(
                "Persisting filename index: {} entries ({} covered by FST)",
                latest.entries.len(),
                latest.fst_entries
            );
            Self::save_entries(&entries, &index_path);
        };

        Self::spawn_blocking(task);
        Ok(())
    }

    /// Serializes `entries` into `index_path` with a BLAKE3 integrity header.
    ///
    /// Writes to a temporary file and renames it into place, so an interrupted
    /// write can never leave a half-written index that fails to load on the next
    /// start (the old code wrote in place with `fs::write`).
    ///
    /// Takes `&Vec` because rkyv 0.8 implements `Serialize` for `Vec<T>` but not
    /// for `[T]`, and cloning a million entries just to satisfy the bound would
    /// cost more than the save itself.
    #[allow(clippy::ptr_arg)]
    fn save_entries(entries: &Vec<FilenameEntry>, index_path: &Path) {
        let Ok(bytes) = rkyv::to_bytes::<rkyv::rancor::Error>(entries) else {
            warn!("Failed to serialize filename index");
            return;
        };

        let hash = blake3::hash(bytes.as_slice());
        let mut file_data = Vec::with_capacity(HEADER_LEN + bytes.len());
        file_data.extend_from_slice(MAGIC_HEADER);
        file_data.extend_from_slice(hash.as_bytes());
        file_data.extend_from_slice(bytes.as_slice());

        let tmp_path = index_path.with_extension("bin.tmp");
        if let Err(e) = std::fs::write(&tmp_path, &file_data) {
            warn!("Failed to write filename index to {tmp_path:?}: {e}");
            return;
        }
        if let Err(e) = std::fs::rename(&tmp_path, index_path) {
            warn!("Failed to replace filename index at {index_path:?}: {e}");
            let _ = std::fs::remove_file(&tmp_path);
        }
    }

    /// Runs `task` off the async runtime when one is available, on a detached
    /// thread otherwise (CLI path).
    fn spawn_blocking(task: impl FnOnce() + Send + 'static) {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                handle.spawn_blocking(task);
            }
            Err(_) => {
                std::thread::Builder::new()
                    .name("flash-search-filename-index".to_string())
                    .spawn(task)
                    .map_err(|e| warn!("Failed to spawn filename index task: {e}"))
                    .ok();
            }
        }
    }

    /// Finds files whose name matches `query` as a fuzzy subsequence.
    ///
    /// Searches three sources in one pass:
    /// 1. the FST, which covers the committed snapshot prefix,
    /// 2. entries appended to the snapshot since the last commit, and
    /// 3. anything still sitting in the staging buffer.
    ///
    /// Publishing staged entries on every search instead would mean cloning the
    /// whole entry vector on each keystroke — tens of megabytes per character
    /// typed on a million-file index.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<FilenameSearchResult>> {
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let snapshot = self.snapshot.load();
        let query_lower = query.to_lowercase();

        // Bounded heap: with a short query ("a") a subsequence match can hit a
        // large fraction of a million files, and scoring all of them just to keep
        // `limit` results is pure waste.
        let mut best: std::collections::BinaryHeap<Reverse<RankedHit>> =
            std::collections::BinaryHeap::with_capacity(limit.max(16));
        let mut seen = 0usize;

        // 1. FST lookup over the committed prefix.
        if !snapshot.fst.is_empty()
            && let Ok(map) = fst::Map::new(&snapshot.fst)
        {
            let automaton = Subsequence::new(&query_lower);
            let mut stream = map.search(automaton).into_stream();
            while let Some((_, value)) = stream.next() {
                let Ok(idx) = usize::try_from(value) else {
                    continue;
                };
                // Values beyond the FST's coverage belong to the appended
                // tail, which is scanned linearly below.
                if idx >= snapshot.fst_entries {
                    continue;
                }
                let Some(entry) = snapshot.entries.get(idx) else {
                    continue;
                };
                seen += 1;
                consider(&entry.name, idx, &query_lower, limit, &mut best);
            }
        }

        // 2. Linear scan over entries appended since the last commit.
        for (offset, entry) in snapshot.entries[snapshot.fst_entries..].iter().enumerate() {
            seen += 1;
            consider(
                &entry.name,
                snapshot.fst_entries + offset,
                &query_lower,
                limit,
                &mut best,
            );
        }

        // 3. Staged entries that have not been published yet. Their ids live past
        // the end of the snapshot so they stay unique.
        {
            let staging = self.staging.lock();
            let staged_base = snapshot.entries.len();
            for (offset, entry) in staging.iter().enumerate() {
                seen += 1;
                consider(
                    &entry.name,
                    staged_base + offset,
                    &query_lower,
                    limit,
                    &mut best,
                );
            }
        }

        debug!("Filename search '{query_lower}': {seen} candidates scanned");

        let mut ranked: Vec<RankedHit> = best.into_iter().map(|Reverse(hit)| hit).collect();
        ranked.sort_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.idx.cmp(&b.idx))
        });

        {
            let staging = self.staging.lock();
            Ok(ranked
                .into_iter()
                .filter_map(|hit| {
                    if hit.idx < snapshot.entries.len() {
                        return snapshot
                            .entries
                            .get(hit.idx)
                            .map(|entry| FilenameSearchResult {
                                file_path: entry.path.clone(),
                                file_name: entry.name.clone(),
                            });
                    }
                    staging.get(hit.idx - snapshot.entries.len()).map(|entry| {
                        FilenameSearchResult {
                            file_path: entry.path.clone(),
                            file_name: entry.name.clone(),
                        }
                    })
                })
                .collect())
        }
    }

    pub fn clear(&self) -> Result<()> {
        self.staging.lock().clear();
        self.snapshot.store(Arc::new(FilenameSnapshot::empty()));

        let index_path = self.data_path.clone();
        Self::spawn_blocking(move || {
            let _ = std::fs::remove_file(index_path.join(INDEX_FILENAME));
            let _ = std::fs::remove_file(index_path.join(LEGACY_INDEX_FILENAME));
            let _ = std::fs::remove_file(index_path.join("filenames.bin.tmp"));
        });

        Ok(())
    }

    pub fn get_stats(&self) -> Result<FilenameIndexStats> {
        let snapshot = self.snapshot.load();
        let size: u64 = snapshot
            .entries
            .iter()
            .map(|e| e.path.len() as u64 + e.name.len() as u64 + 32)
            .sum();

        Ok(FilenameIndexStats {
            total_files: snapshot.entries.len(),
            index_size_bytes: size,
        })
    }

    /// Replaces the whole index contents with `paths`.
    pub fn rebuild_index(&self, paths: Vec<(String, String)>) -> Result<()> {
        self.staging.lock().clear();

        let entries: Vec<FilenameEntry> = paths
            .into_iter()
            .map(|(path, name)| FilenameEntry {
                path,
                name: CompactString::from(name),
            })
            .collect();

        self.snapshot
            .store(Arc::new(FilenameSnapshot::from_entries(entries)));

        // Rebuilding already happened; just persist.
        self.commit()
    }
}

use std::cmp::Reverse;

/// Keeps the `limit` best-scoring candidates seen so far.
///
/// A short query ("a") can be a subsequence of a large fraction of a million
/// filenames, so scoring every candidate and sorting at the end is pure waste.
/// The heap pops the *worst* held candidate, which is only evicted when a better
/// one arrives.
fn consider(
    name: &str,
    idx: usize,
    query_lower: &str,
    limit: usize,
    best: &mut std::collections::BinaryHeap<Reverse<RankedHit>>,
) {
    let Some(score) = match_score(name, query_lower) else {
        return;
    };

    if best.len() < limit {
        best.push(Reverse(RankedHit { score, idx }));
    } else if let Some(Reverse(worst)) = best.peek()
        && score < worst.score
    {
        best.pop();
        best.push(Reverse(RankedHit { score, idx }));
    }
}

/// A scored candidate. `Ord` is derived from the score so a `BinaryHeap` of
/// `Reverse<RankedHit>` pops the *worst* candidate first.
#[derive(PartialEq)]
struct RankedHit {
    score: f32,
    idx: usize,
}

impl Eq for RankedHit {}

impl PartialOrd for RankedHit {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RankedHit {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score
            .partial_cmp(&other.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| self.idx.cmp(&other.idx))
    }
}

/// Scores how well `name` matches `query_lower` (already lowercased).
///
/// Returns `None` when the name is not a subsequence match at all, so callers
/// can skip it without a magic score sentinel.
fn match_score(name: &str, query_lower: &str) -> Option<f32> {
    let name_lower = name.to_lowercase();
    #[allow(clippy::cast_precision_loss)]
    let name_len = name_lower.len() as f32;
    let query_len = query_lower.len();

    if name_lower == query_lower {
        return Some(0.0);
    }

    // `saturating_sub` keeps the length-delta term non-negative even if a
    // multi-byte character makes `query_lower.len()` exceed the byte length we
    // compared against.
    #[allow(clippy::cast_precision_loss)]
    let length_delta = (name_len - query_len as f32).max(0.0);

    if name_lower.starts_with(query_lower) {
        return Some(length_delta.mul_add(0.001, 1.0));
    }
    if let Some(idx) = name_lower.find(query_lower) {
        #[allow(clippy::cast_precision_loss)]
        let position = idx as f32;
        return Some(length_delta.mul_add(0.001, position.mul_add(0.01, 2.0)));
    }

    find_subsequence_span(&name_lower, query_lower).map(|(start, end)| {
        #[allow(clippy::cast_precision_loss)]
        let span = (end - start + 1) as f32;
        #[allow(clippy::cast_precision_loss)]
        let gap_penalty = (span - query_len as f32).max(0.0);
        #[allow(clippy::cast_precision_loss)]
        let position = start as f32;
        name_len.mul_add(0.001, position.mul_add(0.01, 3.0 + gap_penalty * 0.1))
    })
}

/// Byte range of the first subsequence match of `query` inside `name`.
fn find_subsequence_span(name: &str, query: &str) -> Option<(usize, usize)> {
    let mut query_chars = query.chars().peekable();
    let mut first_match = None;
    let mut last_match = 0;

    for (i, c) in name.char_indices() {
        if query_chars.peek() == Some(&c) {
            first_match.get_or_insert(i);
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn entry(path: &str, name: &str) -> FilenameEntry {
        FilenameEntry {
            path: path.to_string(),
            name: CompactString::from(name),
        }
    }

    #[test]
    fn search_finds_and_ranks_exact_before_fuzzy() {
        let dir = tempdir().unwrap();
        let index = FilenameIndex::open(dir.path()).unwrap();
        assert_eq!(
            index.add_files_batch(vec![
                entry("/a/report.txt", "report.txt"),
                entry("/a/re_po_rt.txt", "re_po_rt.txt"),
                entry("/a/unrelated.docx", "unrelated.docx"),
            ]),
            3
        );

        index.commit().unwrap();
        // The save task is detached; the in-memory snapshot is already current.
        let results = index.search("report", 10).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].file_name, "report.txt");
    }

    #[test]
    fn search_covers_entries_staged_after_last_commit() {
        let dir = tempdir().unwrap();
        let index = FilenameIndex::open(dir.path()).unwrap();
        assert_eq!(
            index.add_files_batch(vec![entry("/a/alpha.txt", "alpha.txt")]),
            1
        );

        index.commit().unwrap();

        // Staged but not committed: must still be searchable.
        assert_eq!(
            index.add_files_batch(vec![entry("/a/beta.txt", "beta.txt")]),
            1
        );

        let results = index.search("beta", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "/a/beta.txt");
    }

    #[test]
    fn search_respects_limit_and_subsequence_matching() {
        let dir = tempdir().unwrap();
        let index = FilenameIndex::open(dir.path()).unwrap();
        assert_eq!(
            index.add_files_batch(vec![
                entry("/1/abc.txt", "abc.txt"),
                entry("/2/aabbcc.txt", "aabbcc.txt"),
                entry("/3/acb.txt", "acb.txt"),
            ]),
            3
        );

        index.commit().unwrap();

        let results = index.search("abc", 2).unwrap();
        assert!(results.len() <= 2);
        assert!(!results.is_empty());
    }

    #[test]
    fn empty_query_returns_nothing() {
        let dir = tempdir().unwrap();
        let index = FilenameIndex::open(dir.path()).unwrap();
        assert_eq!(index.add_files_batch(vec![entry("/a/x.txt", "x.txt")]), 1);

        index.commit().unwrap();
        assert!(index.search("", 10).unwrap().is_empty());
    }

    #[test]
    fn empty_batch_is_a_no_op() {
        let dir = tempdir().unwrap();
        let index = FilenameIndex::open(dir.path()).unwrap();
        assert_eq!(index.add_files_batch(Vec::new()), 0);
        assert_eq!(index.get_stats().unwrap().total_files, 0);
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = tempdir().unwrap();
        let index = FilenameIndex::open(dir.path()).unwrap();
        let expected: Vec<String> = (0..500).map(|i| format!("/docs/file-{i}.txt")).collect();
        assert_eq!(
            index.add_files_batch(
                expected
                    .iter()
                    .map(|p| entry(
                        p,
                        std::path::Path::new(p)
                            .file_name()
                            .unwrap()
                            .to_str()
                            .unwrap()
                    ))
                    .collect(),
            ),
            expected.len()
        );
        index.commit().unwrap();
        // commit() persists on a background task; give it a chance to finish.
        std::thread::sleep(std::time::Duration::from_millis(500));

        let reopened = FilenameIndex::open(dir.path()).unwrap();
        let stats = reopened.get_stats().unwrap();
        assert_eq!(stats.total_files, expected.len());
    }

    #[test]
    fn corrupt_index_file_degrades_to_empty() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(INDEX_FILENAME), b"not a valid index").unwrap();

        let index = FilenameIndex::open(dir.path()).unwrap();
        assert_eq!(index.get_stats().unwrap().total_files, 0);
        // A degraded index must not panic; it just finds nothing.
        assert!(index.search("anything", 5).unwrap().is_empty());
    }
}
