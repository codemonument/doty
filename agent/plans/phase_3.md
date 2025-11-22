# Phase 3 Implementation Plan: Detection & Adoption

## Overview

Phase 3 focuses on detecting untracked files in target directories and providing an interactive adoption workflow to import existing configs into the Doty repository. This phase requires:

1. A scanner to compare filesystem reality with config/state
2. A `detect` command for reporting
3. An `adopt` command for importing files
4. Interactive prompts for user decisions
5. **Proper testing infrastructure for interactive CLI using `portable-pty`**

## Existing Foundation

The codebase already has:

- ✅ Config parsing with `LinkStrategy` support (`LinkFolder` and `LinkFilesRecursive`)
- ✅ State management for tracking deployed links
- ✅ Linker with diff calculation and filesystem operations
- ✅ Path resolution strategy (config vs cwd)
- ✅ VFS abstraction for testing

## Phase 3 Tasks Breakdown

### 3.0 Filesystem Utilities Refactoring (`src/fs_utils.rs`)

**Purpose**: Extract reusable filesystem operations from `linker.rs` into a shared module that both the linker and scanner can use.

**Rationale**: The scanner will need many of the same filesystem operations that the linker already implements. Rather than duplicating code, we extract these utilities into a shared module following the DRY principle.

**Functions to extract from `linker.rs`:**

```rust
// 1. Directory scanning
pub fn scan_directory_recursive(dir: &Utf8Path) -> Result<Vec<Utf8PathBuf>>
// - Pure filesystem scanning logic
// - Returns all files in directory tree
// - Currently private method in Linker (line 416-432)

// 2. Path resolution
pub fn resolve_target_path(target: &Utf8Path, base_path: &Utf8Path) -> Result<Utf8PathBuf>
// - Handles ~ expansion (relative to HOME)
// - Handles absolute paths
// - Handles relative paths (relative to base_path)
// - Currently private method in Linker (line 498-517)
// - Change from self.config_dir_or_cwd to base_path parameter

// 3. Filesystem type detection
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsType {
    File,
    Directory,
    Symlink,
}

pub fn get_fs_type(path: &Utf8Path) -> Result<Option<FsType>>
// - Returns filesystem type for a given path
// - Uses symlink_metadata to handle broken symlinks
// - Extracted from enrich_status logic (lines 279-293)

// 4. Symlink operations
pub fn read_symlink_target(path: &Utf8Path) -> Result<Option<Utf8PathBuf>>
// - Reads where a symlink points to (canonical path)
// - Returns None if not a symlink or broken
// - Extracted from enrich_status logic (lines 283-288)

pub fn is_broken_symlink(path: &Utf8Path) -> Result<bool>
// - Checks if path is a symlink that points nowhere
// - New helper function combining symlink check + target validation
```

**Update `src/linker.rs`:**

```rust
// Add import at top of file
use crate::fs_utils::{
    scan_directory_recursive,
    resolve_target_path,
    FsType,
    get_fs_type,
    read_symlink_target,
};

// Remove private implementations of:
// - scan_directory_recursive() (delete lines 416-432)
// - resolve_target_path() (delete lines 498-517)
// - FsType enum (delete lines 42-46)

// Update method calls:
// Before: self.scan_directory_recursive(&source_path)
// After:  scan_directory_recursive(&source_path)

// Before: self.resolve_target_path(target)
// After:  resolve_target_path(target, &self.config_dir_or_cwd)

// Update enrich_status() to use get_fs_type() and read_symlink_target()

// Keep in linker.rs (symlink-specific operations):
// - create_symlink() - Platform-specific symlink creation
// - create_link() - Link creation workflow with parent dir creation
// - remove_link() - Link removal workflow
```

**Update `src/main.rs`:**

```rust
// Add new module declaration
mod fs_utils;
```

**Benefits:**

- ✅ **DRY Principle**: No code duplication between linker and scanner
- ✅ **Testability**: Can test filesystem utilities independently
- ✅ **Clarity**: Separates concerns (filesystem ops vs linking logic)
- ✅ **Reusability**: Other future modules can use these utilities
- ✅ **Maintenance**: Bug fixes in path resolution benefit both linker and scanner

