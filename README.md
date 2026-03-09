<h1 align="center">Flash Search</h1>

<p align="center">
  <img src="assets/logo.png" alt="Flash Search Logo" width="120">
</p>

<p align="center">
  <b>Ultrafast local full-text search with minimal resource footprint</b>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#installation">Installation</a> •
  <a href="#usage">Usage</a> •
  <a href="#tech-stack">Tech Stack</a> •
  <a href="#performance">Performance</a> •
  <a href="#roadmap">Roadmap</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/Iced-0.14-blue?style=flat&logo=rust&logoColor=white" alt="Iced">
  <img src="https://img.shields.io/badge/License-GPL--3.0-blue.svg" alt="License">
</p>

---

<h2 align="center">🚀 Features</h2>

<div align="center">

- **⚡ Blazing Fast**: Sub-50ms search results across millions of documents
- **📂 Filename Search**: Instant filename-only search mode for ultra-fast navigation
- **💾 Minimal Footprint**: <30MB RAM usage at idle (vs 200MB+ for Electron apps)
- **📄 Universal Format Support**: Native support for **75+ formats** (PDF, Office, Images, Archives, Ebooks, etc.)
- **🔍 Full-Text Search**: BM25 scoring, boolean queries, exact phrase matching
- **📸 Integrated OCR**: Search text within images and scanned PDFs via Kreuzberg
- **📊 Advanced Filters**: Filter by size (`size:>1MB`), extension (`ext:rs`), or path (`path:src`)
- **🔄 Live Indexing**: Automatic file watching and incremental updates
- **🎯 Smart Filtering**: .gitignore support, custom exclude patterns
- **🌙 Native UI**: Beautiful dark/light themes using Iced's high-performance renderer.

</div>

<h2 align="center">📥 Installation</h2>

<h3 align="center">Prerequisites</h3>

<div align="center">

- **Windows**: Windows 10/11
- **macOS**: macOS 10.15+
- **Linux**: Vulkan-compatible drivers & development tools (pkg-config, libfontconfig1-dev)

</div>

<h3 align="center">Download</h3>

<div align="center">

