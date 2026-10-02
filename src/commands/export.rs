use crate::error::{FlashError, Result};
use crate::indexer::searcher::SearchResult;

pub fn export_results_csv(results: &[SearchResult], path: &str) -> Result<()> {
    let mut wtr =
        csv::Writer::from_path(path).map_err(|e| FlashError::Io(std::sync::Arc::new(e.into())))?;

    // Write header
    wtr.write_record(["Score", "File Path", "Title"])
        .map_err(|e| FlashError::Io(std::sync::Arc::new(e.into())))?;

    for r in results {
        let title = r.title.as_deref().unwrap_or("");
        wtr.write_record(&[r.score.to_string(), r.file_path.clone(), title.to_string()])
            .map_err(|e| FlashError::Io(std::sync::Arc::new(e.into())))?;
    }

    wtr.flush()
        .map_err(|e| FlashError::Io(std::sync::Arc::new(e)))?;
    Ok(())
}

pub fn export_results_json(results: &[SearchResult], path: &str) -> Result<()> {
    let json = serde_json::to_string_pretty(results)
        .map_err(|e| FlashError::parse(std::path::Path::new(path), e.to_string()))?;
    std::fs::write(path, json).map_err(|e| FlashError::Io(std::sync::Arc::new(e)))?;
    Ok(())
}
