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
- **🔄 Auto-Update**: Seamlessly checks for and installs new updates in the background.
- **🚀 Auto-Launch**: Configurable to launch quietly on system startup so search is always ready.


<h2 align="center">📥 Installation</h2>

<h3 align="center">Prerequisites</h3>

- **Windows**: Windows 10/11
- **macOS**: macOS 10.15+
- **Linux**: Vulkan-compatible drivers & development tools (pkg-config, libfontconfig1-dev)

<h3 align="center">Install via Terminal</h3>

**Windows (PowerShell):**
```powershell
irm https://github.com/SV-stark/FindAll/releases/latest/download/flash-search-installer.ps1 | iex
```

**macOS & Linux:**
```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/SV-stark/FindAll/releases/latest/download/flash-search-installer.sh | sh
```

Or manually download the pre-built binaries for your platform from the [Releases](https://github.com/SV-stark/FindAll/releases) page.

*Note: The installed application binary is called `flash-search`.*

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

1.  **Initial Setup**: Select folders to index on first launch
2.  **Indexing**: The app will scan and index your files (this may take a few minutes for large directories)
3.  **Search**: Press `Alt+Space` (or your custom hotkey) to open the search bar from anywhere

<h3 align="center">Search Syntax</h3>

| Query | Description |
|:---|:---|
| `rust tutorial` | Find documents containing both words |
| `"exact phrase"` | Find exact phrase matches |
| `rust OR python` | Boolean OR operator |
| `code -python` | Exclude documents with "python" |
| `title:api` | Search only in document titles |
| `ext:pdf` | Filter by file extension |
| `path:docs` | Filter by folder path |
| `size:>5MB` | Filter by file size (KB, MB, GB) |

<h3 align="center">Keyboard Shortcuts</h3>

| Shortcut | Action |
|:---|:---|
| `Alt+Space` | Toggle search window (Global Hotkey, configurable in Settings) |
| `Ctrl+Enter` | Open selected file |
| `Ctrl+C` | Copy file path |
| `Esc` | Close search window |
| `↑/↓` | Navigate results |

<h3 align="center">Command-Line Interface</h3>

Flash Search includes a CLI mode for terminal-based querying and scripting:

```bash
# Basic terminal text search
flash-search --cli "query"

# Output as JSON 
flash-search --cli "query" --json
```

<h3 align="center">App Data & Logs</h3>

Flash Search automatically manages its index database and rolling background logs (retained up to 30 days) locally in the `com.flashsearch` data directory:
- **Windows**: `%AppData%\com.flashsearch` or `%LocalAppData%\com.flashsearch`
- **macOS/Linux**: `~/.local/share/com.flashsearch` or `~/.config/com.flashsearch`

<h2 align="center">🏗️ Tech Stack</h2>

<h3 align="center">Core Architecture</h3>

| Component | Technology | Purpose |
|:---|:---|:---|
| **Language** | Rust | Zero-overhead, memory-safe core |
| **GUI** | Iced | High-performance, cross-platform UI framework (<30MB RAM) |
| **Search Engine** | Tantivy | Full-text indexing with BM25 scoring |
| **Document Intelligence** | [Kreuzberg](https://github.com/kreuzberg-dev/kreuzberg) | Universal text extraction (75+ formats) + OCR |
| **Metadata DB** | redb | Pure Rust key-value storage |
| **Concurrency** | Rayon + Tokio | Parallel processing + async I/O |

<h3 align="center">Supported File Formats</h3>

| Category | Supported Formats | OCR |
|:---|:---|:---:|
| **Digital Documents** | PDF (Native & Scanned), XPS, OXPS, PS, EPS | ✅ |
| **MS Office** | DOC, DOCX, XLS, XLSX, PPT, PPTX (incl. Macro/Template variations) | ❌ |
| **Images** | JPG, PNG, WEBP, BMP, GIF, TIFF, JP2, PNM, PBM, PGM, PPM | ✅ |
| **Text & Markup** | MD, TXT, RTF, HTML, HTM, XML, SVG, LaTeX (TEX), RST, ORG | ❌ |
| **E-books & Comics** | EPUB, FB2, MOBI, AZW3, CHM, CBZ, CBR | ❌ |
| **Archives** | ZIP, 7Z, TAR, TGZ, GZ, RAR | ❌ |
| **Data & Emails** | JSON, YAML, TOML, CSV, ODS, EML, MSG | ❌ |

<h2 align="center">📊 Performance</h2>

<div align="center">

Benchmarks on AMD Ryzen 7 5800X with NVMe SSD:

</div>

| Metric | Value |
|:---|:---|
| **Index 10,000 PDFs** | ~45 seconds |
| **Index 100,000 TXT files** | ~12 seconds |
| **Search latency (p50)** | 12ms |
| **Search latency (p99)** | 45ms |
| **Idle RAM usage** | ~25MB |
| **Peak RAM (indexing)** | ~180MB |

<h3 align="center">Comparison</h3>

<div align="center">

| Feature | Flash Search | AnyTXT | Windows Search | Recoll |
|:---|:---|:---|:---|:---|
| Startup Time | Instant | ~2s | System | ~3s |
| RAM Usage | 25MB | 80MB | 150MB+ | 120MB |
| PDF Search | ✅ (Bundled) | ✅ | ⚠️ | ✅ |
| OCR Support | ✅ (Built-in) | ✅ | ❌ | ⚠️ |
| 75+ Formats | ✅ | ✅ | ❌ | ⚠️ |
| Live Updates | ✅ | ✅ | ✅ | ❌ |
| Cross-Platform | ✅ | ⚠️ | ❌ | ✅ |

</div>

<h2 align="center">🗺️ Roadmap</h2>

### Phase 1: Foundation (Completed ✅)
- [x] High-performance core with **Rust**
- [x] Efficient UI framework with **Iced**
- [x] Basic file parsers for PDF, DOCX, and TXT
- [x] Parallel filesystem scanning and incremental indexing

### Phase 2: Polish & UX (Completed ✅)
- [x] Advanced search filters (date, size, extension)
- [x] Global "Spotlight-style" search bar (Alt+Space)
- [x] Real-time file indexing and system tray integration
- [x] Fast filename-only search mode for instant navigation
- [x] Optimized result preview and metadata retrieval
- [x] **Network Discovery**: Support for enterprise SMB/CIFS and NAS drives

### Phase 3: Advanced Intelligence (In Progress 🚧)
- [x] **Universal Text Extraction**: 75+ formats supported via **Kreuzberg**
- [x] **Integrated OCR**: Native support for scanned PDFs and images
- [x] **Pinned Results**: Favorite important files for instant access
- [ ] **Semantic Search**: Vector embeddings for context-aware queries
- [ ] **Natural Language Queries**: "files from last week about taxes"

### Phase 4: Expansion & Connectivity (Future 🔮)
- [ ] **Plugin System**: Community-built parsers and search extensions
- [ ] **Cloud Indexing**: Optional encrypted metadata sync across devices
- [ ] **Developer API**: Headless search service for other applications

<h2 align="center">🤝 Contributing</h2>

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


Flash Search is built on the shoulders of giants. We are deeply grateful to the following projects:

- **[Kreuzberg](https://github.com/kreuzberg-dev/kreuzberg)** - Our secret weapon for universal document parsing and high-performance OCR.
- **[Tantivy](https://github.com/quickwit-oss/tantivy)** - The blazing-fast, index-based search engine that powers our core.
- **[Iced](https://iced.rs/)** - A battery-included, type-safe GUI library that keeps our RAM usage minimal.
- **[Redb](https://github.com/cberner/redb)** - A simple, portable, and high-performance embedded key-value store.
- **[Tokio](https://tokio.rs/)** & **[Rayon](https://github.com/rayon-rs/rayon)** - The powerhouses behind our asynchronous I/O and parallel processing.


---

<p align="center">
  Made with ❤️ using Rust
</p>
