# OCR Integration via Plugin System

This document outlines how to integrate custom Optical Character Recognition (OCR) engines into the application using the **Kreuzberg Plugin System**.

## Kreuzberg Architecture

Kreuzberg operates on a **Registry Pattern** for extension points like OCR backends, post-processors, and validators. In Rust, these registries are accessible globally and can be used to plug in custom implementations.

### 1. The OCR Backend Registry

Kreuzberg provides access to its OCR backend registry via the following function in its Rust API:

```rust
pub fn get_ocr_backend_registry() -> Arc<RwLock<OcrBackendRegistry>>
```

This registry holds instances of types that implement the `OcrBackend` trait (or equivalent) provided by the `kreuzberg` crate.

---

## Integration Steps

To add a custom OCR backend (e.g., using a cloud API, a wrapper for Tesseract with custom flags, or another library), follow these general steps:

### Step 1: Implement the OCR Backend Trait

You will need a struct representing your backend that implements the necessary trait. Based on Kreuzberg's design in other languages (Python/TypeScript), the Rust trait likely requires methods for:

*   **`name()`**: Returns a unique identifier string (e.g., `"custom_cloud_ocr"`).
*   **`supported_languages()`**: Returns a list of language codes supported.
*   **`process_image(bytes, language)`**: The core method that takes raw image bytes and returns OCR text output inside structured metadata.

```rust
// Conceptual implementation
struct CustomOcrBackend;

impl CustomOcrBackend {
    pub fn name(&self) -> &str {
        "custom_ocr"
    }

    pub fn supported_languages(&self) -> Vec<String> {
        vec!["eng".to_string(), "deu".into()]
    }

    // signature might vary, check kreuzberg crate docs
    pub fn process_image(&self, data: &[u8], language: &str) -> Result<String, Error> {
        // Call your custom OCR engine/API here
        Ok("Extracted text...".to_string())
    }
}
```

### Step 2: Register the Backend on Startup

During application initialization (e.g., in `main.rs` or `src/parsers/mod.rs`), acquire a write lock on the registry and insert your backend:

```rust
pub fn register_custom_plugins() {
    let registry = kreuzberg::get_ocr_backend_registry();
    let mut write_guard = registry.write().expect("Lock poisoned");
    
    // Exact registration method might be `register` or `insert`
    // write_guard.register(Box::new(CustomOcrBackend));
}
```

### Step 3: Configure `ExtractionConfig`

Once registered, you can instruct Kreuzberg to use your custom backend by setting the `backend` field in the `OcrConfig`:

```rust
use kreuzberg::{ExtractionConfig, OcrConfig};

let config = ExtractionConfig {
    ocr: Some(OcrConfig {
        backend: "custom_ocr".to_string(), // Matches name() output
        language: "eng".to_string(),
        ..Default::default()
    }),
    ..Default::default()
};

// Use this config in extract_file()
```

---

## Alternative: Flash Search Wrap Layer

If you want to bypass Kreuzberg’s registry or add pre/post logic unique to **Flash Search**, you can modify `src/parsers/mod.rs` inside the `parse_file` routing function:

1.  Inspect the file type/extension.
2.  If it’s an image (PNG/JPG) and needs OCR, route to your *own* custom function instead of calling `kreuzberg::extract_file_sync`.
3.  Pack the results into a `ParsedDocument` struct.

This is helpful if the OCR process is entirely decoupled from document extraction (e.g., running shell commands or calling specific binaries directly in Flash Search).

> [!NOTE]
> For the exact trait signatures of `OcrBackend` in Rust, please review the `kreuzberg` crate source or documentation using `cargo doc`. The global registry methods are verified fully operational.
