# Flash Search

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

## 🚀 Features

- **⚡ Blazing Fast**: Sub-50ms search results across millions of documents
- **📂 Filename Search**: Instant filename-only search mode for ultra-fast navigation
- **💾 Minimal Footprint**: <30MB RAM usage at idle (vs 200MB+ for Electron apps)
- **📄 Universal Format Support**: PDF, DOCX, XLSX, EPUB, EML, MSG, ZIP, Markdown, Code files, ODT, ODP, ODS
- **🔍 Full-Text Search**: BM25 scoring, boolean queries, exact phrase matching
- **📊 Advanced Filters**: Filter by size (`size:>1MB`), extension (`ext:rs`), or path (`path:src`)
- **🔄 Live Indexing**: Automatic file watching and incremental updates
- **🎯 Smart Filtering**: .gitignore support, custom exclude patterns
- **🌙 Native UI**: Beautiful dark/light themes using Iced's high-performance renderer.

## 📥 Installation

### Prerequisites

- **Windows**: Windows 10/11
- **macOS**: macOS 10.15+
- **Linux**: Vulkan-compatible drivers & development tools (pkg-config, libfontconfig1-dev)

### Download

Download the latest release for your platform from the [Releases](https://github.com/SV-stark/FindAll/releases) page.

### Build from Source

```bash
# Clone the repository
git clone https://github.com/SV-stark/FindAll.git
cd FindAll

# Build the application
cargo build --release

# Run in development mode
cargo run
```

## 🎮 Usage

### First Launch

1. **Initial Setup**: Select folders to index on first launch
2. **Indexing**: The app will scan and index your files (this may take a few minutes for large directories)
3. **Search**: Press `Alt+Space` (or your custom hotkey) to open the search bar from anywhere

### Search Syntax

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

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Alt+Space` | Toggle search window (Global Hotkey, configurable in Settings) |
| `Ctrl+Enter` | Open selected file |
| `Ctrl+C` | Copy file path |
| `Esc` | Close search window |
| `↑/↓` | Navigate results |

## 🏗️ Tech Stack

### Core Architecture

| Component | Technology | Purpose |
|-----------|------------|---------|
| **Language** | Rust | Zero-overhead, memory-safe core |
| **GUI** | Iced | High-performance, cross-platform UI framework (<30MB RAM) |
| **Search Engine** | Tantivy | Full-text indexing with BM25 scoring |
| **Metadata DB** | redb | Pure Rust key-value storage |
| **Concurrency** | Rayon + Tokio | Parallel processing + async I/O |

### Supported File Formats

| Format | Parser | Status |
|--------|--------|--------|
| PDF | `pdf-extract` / `lopdf` | ✅ Supported |
| DOCX | `zip` + `quick-xml` | ✅ Supported |
| XLSX, XLS, XLSB | `calamine` | ✅ Supported |
| EPUB, EML, MSG | Native + `zip` | ✅ Supported |
| ZIP, 7z, RAR | `zip` / `sevenz` | ✅ Supported (ZIP) |
| TXT, MD, Code | Native Rust | ✅ Supported |
| ODT, ODP, ODS | `litchi` | ✅ Supported |
| Images (OCR) | `ocrs` / Tesseract | 🚧 Planned |

## 📊 Performance

Benchmarks on AMD Ryzen 7 5800X with NVMe SSD:

| Metric | Value |
|--------|-------|
| **Index 10,000 PDFs** | ~45 seconds |
| **Index 100,000 TXT files** | ~12 seconds |
| **Search latency (p50)** | 12ms |
| **Search latency (p99)** | 45ms |
| **Idle RAM usage** | ~25MB |
| **Peak RAM (indexing)** | ~180MB |

### Comparison

| Feature | Flash Search | AnyTXT | Windows Search | Recoll |
|---------|-------------|---------|----------------|---------|
| Startup Time | Instant | ~2s | System | ~3s |
| RAM Usage | 25MB | 80MB | 150MB+ | 120MB |
| PDF Search | ✅ | ✅ | ⚠️ | ✅ |
| Live Updates | ✅ | ✅ | ✅ | ❌ |
| Cross-Platform | ✅ | ⚠️ | ❌ | ✅ |

## 🗺️ Roadmap

### Phase 1: Core (Completed ✅)
- [x] Project setup with Iced
- [x] Basic file parsers (PDF, DOCX, TXT)
- [x] Tantivy integration
- [x] Parallel file scanning

### Phase 2: Polish (Completed ✅)
- [x] Advanced search filters (date, size, type)
- [x] Search result preview panel
- [x] Export search results (CSV, JSON)
- [x] Fast filename-only search & indexing
- [x] Enhanced indexing telemetry

### Phase 3: Advanced Features (In Progress 🚧)
- [x] Search history and favorites (Pinned files)
- [ ] Natural language queries
- [ ] Plugin system for custom parsers
- [ ] OCR support for images and scanned PDFs
- [ ] Cloud sync for index

### Phase 4: Enterprise (Future 🔮)
- [ ] Network drive support
- [ ] Multi-user indexing
- [ ] Web interface
- [ ] API for integrations

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Setup

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and setup
git clone https://github.com/SV-stark/FindAll.git
cd FindAll

# Run development server
cargo run
```

## 📄 License

This project is licensed under the GNU General Public License v3 - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [Tantivy](https://github.com/quickwit-oss/tantivy) - The blazing-fast search engine library
- [Iced](https://iced.rs/) - The cross-platform UI framework for Rust
- [Redb](https://github.com/cberner/redb) - The pure Rust embedded database

---

<p align="center">
  Made with ❤️ using Rust
</p>
