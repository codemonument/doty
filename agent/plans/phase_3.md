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

### 3.2 Detect Command (`commands.rs::detect()`)

**Purpose**: Execute the drift detection and report findings to user

**Signature**:

```rust
pub fn detect(config_path: Utf8PathBuf, interactive: bool) -> Result<()>
```

**Logic**:

1. Load config and state
2. Create linker and scanner
3. Run `scanner.scan_targets()`
4. Print categorized results:
   - **Untracked files (only for `LinkFilesRecursive` packages)**
   - Broken symlinks
   - Modified files
5. If `interactive`:
   - For untracked files: prompt "Adopt these files? [y/n]"
   - For broken links: prompt "Remove broken symlink? [y/n]"
   - Execute chosen actions

**Output format**:

```
Untracked files in LinkFilesRecursive nvim → ~/.config/nvim:
  [?] ~/.config/nvim/plugin/custom.lua
  [?] ~/.config/nvim/after/ftplugin/rust.lua

Broken symlinks:
  [!] ~/.zshrc → zsh/.zshrc (source missing)

Run 'doty detect --interactive' to adopt or cleanup
```

**Note**: `LinkFolder` packages won't appear in untracked files section because the entire directory is symlinked.

**Tests**:

- Test non-interactive mode (reporting only) - standard unit tests
- Test interactive mode with `portable-pty` - see section 3.5
- Integration test with full workflow
- Verify `LinkFolder` packages don't generate false positives

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

1. **Scanner module** (3.1) - Core logic, no CLI interaction
   - Focus on `LinkFilesRecursive` directory scanning
   - Skip untracked file detection for `LinkFolder`
2. **Detect command** (3.2) - Non-interactive mode first
3. **Helper functions** (3.4) - Move, update config, prompts
4. **PTY testing infrastructure** (3.5) - Setup before interactive features
5. **Adopt manual mode** (3.3.A) - Full manual workflow with PTY tests
6. **Detect interactive** (3.2 + 3.3.B) - Connect detect to adopt with PTY tests
7. **Polish** - Error handling, edge cases, better UX

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
