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

### Example Config

```kdl
// doty.kdl

// Global defaults
defaults {
    // Global settings if needed
}

// Simple package using LinkFolder (Stow-mode)
// First argument is the source path relative to the repo root
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

### 4.1 `doty link`

- **Aliases**: `deploy`, `install`, `i`
- **Description**: Applies the configuration, creating symlinks based on the
  chosen strategy.
- **Options**:
  - `--dry-run`: Simulates changes (creations/deletions) without modifying the
    filesystem.
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

### Phase 1: Core Foundation

- [ ] Project setup (`cargo init`, dependencies).
- [ ] **Config Engine**: Implement `kdl` parsing to Rust structs.
- [ ] **State Engine**: Implement reading/writing `.doty/state/<hostname>.kdl`.
- [ ] **Tests**: Unit tests for Config and State serialization/deserialization.

### Phase 2: The Linker (Core Logic)

- [ ] **Strategy: LinkFolder**: Implement directory symlinking logic.
- [ ] **Strategy: LinkFilesRecursive**: Implement recursive file symlinking
      logic.
- [ ] **Command: Link**: Implement `doty link` with `--dry-run` and State
      updates.
- [ ] **Command: Clean**: Implement `doty clean` using State.
- [ ] **Tests**: Integration tests for linking and cleaning (mock filesystem).

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
