# Third-Party Licenses

Flash Search is built upon several open-source libraries. This document provides a summary of these dependencies and their respective licenses.

## Core Dependencies

| Library | Purpose | License |
|---------|---------|---------|
| [Slint](https://slint.dev/) | GUI Framework | GPL-3.0 / Apache-2.0 |
| [Tantivy](https://github.com/quickwit-oss/tantivy) | Search Engine | MIT |
| [redb](https://github.com/cberner/redb) | Metadata Database | MIT / Apache-2.0 |
| [Tokio](https://tokio.rs/) | Async Runtime | MIT |
| [Rayon](https://github.com/rayon-rs/rayon) | Data Parallelism | MIT / Apache-2.0 |
| [quick-xml](https://github.com/tafia/quick-xml) | XML Parsing | MIT |
| [zip-rs](https://github.com/zip-rs/zip) | ZIP Extraction | MIT |
| [pdf-extract](https://github.com/jrmuizel/pdf-extract) | PDF Parsing | MIT |
| [memmap2](https://github.com/RazrFalcon/memmap2-rs) | Memory Mapping | MIT / Apache-2.0 |
| [blake3](https://github.com/BLAKE3-team/BLAKE3) | Hashing | CC0-1.0 / Apache-2.0 |
| [mimalloc](https://github.com/microsoft/mimalloc) | Memory Allocator | MIT |

## Other Dependencies

Most other utilities used (e.g., `anyhow`, `thiserror`, `chrono`, `walkdir`, `regex`, `arboard`, `opener`, `rfd`) are licensed under either **MIT** or **Apache-2.0**.

The [litchi](https://github.com/DevExzh/litchi.git) crate used for document parsing is licensed under **GPL-3.0**.

---

For full details, please refer to the individual package metadata on [crates.io](https://crates.io).