**Tests:**

- Test `scan_directory_recursive()` with nested directories
- Test `resolve_target_path()` with ~, absolute, and relative paths
- Test `get_fs_type()` with files, directories, and symlinks
- Test `read_symlink_target()` with valid and broken symlinks
- Test `is_broken_symlink()` detection
- Use real filesystem for these tests (not MemoryFS, as these are low-level utilities)

**Implementation steps:**

1. Create `src/fs_utils.rs` with extracted functions
2. Add comprehensive tests for each utility function
3. Update `src/linker.rs` to use `fs_utils` functions
4. Run existing linker tests to ensure no regressions
5. Update `src/main.rs` to declare the new module

### 3.1 Scanner Module (`src/scanner.rs`)

**Purpose**: Scan target directories and detect differences between filesystem reality and Doty's knowledge (config + state)

**Key structures**:

```rust
pub enum DriftType {
    Untracked,      // File exists in target but not in source (LinkFilesRecursive only)
    Broken,         // Symlink exists but points nowhere
    Modified,       // Target file modified (not a symlink anymore)
    Orphaned,       // In state but not in config (already handled by linker)
}

pub struct DriftItem {
    pub target_path: Utf8PathBuf,
    pub drift_type: DriftType,
    pub package: Option<Package>,  // Which package this relates to
}
```

**Functions**:

- `scan_targets(config: &DotyConfig, state: &DotyState, linker: &Linker) -> Result<Vec<DriftItem>>`
  - Iterate through all packages in config
  - **For `LinkFilesRecursive` packages where source is a directory:**
    - Scan target directory for files
    - Compare with source directory to find untracked files
    - Files in target but not in source = Untracked
  - **For `LinkFolder` packages:**
    - Skip scanning (target is symlinked to source, no drift possible)
    - Only check if the symlink itself is broken
  - Compare with state to identify broken/orphaned links
  - Return list of drift items

**Scanning logic detail**:

```rust
// For each package
match package.strategy {
    LinkStrategy::LinkFolder => {
        // Only check if the symlink is valid
        // No untracked file detection needed
        if target_is_broken_symlink {
            drift_items.push(DriftItem::Broken);
        }
    }
    LinkStrategy::LinkFilesRecursive => {
        // Only scan if source is a directory
        if source.is_dir() {
            let source_files = scan_directory_recursive(&source)?;
            let target_files = scan_directory_recursive(&target)?;
            
            for target_file in target_files {
                let relative_path = target_file.strip_prefix(&target)?;
                let corresponding_source = source.join(relative_path);
                
                if !corresponding_source.exists() {
                    // File in target but not in source = Untracked
                    drift_items.push(DriftItem::Untracked);
                }
            }
        }
    }
}
```

**Tests**:

- Test untracked file detection for `LinkFilesRecursive` with directory source
- Test that `LinkFolder` doesn't scan for untracked files
- Test broken symlink detection for both strategies
- Test mixed scenarios (some tracked, some untracked)
- Use MemoryFS for testing

### 3.2 Detect Command (`commands.rs::detect()`) ✅ COMPLETED

**Purpose**: Execute the drift detection and report findings to user

**Signature**:

```rust
pub fn detect(config_path: Utf8PathBuf, interactive: bool) -> Result<()>
```

**Logic**:

1. Load config and state using existing patterns
2. Create scanner (not linker - scanner handles drift detection)
3. Run `scanner.scan_targets()` to get all drift items
4. Group drift items by type and package:
   - **Untracked files** (only for `LinkFilesRecursive` packages)
   - **Broken symlinks**
   - Modified files (handled elsewhere)
5. Print categorized results with clear formatting
6. If `interactive`:
   - Show placeholder for interactive adoption (step 3.3)
   - Suggest running interactive mode for adoption/cleanup

**Output format**:

