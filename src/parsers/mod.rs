use crate::error::{FlashError, Result};
use phf::phf_map;
use std::ffi::OsStr;
use std::path::Path;

pub mod docx;
pub mod epub;
pub mod excel;
pub mod extended;
pub mod memory_map;
pub mod odf;
pub mod pdf;
pub mod pptx;
pub mod text;

#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub path: String,
    pub content: String,
    pub title: Option<String>,
}

static DOCX_EXTENSIONS: phf::Map<&'static str, ()> = phf_map! {
    "docx" => (),
};

static PPTX_EXTENSIONS: phf::Map<&'static str, ()> = phf_map! {
    "pptx" => (),
};

static ODF_EXTENSIONS: phf::Map<&'static str, ()> = phf_map! {
    "odt" => (),
    "odp" => (),
    "ods" => (),
};

static TEXT_EXTENSIONS: phf::Map<&'static str, ()> = phf_map! {
    "txt" => (),
    "md" => (),
    "js" => (),
    "ts" => (),
    "json" => (),
    "html" => (),
    "css" => (),
    "xml" => (),
    "rs" => (),
    "toml" => (),
    "py" => (),
    "java" => (),
    "kt" => (),
    "c" => (),
    "cpp" => (),
    "h" => (),
    "hpp" => (),
    "go" => (),
    "rb" => (),
    "php" => (),
    "swift" => (),
    "dart" => (),
    "yaml" => (),
    "yml" => (),
    "ini" => (),
    "conf" => (),
    "env" => (),
    "sh" => (),
    "bat" => (),
    "ps1" => (),
    "sql" => (),
    "r" => (),
    "log" => (),
    "svelte" => (),
    "vue" => (),
    "scss" => (),
    "less" => (),
    "svg" => (),
    "ics" => (),
    "vcf" => (),
    "cmake" => (),
    "gradle" => (),
    "properties" => (),
    "proto" => (),
    "dockerfile" => (),
    "cs" => (),
    "jsx" => (),
    "tsx" => (),
    "asm" => (),
    "s" => (),
    "m" => (),
    "pl" => (),
    "lua" => (),
    "ex" => (),
    "exs" => (),
    "erl" => (),
    "clj" => (),
    "fs" => (),
    "fsx" => (),
    "vb" => (),
    "pas" => (),
    "d" => (),
    "zig" => (),
    "nim" => (),
    "hlsl" => (),
    "glsl" => (),
    "makefile" => (),
    "csv" => (),
    "tsv" => (),
    "dat" => (),
    "tex" => (),
    "latex" => (),
    "rst" => (),
    "adoc" => (),
    "asciidoc" => (),
    "gitignore" => (),
    "gitattributes" => (),
    "editorconfig" => (),
    "prettierrc" => (),
    "eslintrc" => (),
    "babelrc" => (),
    "webpack" => (),
    "nginx" => (),
    "apache" => (),
    "htaccess" => (),
    "fish" => (),
    "zsh" => (),
    "csh" => (),
    "awk" => (),
    "sed" => (),
    "vim" => (),
    "vimrc" => (),
    "gitconfig" => (),
};

static OTHER_EXTENSIONS: phf::Map<&'static str, ParserType> = phf_map! {
    "epub" => ParserType::Epub,
    "pdf" => ParserType::Pdf,
    "xlsx" => ParserType::Excel,
    "xls" => ParserType::Excel,
    "xlsb" => ParserType::Excel,
    "rtf" => ParserType::Rtf,
    "eml" => ParserType::Eml,
    "msg" => ParserType::Msg,
    "chm" => ParserType::Chm,
    "azw" => ParserType::Azw,
    "azw3" => ParserType::Azw,
    "mobi" => ParserType::Azw,
    "zip" => ParserType::Zip,
    "7z" => ParserType::SevenZ,
    "rar" => ParserType::Rar,
};

#[derive(Clone, Copy)]
enum ParserType {
    Docx,
    Pptx,
    Odf,
    Epub,
    Pdf,
    Excel,
    Rtf,
    Eml,
    Msg,
    Chm,
    Azw,
    Zip,
    SevenZ,
    Rar,
    Text,
}

