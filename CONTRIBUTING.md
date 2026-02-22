# Contributing to Flash Search

Thank you for your interest in contributing to Flash Search!

## Development Setup

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and setup
git clone https://github.com/yourusername/flash-search.git
cd flash-search

# Run development server
cargo run
```

## Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test
```

## Code Style

- Use `cargo fmt` before committing
- Use `cargo clippy` to catch common mistakes
- Follow existing code conventions

## Pull Request Process

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests and formatting
5. Submit a pull request

## Reporting Issues

Please include:
- Steps to reproduce
- Expected vs actual behavior
- System information

## License

By contributing, you agree that your contributions will be licensed under GPLv3.
