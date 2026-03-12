#[cfg(target_os = "windows")]
mod windows_usn {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::collections::HashMap;
    use windows::Win32::Foundation::{HANDLE, CloseHandle, GENERIC_READ, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
    use windows::Win32::Storage::FileSystem::{CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_ATTRIBUTE_SYSTEM};
    use windows::Win32::System::Ioctl::{
        FSCTL_ENUM_USN_DATA, FSCTL_QUERY_USN_JOURNAL, FSCTL_READ_USN_JOURNAL, MFT_ENUM_DATA_V0, 
        USN_JOURNAL_DATA_V0, USN_RECORD_V2, READ_USN_JOURNAL_DATA_V0
    };
    use windows::Win32::System::IO::DeviceIoControl;
    use crate::error::{FlashError, Result};
    use crate::scanner::{ProgressEvent, ProgressType};
    use tokio::sync::mpsc;
    use tracing::{info, warn, debug};

    #[derive(Debug)]
    struct FileInfo {
        name: String,
        parent_frn: u64,
        is_dir: bool,
    }

    pub fn scan_volume(
        root: &Path,
        path_tx: std::sync::mpsc::Sender<PathBuf>,
        progress_tx: Option<mpsc::Sender<ProgressEvent>>,
        total_count: Arc<AtomicUsize>,
    ) -> Result<()> {
        let drive_letter = root.to_string_lossy();
        let volume_path = format!("\\\\.\\{}", &drive_letter[..2]);
        let mut volume_wide: Vec<u16> = volume_path.encode_utf16().collect();
        volume_wide.push(0);

        unsafe {
            let handle = CreateFileW(
                windows::core::PCWSTR(volume_wide.as_ptr()),
                GENERIC_READ.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                None,
            ).map_err(|e| FlashError::index(format!("Failed to open volume handle: {}", e)))?;

            let result = iterate_mft(handle, &drive_letter[..3], path_tx, progress_tx, total_count);
            let _ = CloseHandle(handle);
            result
        }
    }

    unsafe fn iterate_mft(
        handle: HANDLE,
        drive_root: &str,
        path_tx: std::sync::mpsc::Sender<PathBuf>,
        progress_tx: Option<mpsc::Sender<ProgressEvent>>,
        total_count: Arc<AtomicUsize>,
    ) -> Result<()> {
        let mut journal_data = USN_JOURNAL_DATA_V0::default();
        let mut bytes_returned = 0u32;

        DeviceIoControl(
            handle,
            FSCTL_QUERY_USN_JOURNAL,
            None,
            0,
            Some(&mut journal_data as *mut _ as *mut _),
            std::mem::size_of::<USN_JOURNAL_DATA_V0>() as u32,
            Some(&mut bytes_returned),
            None,
        ).map_err(|e| FlashError::index(format!("Query USN Journal failed: {}", e)))?;

        let mut mft_enum_data = MFT_ENUM_DATA_V0 {
            StartUsn: 0,
            LowUsn: 0,
            HighUsn: journal_data.NextUsn,
        };

        let mut fs_map: HashMap<u64, FileInfo> = HashMap::with_capacity(500_000);
        let mut buffer = [0u8; 65536];

        info!("Enumerating MFT records...");

        loop {
            let mut bytes_returned = 0u32;
            let success = DeviceIoControl(
                handle,
                FSCTL_ENUM_USN_DATA,
                Some(&mft_enum_data as *const _ as *const _),
                std::mem::size_of::<MFT_ENUM_DATA_V0>() as u32,
                Some(buffer.as_mut_ptr() as *mut _),
                buffer.len() as u32,
                Some(&mut bytes_returned),
                None,
            );

            if !success.as_bool() || bytes_returned < 8 {
                break;
            }

            let next_usn = *(buffer.as_ptr() as *const i64);
            mft_enum_data.StartUsn = next_usn;

            let mut offset = 8;
            while offset < bytes_returned as usize {
                let record = &*(buffer.as_ptr().add(offset) as *const USN_RECORD_V2);
                
                // Skip system files to clean up the output and increase performance
                if (record.FileAttributes & FILE_ATTRIBUTE_SYSTEM.0) == 0 {
                    let name_ptr = buffer.as_ptr().add(offset + record.FileNameOffset as usize) as *const u16;
                    let name_len = (record.FileNameLength / 2) as usize;
                    let name = String::from_utf16_lossy(std::slice::from_raw_parts(name_ptr, name_len));

                    let frn = record.FileReferenceNumber;
                    let parent_frn = record.ParentFileReferenceNumber;
                    let is_dir = (record.FileAttributes & 0x00000010) != 0; // FILE_ATTRIBUTE_DIRECTORY

                    fs_map.insert(frn, FileInfo {
                        name,
                        parent_frn,
                        is_dir,
                    });
                }

                offset += record.RecordLength as usize;
            }
        }

        info!("MFT Enumeration finished. Reconstructing paths for {} items...", fs_map.len());

        let mut count = 0;

        for (&_frn, info) in &fs_map {
            if !info.is_dir {
                let mut path_parts = Vec::with_capacity(8);
                path_parts.push(info.name.as_str());

                let mut current_parent = info.parent_frn;
                let mut depth = 0;
                let mut valid_path = true;

                // Trace back to root. In NTFS, root's parent is itself. FRN 5 is typically root.
                while depth < 50 {
                    if let Some(parent_info) = fs_map.get(&current_parent) {
                        path_parts.push(parent_info.name.as_str());
                        if current_parent == parent_info.parent_frn {
                            break; // Reached root
                        }
                        current_parent = parent_info.parent_frn;
                        depth += 1;
                    } else {
                        // Parent not found, orphaned or root not in map
                        valid_path = false;
                        break;
                    }
                }

                if valid_path {
                    let mut full_path = PathBuf::from(drive_root);
                    for part in path_parts.iter().rev() {
                        // Skip if it's the drive root name itself being reported
                        if !part.is_empty() && !part.contains(':') {
                            full_path.push(part);
                        }
                    }

                    let _ = path_tx.send(full_path.clone());
                    count += 1;

                    if count % 2000 == 0 {
                        total_count.store(count, Ordering::Relaxed);
                        if let Some(tx) = &progress_tx {
                            let _ = tx.try_send(ProgressEvent {
                                ptype: ProgressType::Filename,
                                current_file: info.name.clone(),
                                current_folder: "".to_string(),
                                processed: count,
                                total: 0,
                                status: format!("Reconstructing MFT: {} files", count),
                                eta_seconds: 0,
                                files_per_second: 0.0,
                            });
                        }
                    }
                }
            }
        }

        total_count.store(count, Ordering::Relaxed);
        Ok(())
    }

    pub fn watch_volume(
        root: &Path, 
        tx: tokio::sync::mpsc::Sender<(PathBuf, crate::watcher::WatcherAction)>
    ) -> Result<()> {
        let drive_letter = root.to_string_lossy();
        let volume_path = format!("\\\\.\\{}", &drive_letter[..2]);
        let mut volume_wide: Vec<u16> = volume_path.encode_utf16().collect();
        volume_wide.push(0);

        let drive_root_str = drive_letter[..3].to_string();

        std::thread::spawn(move || {
            unsafe {
                let handle = CreateFileW(
                    windows::core::PCWSTR(volume_wide.as_ptr()),
                    GENERIC_READ.0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    None,
                    OPEN_EXISTING,
                    FILE_FLAG_BACKUP_SEMANTICS,
                    None,
                ).unwrap_or(HANDLE(0));

                if handle == HANDLE(0) {
                    error!("Failed to open handle for USN monitoring.");
                    return;
                }

                let mut journal_data = USN_JOURNAL_DATA_V0::default();
                let mut bytes_returned = 0u32;
                
                let success = DeviceIoControl(
                    handle,
                    FSCTL_QUERY_USN_JOURNAL,
                    None,
                    0,
                    Some(&mut journal_data as *mut _ as *mut _),
                    std::mem::size_of::<USN_JOURNAL_DATA_V0>() as u32,
                    Some(&mut bytes_returned),
                    None,
                );

                if !success.as_bool() {
                    let _ = CloseHandle(handle);
                    return;
                }

                // USN Reasons
                const USN_REASON_FILE_CREATE: u32 = 0x00000100;
                const USN_REASON_FILE_DELETE: u32 = 0x00000200;
                const USN_REASON_CLOSE: u32 = 0x80000000;

                let mut read_data = READ_USN_JOURNAL_DATA_V0 {
                    StartUsn: journal_data.NextUsn,
                    ReasonMask: 0xFFFFFFFF,
                    ReturnOnlyOnClose: 1, // Only get events when file is closed (finished writing)
                    Timeout: 0,
                    BytesToWaitFor: 0,
                    UsnJournalID: journal_data.UsnJournalID,
                    MinMajorVersion: 2,
                    MaxMajorVersion: 2,
                };

                let mut buffer = [0u8; 8192];

                loop {
                    let mut bytes_returned = 0u32;
                    let success = DeviceIoControl(
                        handle,
                        FSCTL_READ_USN_JOURNAL,
                        Some(&read_data as *const _ as *const _),
                        std::mem::size_of::<READ_USN_JOURNAL_DATA_V0>() as u32,
                        Some(buffer.as_mut_ptr() as *mut _),
                        buffer.len() as u32,
                        Some(&mut bytes_returned),
                        None,
                    );

                    if success.as_bool() && bytes_returned >= 8 {
                        let next_usn = *(buffer.as_ptr() as *const i64);
                        read_data.StartUsn = next_usn;

                        let mut offset = 8;
                        while offset < bytes_returned as usize {
                            let record = &*(buffer.as_ptr().add(offset) as *const USN_RECORD_V2);
                            
                            if (record.FileAttributes & FILE_ATTRIBUTE_SYSTEM.0) == 0 {
                                let name_ptr = buffer.as_ptr().add(offset + record.FileNameOffset as usize) as *const u16;
                                let name_len = (record.FileNameLength / 2) as usize;
                                let name = String::from_utf16_lossy(std::slice::from_raw_parts(name_ptr, name_len));
                                
                                // Simplified path: In a full impl, we'd use the FRN map.
                                // For now, we only support top-level changes or 
                                // we'd need to keep the fs_map in memory.
                                // As a compromise for this prototype, we'll try to get the path
                                // by using the parent FRN if we have a way to cache it.
                                let mut changed_path = PathBuf::from(&drive_root_str);
                                changed_path.push(name);
                                
                                let action = if (record.Reason & USN_REASON_FILE_DELETE) != 0 {
                                    crate::watcher::WatcherAction::Remove
                                } else {
                                    crate::watcher::WatcherAction::Index
                                };
                                
                                let _ = tx.blocking_send((changed_path, action));
                            }

                            offset += record.RecordLength as usize;
                        }
                    } else {
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    }
                }
            }
        });
        
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod macos_fsevents {
    use std::path::{Path, PathBuf};
    use crate::error::Result;
    use tracing::info;

    pub fn scan_volume(root: &Path) -> Result<()> {
        info!("macOS Spotlight / APFS parallel scan stub for {:?}", root);
        Ok(())
    }
    
    pub fn watch_volume(root: &Path) -> Result<()> {
        info!("macOS FSEvents real-time monitoring stub for {:?}", root);
        Ok(())
    }
}

#[cfg(target_os = "linux")]
mod linux_fanotify {
    use std::path::{Path, PathBuf};
    use crate::error::Result;
    use tracing::info;

    pub fn scan_volume(root: &Path) -> Result<()> {
        info!("Linux io_uring / parallel scan stub for {:?}", root);
        Ok(())
    }
    
    pub fn watch_volume(root: &Path) -> Result<()> {
        info!("Linux fanotify real-time monitoring stub for {:?}", root);
        Ok(())
    }
}

use crate::error::Result;
use crate::scanner::{ProgressEvent, ProgressType};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn};
use ignore::WalkBuilder;

pub trait DriveScanner: Send + Sync {
    fn scan(
        &self,
        root: PathBuf,
        exclude_patterns: Vec<String>,
        path_tx: std::sync::mpsc::Sender<PathBuf>,
        progress_tx: Option<mpsc::Sender<ProgressEvent>>,
        total_count: Arc<AtomicUsize>,
    ) -> Result<()>;

    // Real-time hook for whole drives
    fn watch(
        &self, 
        root: PathBuf, 
        tx: tokio::sync::mpsc::Sender<(PathBuf, crate::watcher::WatcherAction)>
    ) -> Result<()> {
        // Default no-op
        Ok(())
    }
}

pub struct DefaultDriveScanner;

impl DriveScanner for DefaultDriveScanner {
    fn scan(
        &self,
        root: PathBuf,
        exclude_patterns: Vec<String>,
        path_tx: std::sync::mpsc::Sender<PathBuf>,
        progress_tx: Option<mpsc::Sender<ProgressEvent>>,
        total_count: Arc<AtomicUsize>,
    ) -> Result<()> {
        let mut builder = WalkBuilder::new(&root);

        let mut override_builder = ignore::overrides::OverrideBuilder::new(&root);
        for pattern in &exclude_patterns {
            let ignore_pattern = format!("!{}", pattern);
            if let Err(e) = override_builder.add(&ignore_pattern) {
                warn!("Invalid exclude pattern '{}': {}", pattern, e);
            }
        }
        if let Ok(overrides) = override_builder.build() {
            builder.overrides(overrides);
        }

        builder.follow_links(true).standard_filters(false);
        builder.max_depth(Some(20));

        info!("Starting DefaultDriveScanner for {}", root.display());
        let walker = builder.build_parallel();
        
        walker.run(|| {
            let path_tx = path_tx.clone();
            let progress_tx = progress_tx.clone();
            let total = total_count.clone();
            Box::new(move |entry| {
                if let Ok(entry) = entry {
                    if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                        let path = entry.path().to_path_buf();
                        let _ = path_tx.send(path);
                        let count = total.fetch_add(1, Ordering::Relaxed);

                        if count % 100 == 0 {
                            if let Some(tx) = &progress_tx {
                                let _ = tx.try_send(ProgressEvent {
                                    ptype: ProgressType::Filename,
                                    current_file: entry.file_name().to_string_lossy().to_string(),
                                    current_folder: "".to_string(),
                                    processed: count,
                                    total: 0,
                                    status: "Scanning filenames...".to_string(),
                                    eta_seconds: 0,
                                    files_per_second: 0.0,
                                });
                            }
                        }
                    }
                }
                ignore::WalkState::Continue
            })
        });

        let final_count = total_count.load(Ordering::Relaxed);
        if let Some(tx) = &progress_tx {
            let _ = tx.try_send(ProgressEvent {
                ptype: ProgressType::Filename,
                current_file: "".to_string(),
                current_folder: "".to_string(),
                processed: final_count,
                total: final_count,
                status: "Filename scan complete".to_string(),
                eta_seconds: 0,
                files_per_second: 0.0,
            });
        }

        Ok(())
    }
}

#[cfg(target_os = "windows")]
pub struct WindowsDriveScanner;

#[cfg(target_os = "windows")]
impl DriveScanner for WindowsDriveScanner {
    fn scan(
        &self,
        root: PathBuf,
        exclude_patterns: Vec<String>,
        path_tx: std::sync::mpsc::Sender<PathBuf>,
        progress_tx: Option<mpsc::Sender<ProgressEvent>>,
        total_count: Arc<AtomicUsize>,
    ) -> Result<()> {
        let is_root = root.parent().is_none() || root.to_string_lossy().len() <= 3;
        
        if is_root && root.exists() {
            info!("Whole drive detected, attempting MFT scan for {:?}", root);
            if let Err(e) = windows_usn::scan_volume(&root, path_tx.clone(), progress_tx.clone(), total_count.clone()) {
                warn!("MFT scan failed, falling back to parallel walk: {}", e);
            } else {
                return Ok(());
            }
        }

        let fallback = DefaultDriveScanner;
        fallback.scan(root, exclude_patterns, path_tx, progress_tx, total_count)
    }