impl ParserType {
    fn from_extension(ext: &str) -> Option<Self> {
        let ext_lower = ext.to_lowercase();
        if DOCX_EXTENSIONS.contains_key(&ext_lower) {
            Some(ParserType::Docx)
        } else if PPTX_EXTENSIONS.contains_key(&ext_lower) {
            Some(ParserType::Pptx)
        } else if ODF_EXTENSIONS.contains_key(&ext_lower) {
            Some(ParserType::Odf)
        } else if TEXT_EXTENSIONS.contains_key(&ext_lower) {
            Some(ParserType::Text)
        } else {
            OTHER_EXTENSIONS.get(&ext_lower).copied()
        }
    }
}

/// Parse file without allocating - uses byte comparison
#[inline]
#[allow(dead_code)]
fn extension_matches(ext: &OsStr, target: &str) -> bool {
    if let Some(ext_bytes) = ext.to_str().map(|s| s.as_bytes()) {
        if ext_bytes.len() != target.len() {
            return false;
        }
        ext_bytes
            .iter()
            .zip(target.bytes())
            .all(|(a, b)| a.eq_ignore_ascii_case(&b))
    } else {
        false
    }
}

/// Detect file type and route to appropriate parser
/// Uses phf for O(1) static lookup
pub fn parse_file(path: &Path) -> Result<ParsedDocument> {
    let extension = path.extension().unwrap_or_default();

    if let Some(ext_str) = extension.to_str() {
        let ext_lower = ext_str.to_lowercase();
        match ParserType::from_extension(&ext_lower) {
            Some(ParserType::Docx) => return docx::parse_docx(path),
            Some(ParserType::Pptx) => return pptx::parse_pptx(path),
            Some(ParserType::Odf) => return odf::parse_odf(path),
            Some(ParserType::Epub) => return epub::parse_epub(path),
            Some(ParserType::Pdf) => return pdf::parse_pdf(path),
            Some(ParserType::Excel) => return excel::parse_excel(path),
            Some(ParserType::Rtf) => return extended::parse_rtf(path),
            Some(ParserType::Eml) => return extended::parse_eml(path),
            Some(ParserType::Msg) => return extended::parse_msg(path),
            Some(ParserType::Chm) => return extended::parse_chm(path),
            Some(ParserType::Azw) => return extended::parse_azw(path),
            Some(ParserType::Zip) => return extended::parse_zip_content(path),
            Some(ParserType::SevenZ) => return extended::parse_7z_content(path),
            Some(ParserType::Rar) => return extended::parse_rar_content(path),
            Some(ParserType::Text) => return text::parse_text(path),
            None => {}
        }
    }

    let ext_str = extension.to_string_lossy().to_string();
    Err(FlashError::unsupported_format(ext_str.clone(), ext_str))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_extension_matches() {
        assert!(extension_matches(OsStr::new("docx"), "docx"));
        assert!(extension_matches(OsStr::new("DOCX"), "docx"));
        assert!(extension_matches(OsStr::new("Docx"), "docx"));
        assert!(!extension_matches(OsStr::new("pdf"), "docx"));
    }

    #[test]
    fn test_parser_type_from_extension() {
        assert!(matches!(
            ParserType::from_extension("txt"),
            Some(ParserType::Text)
        ));
        assert!(matches!(
            ParserType::from_extension("TXT"),
            Some(ParserType::Text)
        ));
        assert!(matches!(
            ParserType::from_extension("rs"),
            Some(ParserType::Text)
        ));
        assert!(matches!(
            ParserType::from_extension("js"),
            Some(ParserType::Text)
        ));
        assert!(matches!(
            ParserType::from_extension("docx"),
            Some(ParserType::Docx)
        ));
        assert!(matches!(ParserType::from_extension("exe"), None));
    }

    #[test]
    fn test_parse_file_txt() {
        let path = PathBuf::from("tests/fixtures/sample.txt");
        if path.exists() {
            let result = super::parse_file(&path);
            assert!(result.is_ok());
        }
    }
}