```
Detecting unmonitored files 🔍
Config: /path/to/doty.kdl

Untracked files in LinkFilesRecursive source/test-app-dir → target/test-app-dir:
  [?] /path/to/target/test-app-dir/user-custom.txt

Broken symlinks:
  [!] /path/to/target/broken-link → /path/to/source/missing

Run 'doty detect --interactive' interactive mode to adopt or cleanup
```

**Interactive mode**:

```
Detecting unmonitored files 🔍 [INTERACTIVE]
Config: /path/to/doty.kdl

Untracked files in LinkFilesRecursive source/test-app-dir → target/test-app-dir:
  [?] /path/to/target/test-app-dir/user-custom.txt

Broken symlinks:
  [!] /path/to/target/broken-link → /path/to/source/missing

Interactive mode:
Adopt untracked files for LinkFilesRecursive source/test-app-dir → target/test-app-dir:
  (Interactive adoption not yet implemented - see step 3.3)

Remove broken symlinks?
  (Interactive cleanup not yet implemented - see step 3.3)
```

**Key Features Implemented**:

- ✅ **Strategy-aware detection**: Only shows untracked files for `LinkFilesRecursive` packages
- ✅ **LinkFolder compliance**: `LinkFolder` packages don't generate false untracked positives
- ✅ **Clear reporting**: Groups by package with source→target identification
- ✅ **Broken symlink detection**: Shows source information from state when available
- ✅ **Interactive foundation**: Detects `--interactive` flag and shows placeholders for step 3.3
- ✅ **Consistent formatting**: Uses colored output and patterns from other commands
- ✅ **Error handling**: Proper context and error messages
- ✅ **CLI integration**: Supports both `--interactive` and `-i` options

**Tests**:

- ✅ Non-interactive mode reporting (standard unit tests)
- ✅ Interactive mode detection and placeholder display
- ✅ Integration testing with real playground scenarios
- ✅ Verified `LinkFolder` packages don't show untracked files
- ✅ Verified `LinkFilesRecursive` packages correctly detect untracked files
- ✅ All 50 tests pass (45 existing + 5 scanner tests)

**CLI Integration**:

```rust
Detect {
    /// Run in interactive mode for adoption/cleanup
    #[arg(short = 'i', long)]
    interactive: bool,
},
```

**Note**: `LinkFolder` packages won't appear in untracked files section because the entire directory is symlinked.

### 3.3 Adopt Command (`commands.rs::adopt()`)

**Purpose**: Interactive wizard to import existing configs into Doty

**Two modes**:

#### A. Manual mode (user provides path):

```rust
pub fn adopt_manual(config_path: Utf8PathBuf, target_path: Utf8PathBuf) -> Result<()>
```

1. Prompt: "Which strategy? [1=LinkFolder, 2=LinkFilesRecursive]"
2. Prompt: "Enter source path in repo (e.g., 'nvim'):"
3. If directory: prompt "Select files to ignore (space-separated):"
4. Move files: target → source
5. Update `doty.kdl` with new package entry
6. Run `doty link` logic to create symlinks

#### B. From detect mode (called by `detect --interactive`):

```rust
pub fn adopt_from_detect(drift_items: Vec<DriftItem>, config: &mut DotyConfig) -> Result<()>
```

1. Group untracked files by package (only `LinkFilesRecursive` packages will have untracked files)
2. For each package with untracked files:
   - Show list of untracked files
   - Prompt: "Adopt these files? [y/n/s(elect)]"
   - If select: let user choose which files
3. Move selected files from target → source
4. Update state (files are now tracked)
5. No need to update config (package already exists)

**Ignore handling**:

- `LinkFolder`: Copy ignored files to source but add to `.gitignore`
- `LinkFilesRecursive`: Leave ignored files in target (not tracked)

**Tests**:

- Test manual adoption workflow with `portable-pty`
- Test adoption from detect mode with `portable-pty`
- Test ignore handling for both strategies
- Test config file updates
- Use MemoryFS for file operations testing

### 3.4 Helper Functions

**Move files safely**:

```rust
fn move_file_to_source(
    target_path: &Utf8Path,
    source_path: &Utf8Path,
    dry_run: bool
) -> Result<()>
```

**Update config file**:

```rust
fn append_package_to_config(
    config_path: &Utf8Path,
    package: &Package
) -> Result<()>
```

**Interactive prompts** (using `dialoguer` crate):

```rust
fn prompt_yes_no(message: &str) -> Result<bool>
fn prompt_choice(message: &str, choices: &[&str]) -> Result<usize>
fn prompt_multiselect(message: &str, items: &[String]) -> Result<Vec<usize>>
```

### 3.5 Interactive Testing Infrastructure (`tests/interactive_tests.rs`)

**Purpose**: Test interactive CLI commands using `portable-pty` to simulate real terminal interaction

**Setup**:

```rust
use portable_pty::{native_pty_system, CommandBuilder, PtySize};

struct PtySession {
    pair: portable_pty::PtyPair,
    reader: Box<dyn std::io::Read + Send>,
    writer: Box<dyn std::io::Write + Send>,
}

impl PtySession {
    fn new() -> Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        
        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        
        Ok(Self { pair, reader, writer })
    }
    
    fn send_input(&mut self, input: &str) -> Result<()> {
        write!(self.writer, "{}", input)?;
        self.writer.flush()?;
        Ok(())
    }
    
    fn read_output(&mut self, timeout_ms: u64) -> Result<String> {
        // Read with timeout
        // Parse ANSI escape codes if needed
        // Return clean output
    }
    
    fn expect(&mut self, expected: &str, timeout_ms: u64) -> Result<()> {
        let output = self.read_output(timeout_ms)?;
        assert!(output.contains(expected), 
            "Expected '{}' not found in output: '{}'", expected, output);
        Ok(())
    }
}
```

**Test scenarios**:

#### 1. Test `detect --interactive` with yes/no prompts

```rust
#[test]
fn test_detect_interactive_adopt_yes() {
    // Setup test filesystem with untracked files in LinkFilesRecursive package
    // Launch doty in PTY
    let mut pty = PtySession::new().unwrap();
    let mut cmd = CommandBuilder::new("cargo");
    cmd.args(&["run", "--", "detect", "--interactive"]);
    
    let child = pty.pair.slave.spawn_command(cmd).unwrap();
    
    // Expect prompt
    pty.expect("Adopt these files? [y/n]", 5000).unwrap();
    
    // Send 'y' + Enter
    pty.send_input("y\n").unwrap();
    
    // Verify files were adopted
    pty.expect("✓ Adopted", 5000).unwrap();
    
    child.wait().unwrap();
}

#[test]
fn test_detect_interactive_adopt_no() {
    // Similar but send 'n' and verify files not adopted
}
```

#### 2. Test `adopt` manual workflow

```rust
#[test]
fn test_adopt_manual_link_folder() {
    let mut pty = PtySession::new().unwrap();
    let mut cmd = CommandBuilder::new("cargo");
    cmd.args(&["run", "--", "adopt", "~/.config/test-app"]);
    
    let child = pty.pair.slave.spawn_command(cmd).unwrap();
    
    // Expect strategy prompt
    pty.expect("Which strategy?", 5000).unwrap();
    pty.send_input("1\n").unwrap();  // LinkFolder
    
    // Expect source path prompt
    pty.expect("Enter source path", 5000).unwrap();
    pty.send_input("test-app\n").unwrap();
    
    // Verify success
    pty.expect("✓", 5000).unwrap();
    
    child.wait().unwrap();
}
```

#### 3. Test multiselect prompts

```rust
#[test]
fn test_adopt_with_file_selection() {
    let mut pty = PtySession::new().unwrap();
    // Test space-bar selection of multiple files
    // Test arrow key navigation
    // Test enter to confirm
}
```

#### 4. Test error handling in interactive mode

```rust
#[test]
fn test_interactive_invalid_input() {
    // Test invalid responses (e.g., "xyz" when expecting y/n)
    // Verify re-prompting or error message
}
```