    fn watch(
        &self, 
        root: PathBuf, 
        tx: tokio::sync::mpsc::Sender<(PathBuf, crate::watcher::WatcherAction)>
    ) -> Result<()> {
        windows_usn::watch_volume(&root, tx)
    }
}

#[cfg(target_os = "macos")]
pub struct MacDriveScanner;

#[cfg(target_os = "macos")]
impl DriveScanner for MacDriveScanner {
    fn scan(
        &self,
        root: PathBuf,
        exclude_patterns: Vec<String>,
        path_tx: std::sync::mpsc::Sender<PathBuf>,
        progress_tx: Option<mpsc::Sender<ProgressEvent>>,
        total_count: Arc<AtomicUsize>,
    ) -> Result<()> {
        let _ = macos_fsevents::scan_volume(&root);
        let fallback = DefaultDriveScanner;
        fallback.scan(root, exclude_patterns, path_tx, progress_tx, total_count)
    }
}

#[cfg(target_os = "linux")]
pub struct LinuxDriveScanner;

#[cfg(target_os = "linux")]
impl DriveScanner for LinuxDriveScanner {
    fn scan(
        &self,
        root: PathBuf,
        exclude_patterns: Vec<String>,
        path_tx: std::sync::mpsc::Sender<PathBuf>,
        progress_tx: Option<mpsc::Sender<ProgressEvent>>,
        total_count: Arc<AtomicUsize>,
    ) -> Result<()> {
        let _ = linux_fanotify::scan_volume(&root);
        let fallback = DefaultDriveScanner;
        fallback.scan(root, exclude_patterns, path_tx, progress_tx, total_count)
    }
}

