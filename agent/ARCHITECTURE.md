# Doty - Architecture & Design

## 1. Overview
**Doty** is a hybrid dotfiles manager written in Rust. It bridges the gap between directory-linking (GNU Stow) and file-linking (Dotter) strategies, offering high configurability and safety.

## 2. Core Concepts

### 2.1 Linking Strategies
Doty supports granular linking strategies per package or path:

*   **`LinkFolder` (Stow-like)**
    *   **Behavior**: Creates a single symbolic link for the root directory of the package.
    *   **Use Case**: `.config/app` directories where you want to track *all* files, including new ones created by the app, immediately in Git.
    *   **Pros**: Zero maintenance for new files; "What you see is what you get".
    *   **Cons**: Overwrites the target directory; some apps might fail if their config root is a symlink.

*   **`LinkFilesRecursive` (Dotter-like)**
    *   **Behavior**: Recreates the directory structure at the target and symlinks individual files.
    *   **Use Case**: Complex configs, apps that dislike symlinked roots, or when you only want to manage specific files in a shared folder.
    *   **Pros**: Safe; merges with existing files; doesn't disturb untracked files.
    *   **Cons**: New files created by the app are not automatically tracked (requires `detect`).

### 2.2 Drift Detection
Because `LinkFilesRecursive` allows the target directory to contain files not in the repo, Doty needs a way to find them.
*   **Command**: `doty detect`
*   **Function**: Scans target directories defined in the config and reports "Untracked" files (present in target but not in source) or "Drift" (target file modified and no longer pointing to source).

## 3. Configuration
*   **Format**: [KDL (Kuddle)](https://kdl.dev/)
*   **File**: `doty.kdl`
*   **Reasoning**: Rust-native, intuitive CLI-like syntax, supports type annotations, and has excellent comment support.

### Example Config
```kdl
// doty.kdl

// Global defaults
defaults {
    strategy "LinkFilesRecursive"
}

// Simple package using LinkFolder (Stow-mode)
link "nvim" {
    source "nvim"
    target "~/.config/nvim"
    strategy "LinkFolder"
}

// Complex package using LinkFilesRecursive (Dotter-mode)
package "zsh" {
    // Explicit link with attributes
    link ".zshrc" source="zsh/.zshrc" target="~/.zshrc"
    
    // Inherits default strategy (LinkFilesRecursive)
    link "scripts" source="zsh/scripts" target="~/bin"
}
```

## 4. CLI Commands
*   `doty link` (or `install`): Applies the configuration, creating symlinks based on the chosen strategy.
*   `doty clean`: Removes symlinks managed by Doty.
*   `doty detect`: Audits targets for untracked files or broken links.
*   `doty status`: Shows current system health and mapping status.

## 5. Tech Stack
*   **Language**: Rust
*   **Config Parser**: `kdl`
*   **CLI Arguments**: `clap`
*   **Path Handling**: `camino` (UTF-8 paths) or `std::path`
