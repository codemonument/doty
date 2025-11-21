# Doty - Agent Guidelines

Consult ./agent/ARCHITECTURE.md before implementing any changes!
It includes the current architecture and implementation status.

## Build & Test Commands

```bash
# Build the project
cargo build

# Build with optimizations
cargo build --release

# Run all tests
cargo test

# Run a single test (example)
cargo test test_link_folder_creates_symlink

# Run tests with output
cargo test -- --nocapture

# Check code without building
cargo check

# Format code
cargo fmt

# Run clippy lints
cargo clippy

# Run clippy with all targets and features
cargo clippy --all-targets --all-features
```

## Code Style Guidelines

### Imports & Dependencies
- Use `camino::Utf8PathBuf` for all path operations (UTF-8 guaranteed paths)
- Use `anyhow::{Context, Result}` for error handling with context
- Group imports: std libs first, then external crates, then local modules
- Import specific items rather than using `*` where possible

### Formatting & Types
- Use `cargo fmt` for formatting (rustfmt standard)
- Prefer `Utf8PathBuf` over `PathBuf` for user-facing paths
- Use `vfs::VfsPath` for filesystem operations (enables testing with MemoryFS)
- Define enums with `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` where appropriate
- Use `#[derive(Debug, Clone, PartialEq)]` for structs that need cloning

### Naming Conventions
- Use `snake_case` for functions and variables
- Use `PascalCase` for types and structs
- Use `SCREAMING_SNAKE_CASE` for constants
- Module names should be `snake_case`
- Function names should be descriptive (e.g., `link_folder`, `parse_package`)

### Error Handling
- Use `anyhow::Result<T>` as the return type for fallible functions
- Use `.with_context()` to add context to errors
- Use `anyhow::bail!()` for early returns with errors
- Handle VFS operations with proper error context

### Testing
- Write unit tests in `#[cfg(test)]` modules within each file
- Use `vfs::MemoryFS` for filesystem testing (no real I/O)
- Test both success and error cases
- Use descriptive test names that explain what they test
- Include integration tests for complex workflows

### Architecture Patterns
- Keep modules focused: `config` for parsing, `state` for persistence, `linker` for operations
- Use VFS abstraction for all filesystem operations
- Separate parsing logic from business logic
- Use KDL format for configuration and state files
- Implement `Display` trait for user-facing enums