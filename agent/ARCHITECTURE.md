# Doty - Architecture & Design

## 1. Overview

**Doty** is a hybrid dotfiles manager written in Rust. It bridges the gap
between directory-linking (GNU Stow) and file-linking (Dotter) strategies,
offering high configurability and safety.

## 2. Core Concepts

### 2.1 Linking Strategies

Doty supports granular linking strategies per package or path:

- **`LinkFolder` (Stow-like)**
  - **Behavior**: Creates a single symbolic link for the root directory of the
    package.
  - **Use Case**: `.config/app` directories where you want to track _all_ files,
    including new ones created by the app, immediately in Git.
  - **Pros**: Zero maintenance for new files; "What you see is what you get".
  - **Cons**: Overwrites the target directory; some apps might fail if their
    config root is a symlink.

- **`LinkFilesRecursive` (Dotter-like)**
  - **Behavior**: Recreates the directory structure at the target and symlinks
    individual files.
  - **Use Case**: Complex configs, apps that dislike symlinked roots, or when
    you only want to manage specific files in a shared folder.
  - **Pros**: Safe; merges with existing files; doesn't disturb untracked files.
  - **Cons**: New files created by the app are not automatically tracked
    (requires `detect`).

### 2.2 Drift Detection

Because `LinkFilesRecursive` allows the target directory to contain files not in
the repo, Doty needs a way to find them.

- **Command**: `doty detect`
- **Function**: Scans target directories defined in the config and reports
  "Untracked" files (present in target but not in source) or "Drift" (target
  file modified and no longer pointing to source).

### 2.3 State Management

To safely manage symlinks (especially deletions), Doty tracks the state of
deployed links.

- **Location**: `.doty/state/<hostname>.kdl`
- **Purpose**:
  - Tracks exactly which symlinks were created by Doty on this specific machine.
  - Enables safe "cleaning" (removing only what we created).
  - Committed to Git to allow auditing deployments across machines.

## 3. Configuration