#### 5. Test that LinkFolder packages don't show untracked files

```rust
#[test]
fn test_detect_link_folder_no_untracked() {
    // Setup LinkFolder package
    // Verify no untracked files reported
    // Only broken symlinks should be detected
}
```

**PTY testing patterns**:

- Use `expect()` pattern matching for output verification
- Handle ANSI color codes and cursor movement
- Add timeouts to prevent hanging tests
- Clean up child processes properly
- Test both happy paths and error cases

**Benefits of `portable-pty`**:

- ✅ Tests actual terminal interaction (not mocked)
- ✅ Catches issues with prompt rendering
- ✅ Verifies ANSI color output works correctly
- ✅ Tests keyboard input handling (arrows, space, enter)
- ✅ Platform-independent (works on Unix and Windows)

## Dependencies to Add

```toml
# Cargo.toml
[dependencies]
dialoguer = "0.11"  # For interactive prompts
console = "0.15"    # For better terminal formatting

[dev-dependencies]
portable-pty = "0.8"  # For testing interactive CLI
```

## Implementation Order

1. **Filesystem utilities refactoring** (3.0) - Extract shared code first
   - Create `src/fs_utils.rs` with extracted functions
   - Add tests for filesystem utilities
   - Update `src/linker.rs` to use shared utilities
   - Verify all existing tests still pass
2. **Scanner module** (3.1) - Core logic, no CLI interaction
   - Focus on `LinkFilesRecursive` directory scanning
   - Skip untracked file detection for `LinkFolder`
   - Use `fs_utils` for filesystem operations
3. **Detect command** (3.2) - Non-interactive mode first
4. **Helper functions** (3.4) - Move, update config, prompts
5. **PTY testing infrastructure** (3.5) - Setup before interactive features
6. **Adopt manual mode** (3.3.A) - Full manual workflow with PTY tests
7. **Detect interactive** (3.2 + 3.3.B) - Connect detect to adopt with PTY tests
8. **Polish** - Error handling, edge cases, better UX

## Testing Strategy

- **Unit tests**: Scanner logic, file operations, path resolution (standard tests)
  - Verify `LinkFolder` doesn't generate untracked files
  - Verify `LinkFilesRecursive` correctly detects untracked files
- **Integration tests**: Full workflows with MemoryFS (standard tests)
- **Interactive tests**: PTY-based tests for all user prompts (using `portable-pty`)
- **Manual testing**: Real filesystem testing in playground (human verification)
- **Edge cases**: Empty directories, nested structures, broken symlinks, permission errors

## CLI Integration

Update `main.rs`:

```rust
enum Commands {
    // ... existing commands
    Detect {
        #[arg(short, long)]
        interactive: bool,
    },
    Adopt {
        /// Target path to adopt (e.g., ~/.config/nvim)
        target: Option<Utf8PathBuf>,
    },
}
```

## Success Criteria

- ✅ Scanner accurately detects untracked files **only for `LinkFilesRecursive` packages**
- ✅ Scanner **does not** report untracked files for `LinkFolder` packages
- ✅ `doty detect` reports all drift types
- ✅ `doty detect --interactive` allows immediate adoption
- ✅ `doty adopt` wizard successfully imports configs
- ✅ Both link strategies handle adoption correctly
- ✅ Ignore patterns work as expected
- ✅ Config file is updated correctly
- ✅ All tests pass (unit + integration + PTY interactive tests)
- ✅ Interactive prompts work correctly with keyboard input
- ✅ ANSI colors and formatting render properly

## Key Insight

**`LinkFolder` packages create a single symlink at the directory level**, so the target directory IS the source directory. Any files created by apps in that directory automatically appear in the source (the Git repo). This is the "zero maintenance" benefit mentioned in the architecture.

**`LinkFilesRecursive` packages create individual file symlinks**, so the target directory can contain both:

- Symlinked files (tracked in source)
- Real files (untracked, created by apps)

This is why drift detection only makes sense for `LinkFilesRecursive` packages with directory sources.
