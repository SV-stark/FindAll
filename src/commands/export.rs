use crate::indexer::searcher::SearchResult;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

pub fn export_results_csv(results: &[SearchResult], path: &str) -> Result<(), String> {
    let mut file = File::create(path).map_err(|e| e.to_string())?;
    writeln!(file, "Score,File Path,Title").map_err(|e| e.to_string())?;
    for r in results {
        let title = r.title.as_deref().unwrap_or("");
        writeln!(file, "{},{},{}", r.score, r.file_path, title).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn export_results_json(results: &[SearchResult], path: &str) -> Result<(), String> {
    let json = serde_json::to_string_pretty(results).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(())
}