- **Format**: [KDL (Kuddle)](https://kdl.dev/)
- **File**: `doty.kdl`
- **Reasoning**: Rust-native, intuitive CLI-like syntax, supports type
  annotations, and has excellent comment support.

### 3.1 Path Resolution Strategy

Doty supports two strategies for resolving source paths:

- **`config` (Config File Location)** *(Default)*
  - Source paths are resolved relative to the directory containing `doty.kdl`
  - **Use Case**: Consistent behavior regardless of where command is run
  - **Example**: If `doty.kdl` is in `~/dotfiles/configs/`, `nvim` always resolves to `~/dotfiles/configs/nvim`
  - **Benefit**: Allows config file in subdirectory while managing parent directory files

- **`cwd` (Current Working Directory)**
  - Source paths are resolved relative to where `doty` command is executed
  - **Use Case**: When you want to run `doty` from different locations with same config
  - **Example**: Running `doty link` from `~/dotfiles/work/` would resolve `nvim` to `~/dotfiles/work/nvim`

**Example Scenario:**
```
~/dotfiles/
├── configs/
│   └── doty.kdl          # Config file location
├── nvim/                 # Source files
├── zsh/
└── .doty/
    └── state/

# With pathResolution "config" (default):
cd ~/                     # Can run from anywhere
doty -c ~/dotfiles/configs/doty.kdl link
# → nvim resolves to ~/dotfiles/configs/nvim (relative to config)

# With pathResolution "cwd":
cd ~/dotfiles/            # Must run from repo root
doty -c configs/doty.kdl link
# → nvim resolves to ~/dotfiles/nvim (relative to cwd)
```

### Example Config

```kdl
// doty.kdl

// Global defaults
defaults {
    // Path resolution strategy: "config" (default) or "cwd"
    // "config" - resolve paths relative to doty.kdl location
    // "cwd" - resolve paths relative to current working directory
    pathResolution "config"
}

// Simple package using LinkFolder (Stow-mode)
// First argument is the source path relative to the repo root
// (repo root determined by pathResolution strategy)
LinkFolder "nvim" {
    target "~/.config/nvim"
}

// Single line LinkFolder example
LinkFolder "alacritty" target="~/.config/alacritty"

// Single line linking example using LinkFilesRecursive (Dotter-mode)
LinkFilesRecursive "zsh/.zshrc" target="~/.zshrc"

// Another recursive link
LinkFilesRecursive "zsh/scripts" target="~/scripts"
```

## 4. CLI Commands

### Global Options

- **`--config <path>` / `-c <path>`**: Path to the config file (default: `./doty.kdl`)
  - Specifies which config file to use
  - Can be absolute or relative path
  - Example: `doty -c ~/dotfiles/configs/doty.kdl link`

### 4.1 `doty link`

- **Aliases**: `deploy`, `install`, `i`
- **Description**: Applies the configuration, creating symlinks based on the
  chosen strategy.
- **Options**:
  - `--dry-run`: Simulates changes (creations/deletions) without modifying the
    filesystem.
- **Config File Discovery**:
  1. If `--config` / `-c` is specified, use that file
  2. Otherwise, search for `doty.kdl` in current working directory
  3. If not found, error with helpful message
- **Path Resolution of paths inside the config file**:
  - Behavior depends on `defaults.pathResolution` in config:
    - `config` (default): Resolve source paths relative to directory containing `doty.kdl`
    - `cwd`: Resolve source paths relative to current working directory
  - This allows flexible repo structures (e.g., config in subfolder)
- **Logic**:
  1. Read `doty.kdl` and `.doty/state/<hostname>.kdl`.
  2. Calculate Diff (New links, Modified links, Deleted links).
  3. Apply changes (unless `--dry-run`).
  4. Update state file.

### 4.2 `doty clean`

- **Aliases**: `unlink`, `uninstall`, `remove`, `rm`
- **Description**: Removes all symlinks managed by Doty on this machine.
- **Logic**: Uses `.doty/state/<hostname>.kdl` to identify and remove only
  Doty-managed links.

### 4.3 `doty adopt`

- **Description**: Interactive wizard to import existing local configs into the
  Doty repo.
- **Workflow**:
  1. User provides path (e.g., `~/.config/alacritty`).
  2. Prompt: Choose Strategy (`LinkFolder` vs `LinkFilesRecursive`).
  3. Prompt: Select files/folders to ignore.
  4. **Action**:
     - Move files from Target -> Source Repo.
     - Update `doty.kdl`.
     - Run `doty link` logic.
  5. **Ignore Handling**:
     - `LinkFilesRecursive`: Ignored files stay in Target (physically).
     - `LinkFolder`: Ignored files moved to Source but added to `.gitignore`.

### 4.4 `doty detect`

- **Description**: Audits targets for untracked files or broken links.
- **Interactive Mode**:
  - If untracked files are found: Ask to **Adopt** them (trigger `doty adopt`
    logic).
  - If broken links are found: Ask to **Cleanup** (remove dangling symlink).

### 4.5 `doty status`

- **Description**: Shows current system health, mapping status, and sync state.

## 5. Tech Stack

- **Language**: Rust
- **Config Parser**: `kdl`
- **CLI Arguments**: `clap`
- **Path Handling**: `camino` (UTF-8 paths) or `std::path`

## 6. Implementation Phases

### Phase 1: Core Foundation ✅

- [x] Project setup (`cargo init`, dependencies).
- [x] **Config Engine**: Implement `kdl` parsing to Rust structs.
- [x] **State Engine**: Implement reading/writing `.doty/state/<hostname>.kdl`.
- [x] **Tests**: Unit tests for Config and State serialization/deserialization.

### Phase 1.1: Switch from directly using std::fs to using the vfs crate ✅

- [x] **Config Engine**: Switch from directly using std::fs to using the vfs crate
- [x] **State Engine**: Switch from directly using std::fs to using the vfs crate
- [x] **Tests**: Integration tests for Config and State serialization/deserialization (mock filesystem via vfs crates MemoryFS).

### Phase 1.2: Target Path Resolution Fix ✅

**Problem**: The target root in the linker was being calculated from `$HOME` unconditionally, which was incorrect. Target paths should support:
1. Absolute paths (starting with `/`)
2. Relative paths (resolved relative to cwd)
3. `~` expansion (relative to HOME)

**Solution**: 
1. **Remove `target_root` parameter**: The `Linker` no longer takes a `target_root` parameter
2. **Dynamic path resolution**: Target paths are now resolved dynamically based on their format
3. **Fix `resolve_target_path()`**: Properly handles `~` expansion, absolute paths, and relative paths
4. **Fix `clean()` function**: Use `symlink_metadata()` instead of `exists()` to handle broken symlinks
5. **Update tests**: All tests now use absolute paths for targets and pass successfully

**Tasks**:
- [x] Remove `target_root` field from `Linker` struct
- [x] Update `Linker::new()` to only take `repo_root` parameter
- [x] Fix `resolve_target_path()` to handle all path types correctly
- [x] Update `commands.rs` to remove `target_root` calculation
- [x] Fix `clean()` function to handle broken symlinks
- [x] Update all tests to use new constructor and absolute paths
- [x] Verify all 34 tests pass

### Phase 2: The Linker (Core Logic) ✅

- [x] **Strategy: LinkFolder**: Implement directory symlinking logic.
- [x] **Tests**: Unit tests for LinkFolder (mock filesystem via vfs crates MemoryFS).
- [x] **Strategy: LinkFilesRecursive**: Implement recursive file symlinking logic.
- [x] **Tests**: Unit tests for LinkFilesRecursive (mock filesystem via vfs crates MemoryFS).
- [x] **Command: Link**: Implement `doty link` with `--dry-run` and State updates.
- [x] **Tests**: Integration tests for Link (mock filesystem via vfs crates MemoryFS).
- [x] **Command: Clean**: Implement `doty clean` using State.
- [x] **Tests**: Integration tests for Clean (mock filesystem via vfs crates MemoryFS).

### Phase 2.1: Path Resolution Strategy ✅

- [x] **Config**: Add `pathResolution` field to defaults section
- [x] **Parser**: Parse and validate pathResolution setting ("config" or "cwd")
- [x] **CLI**: Implement path resolution logic in commands
- [x] **Tests**: Unit tests for both resolution strategies
- [x] **Documentation**: Update examples and help text

### Phase 3: Detection & Adoption

- [ ] **Scanner**: Implement logic to scan targets and compare with
      Source/State.
- [ ] **Command: Detect**: Implement reporting of untracked/broken files.
- [ ] **Command: Adopt**: Implement file moving and config updating logic.
- [ ] **Interactivity**: Add prompts to `detect` for immediate adoption/cleanup.

### Phase 4: Polish & CLI Experience

- [ ] **Command: Status**: Implement system health overview.
- [ ] **UX**: Pretty printing (colors, diff tables).
- [ ] **Error Handling**: Robust error messages and recovery suggestions.
