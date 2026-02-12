use crate::error::{FlashError, Result};
use crate::parsers::ParsedDocument;
use calamine::{open_workbook, Reader, Xlsx, Xls, Xlsb};
use std::path::Path;

/// Maximum number of cells to process per sheet (prevent DOS from huge spreadsheets)
const MAX_CELLS_PER_SHEET: usize = 1_000_000;
/// Maximum total text length to extract (prevent memory bloat)
const MAX_TOTAL_TEXT_LENGTH: usize = 50_000_000; // 50MB

/// Parse Excel files (XLSX, XLS, XLSB) using calamine
/// Extracts text content from all sheets for indexing
pub fn parse_excel(path: &Path) -> Result<ParsedDocument> {
    // Try XLSX first (most common)
    if let Ok(result) = parse_xlsx(path) {
        return Ok(result);
    }
    
    // Try XLSB (Excel Binary)
    if let Ok(result) = parse_xlsb(path) {
        return Ok(result);
    }
    
    // Try legacy XLS
    if let Ok(result) = parse_xls(path) {
        return Ok(result);
    }
    
    Err(FlashError::Parse(format!(
        "Failed to parse Excel file: {}",
        path.display()
    )))
}

/// Parse XLSX format
fn parse_xlsx(path: &Path) -> Result<ParsedDocument> {
    let mut workbook: Xlsx<_> = open_workbook(path)
        .map_err(|e| FlashError::Parse(format!("Failed to open XLSX: {}", e)))?;
    
    extract_excel_content(path, &mut workbook)
}

/// Parse XLSB format
fn parse_xlsb(path: &Path) -> Result<ParsedDocument> {
    let mut workbook: Xlsb<_> = open_workbook(path)
        .map_err(|e| FlashError::Parse(format!("Failed to open XLSB: {}", e)))?;
    
    extract_excel_content(path, &mut workbook)
}

/// Parse legacy XLS format
fn parse_xls(path: &Path) -> Result<ParsedDocument> {
    let mut workbook: Xls<_> = open_workbook(path)
        .map_err(|e| FlashError::Parse(format!("Failed to open XLS: {}", e)))?;
    
    extract_excel_content(path, &mut workbook)
}

/// Extract content from any calamine workbook type
fn extract_excel_content<P, RS>(
    path: &Path,
    workbook: &mut P,
) -> Result<ParsedDocument> 
where
    RS: std::io::Read + std::io::Seek,
    P: Reader<RS>,
{
    let mut combined_text = String::with_capacity(1024 * 1024); // Start with 1MB capacity
    let mut total_cells = 0usize;
    let sheet_names = workbook.sheet_names().to_vec();
    
    for sheet_name in &sheet_names {
        // Add sheet name as context
        combined_text.push_str("Sheet: ");
        combined_text.push_str(sheet_name);
        combined_text.push('\n');
        
        if let Ok(range) = workbook.worksheet_range(sheet_name) {
            for row in range.rows() {
                for cell in row {
                    total_cells += 1;
                    
                    // Check cell limit
                    if total_cells > MAX_CELLS_PER_SHEET {
                        eprintln!(
                            "Warning: Excel file {} exceeded max cells per sheet limit",
                            path.display()
                        );
                        break;
                    }
                    
                    // Extract cell value as string
                    let cell_text = format_cell_value(cell);
                    if !cell_text.is_empty() {
                        combined_text.push_str(&cell_text);
                        combined_text.push(' ');
                    }
                    
                    // Check total text size limit
                    if combined_text.len() > MAX_TOTAL_TEXT_LENGTH {
                        eprintln!(
                            "Warning: Excel file {} exceeded max text length limit",
                            path.display()
                        );
                        break;
                    }
                }
                combined_text.push('\n');
            }
        }
        combined_text.push('\n');
    }
    
    // Fallback to filename for title since properties aren't easily available in Metadata
    let title = path.file_stem().map(|s| s.to_string_lossy().to_string());
    
    Ok(ParsedDocument {
        path: path.to_string_lossy().to_string(),
        content: combined_text.trim().to_string(),
        title,
    })
}

/// Format a calamine cell value as string
fn format_cell_value(cell: &calamine::Data) -> String {
    match cell {
        calamine::Data::Empty => String::new(),
        calamine::Data::String(s) => s.clone(),
        calamine::Data::Float(f) => f.to_string(),
        calamine::Data::Int(i) => i.to_string(),
        calamine::Data::Bool(b) => b.to_string(),
        calamine::Data::DateTime(dt) => dt.to_string(),
        calamine::Data::Error(e) => format!("#ERROR: {:?}", e),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_format_cell_value() {
        assert_eq!(format_cell_value(&calamine::Data::Empty), "");
        assert_eq!(format_cell_value(&calamine::Data::String("test".to_string())), "test");
        assert_eq!(format_cell_value(&calamine::Data::Float(3.14)), "3.14");
        assert_eq!(format_cell_value(&calamine::Data::Int(42)), "42");
        assert_eq!(format_cell_value(&calamine::Data::Bool(true)), "true");
    }
}
