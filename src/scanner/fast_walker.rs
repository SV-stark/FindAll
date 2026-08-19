use ignore::WalkBuilder;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// High-speed parallel directory walker powered by `ignore`.
pub fn scan_directory_jwalk(
    root: PathBuf,
    path_tx: &flume::Sender<PathBuf>,
    total_count: &Arc<AtomicUsize>,
    cancel_flag: &Arc<AtomicBool>,
) {
    let walker = WalkBuilder::new(root)
        .follow_links(true)
        .hidden(false)
        .ignore(false)
        .git_global(false)
        .git_ignore(false)
        .git_exclude(false)
        .build();

    for entry in walker {
        if cancel_flag.load(Ordering::Relaxed) {
            break;
        }
        if let Ok(entry) = entry
            && entry.file_type().is_some_and(|ft| ft.is_file())
        {
            total_count.fetch_add(1, Ordering::Relaxed);
            let _ = path_tx.send(entry.into_path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_scan() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "hello ignore").unwrap();

        let (tx, rx) = flume::bounded(10);
        let total = Arc::new(AtomicUsize::new(0));
        let cancel = Arc::new(AtomicBool::new(false));

        scan_directory_jwalk(temp_dir.path().to_path_buf(), &tx, &total, &cancel);
        drop(tx);

        let received: Vec<_> = rx.into_iter().collect();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0], file_path);
    }
}
