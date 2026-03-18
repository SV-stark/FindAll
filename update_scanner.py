import re

with open("src/scanner/drive_scanner.rs", "r", encoding="utf-8") as f:
    content = f.read()

# Replace FileInfo struct with DirInfo
content = content.replace(
"""    #[derive(Debug)]
    struct FileInfo {
        name: String,
        parent_frn: u64,
        is_dir: bool,
    }""", 
"""    #[derive(Debug)]
    struct DirInfo {
        name: String,
        parent_frn: u64,
    }""")

# Find iterate_mft and replace its body up to the loop end
old_init = """        let mut fs_map: HashMap<u64, FileInfo> = HashMap::with_capacity(500_000);"""
new_init = """        let mut dir_map: HashMap<u64, DirInfo> = HashMap::with_capacity(100_000);
        let mut files: Vec<(String, u64)> = Vec::with_capacity(500_000);"""

content = content.replace(old_init, new_init)

old_insert = """                    let frn = record.FileReferenceNumber;
                    let parent_frn = record.ParentFileReferenceNumber;
                    let is_dir = (record.FileAttributes & 0x00000010) != 0; // FILE_ATTRIBUTE_DIRECTORY

                    fs_map.insert(
                        frn,
                        FileInfo {
                            name,
                            parent_frn,
                            is_dir,
                        },
                    );"""

new_insert = """                    let frn = record.FileReferenceNumber;
                    let parent_frn = record.ParentFileReferenceNumber;
                    let is_dir = (record.FileAttributes & 0x00000010) != 0; // FILE_ATTRIBUTE_DIRECTORY

                    if is_dir {
                        dir_map.insert(
                            frn,
                            DirInfo {
                                name,
                                parent_frn,
                            },
                        );
                    } else {
                        files.push((name, parent_frn));
                    }"""

content = content.replace(old_insert, new_insert)

old_reconstruct = """        info!(
            "MFT Enumeration finished. Reconstructing paths for {} items...",
            fs_map.len()
        );

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
                }"""

new_reconstruct = """        info!(
            "MFT Enumeration finished. Reconstructing paths for {} files...",
            files.len()
        );

        let mut count = 0;

        for (name, mut current_parent) in files {
            let mut path_parts = Vec::with_capacity(8);
            path_parts.push(name.as_str());

            let mut depth = 0;
            let mut valid_path = true;

            // Trace back to root. In NTFS, root's parent is itself. FRN 5 is typically root.
            while depth < 50 {
                if let Some(parent_info) = dir_map.get(&current_parent) {
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
            }"""

content = content.replace(old_reconstruct, new_reconstruct)

# Fix progress_tx usage
content = content.replace("""                                current_file: info.name.clone(),""", """                                current_file: name.clone(),""")

# Save the updated content
with open("src/scanner/drive_scanner.rs", "w", encoding="utf-8") as f:
    f.write(content)

print("Updated drive_scanner.rs successfully")
