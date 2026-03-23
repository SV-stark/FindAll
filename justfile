# The default recipe lists all available commands
default:
    @just --list

# Run the project in development mode
run:
    cargo run

# Build the project
build:
    cargo build

# Build the release version
release:
    cargo build --release

# Check if the code compiles without producing binaries
check:
    cargo check --all-targets

# Run all tests
test:
    cargo test --all

# Run clippy to check for linting errors and warnings
clippy:
    cargo clippy --all-targets -- -D warnings

# Format the codebase
fmt:
    cargo fmt --all

# Run all performance benchmarks (Divan)
bench:
    cargo bench

# Build the Windows NSIS Installer (Requires makensis installed)
installer-windows: release
    makensis windows/installer.nsi

# Clean the target directory
clean:
    cargo clean