Download the latest release for your platform from the [Releases](https://github.com/SV-stark/FindAll/releases) page.

</div>

<h3 align="center">Build from Source</h3>

```bash
# Clone the repository
git clone https://github.com/SV-stark/FindAll.git
cd FindAll

# Build the application
cargo build --release

# Run in development mode
cargo run
```

<h2 align="center">🎮 Usage</h2>

<h3 align="center">First Launch</h3>

<div align="center">

1. **Initial Setup**: Select folders to index on first launch
2. **Indexing**: The app will scan and index your files (this may take a few minutes for large directories)
3. **Search**: Press `Alt+Space` (or your custom hotkey) to open the search bar from anywhere

</div>

<h3 align="center">Search Syntax</h3>

<div align="center">

| Query | Description |
|-------|-------------|
| `rust tutorial` | Find documents containing both words |
| `"exact phrase"` | Find exact phrase matches |
| `rust OR python` | Boolean OR operator |
| `code -python` | Exclude documents with "python" |
| `title:api` | Search only in document titles |
| `ext:pdf` | Filter by file extension |
| `path:docs` | Filter by folder path |
| `size:>5MB` | Filter by file size (KB, MB, GB) |

</div>

<h3 align="center">Keyboard Shortcuts</h3>

<div align="center">

| Shortcut | Action |
|----------|--------|
| `Alt+Space` | Toggle search window (Global Hotkey, configurable in Settings) |
| `Ctrl+Enter` | Open selected file |
| `Ctrl+C` | Copy file path |
| `Esc` | Close search window |
| `↑/↓` | Navigate results |

</div>

<h2 align="center">🏗️ Tech Stack</h2>

<h3 align="center">Core Architecture</h3>

<div align="center">

| Component | Technology | Purpose |
|-----------|------------|---------|
| **Language** | Rust | Zero-overhead, memory-safe core |
| **GUI** | Iced | High-performance, cross-platform UI framework (<30MB RAM) |
| **Search Engine** | Tantivy | Full-text indexing with BM25 scoring |
| **Document Intelligence** | [Kreuzberg](https://github.com/kreuzberg-dev/kreuzberg) | Universal text extraction (75+ formats) + OCR |
| **Metadata DB** | redb | Pure Rust key-value storage |
| **Concurrency** | Rayon + Tokio | Parallel processing + async I/O |

</div>

<h3 align="center">Supported File Formats</h3>

<div align="center">

| Format Category | Examples | Status |
|-----------------|----------|--------|
| **Documents** | PDF, DOCX, XLSX, PPTX, ODT, RTF | ✅ Supported |
| **Images (OCR)** | JPEG, PNG, TIFF, HEIC, Scanned PDF | ✅ Supported |
| **Archives** | ZIP, 7z, RAR, TAR | ✅ Supported |
| **Email** | EML, MSG, Outlook PST | ✅ Supported |
| **Ebooks** | EPUB, MOBI, AZW3 | ✅ Supported |
| **Code & Text** | 100+ programing languages, MD, JSON, XML | ✅ Supported |
| **Structured Data** | CSV, TSV, Parquet | ✅ Supported |

</div>

<h2 align="center">📊 Performance</h2>

<div align="center">

Benchmarks on AMD Ryzen 7 5800X with NVMe SSD:

</div>

<div align="center">

| Metric | Value |
|--------|-------|
| **Index 10,000 PDFs** | ~45 seconds |
| **Index 100,000 TXT files** | ~12 seconds |
| **Search latency (p50)** | 12ms |
| **Search latency (p99)** | 45ms |
| **Idle RAM usage** | ~25MB |
| **Peak RAM (indexing)** | ~180MB |

</div>

<h3 align="center">Comparison</h3>

<div align="center">

| Feature | Flash Search | AnyTXT | Windows Search | Recoll |
|---------|-------------|---------|----------------|---------|
| Startup Time | Instant | ~2s | System | ~3s |
| RAM Usage | 25MB | 80MB | 150MB+ | 120MB |
| PDF Search | ✅ (Bundled) | ✅ | ⚠️ | ✅ |
| OCR Support | ✅ (Built-in) | ✅ | ❌ | ⚠️ |
| 75+ Formats | ✅ | ✅ | ❌ | ⚠️ |
| Live Updates | ✅ | ✅ | ✅ | ❌ |
| Cross-Platform | ✅ | ⚠️ | ❌ | ✅ |

</div>

<h2 align="center">🗺️ Roadmap</h2>

<h3 align="center">Phase 1: Core (Completed ✅)</h3>
<div align="center">

- [x] Project setup with Iced
- [x] Basic file parsers (PDF, DOCX, TXT)
- [x] Tantivy integration
- [x] Parallel file scanning

</div>

<h3 align="center">Phase 2: Polish (Completed ✅)</h3>
<div align="center">

- [x] Advanced search filters (date, size, type)
- [x] Search result preview panel
- [x] Export search results (CSV, JSON)
- [x] Fast filename-only search & indexing
- [x] Enhanced indexing telemetry

</div>

<h3 align="center">Phase 3: Advanced Features (In Progress 🚧)</h3>
<div align="center">

- [x] Universal Format Support (75+ formats via Kreuzberg)
- [x] Integrated OCR for images and scanned PDFs
- [x] Search history and favorites (Pinned files)
- [ ] Natural language queries
- [ ] Cloud sync for index

</div>

<h3 align="center">Phase 4: Enterprise (Future 🔮)</h3>
<div align="center">

- [ ] Network drive support
- [ ] Multi-user indexing
- [ ] Web interface
- [ ] API for integrations

</div>

<h2 align="center">🤝 Contributing</h2>

<div align="center">

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

</div>

<h3 align="center">Development Setup</h3>

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and setup
git clone https://github.com/SV-stark/FindAll.git
cd FindAll

# Run development server
cargo run
```

<h2 align="center">📄 License</h2>

<div align="center">

This project is licensed under the GNU General Public License v3 - see the [LICENSE](LICENSE) file for details.

</div>

<h2 align="center">🙏 Acknowledgments</h2>

<div align="center">

- [Tantivy](https://github.com/quickwit-oss/tantivy) - The blazing-fast search engine library
- [Iced](https://iced.rs/) - The cross-platform UI framework for Rust
- [Redb](https://github.com/cberner/redb) - The pure Rust embedded database

</div>

---

<p align="center">
  Made with ❤️ using Rust
</p>
