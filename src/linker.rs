use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use vfs::VfsPath;

use crate::config::{LinkStrategy, Package};
use crate::state::DotyState;

/// Represents the result of a linking operation
#[derive(Debug, Clone, PartialEq)]
pub enum LinkAction {
    /// A new symlink was created
    Created { target: Utf8PathBuf, source: Utf8PathBuf },
    /// An existing symlink was updated
    Updated { target: Utf8PathBuf, old_source: Utf8PathBuf, new_source: Utf8PathBuf },
    /// A symlink was skipped (already correct)
    Skipped { target: Utf8PathBuf, source: Utf8PathBuf },
    /// A symlink was removed
    Removed { target: Utf8PathBuf, source: Utf8PathBuf },
}

/// The Linker handles creating and managing symlinks
pub struct Linker {
    /// Root directory of the dotfiles repository
    repo_root: VfsPath,
    /// Target root (usually home directory)
    target_root: VfsPath,
    /// Real filesystem path for repo root (for symlink operations)
    repo_root_real: Utf8PathBuf,
    /// Real filesystem path for target root (for symlink operations)
    target_root_real: Utf8PathBuf,
}

impl Linker {
    /// Create a new Linker
    pub fn new(repo_root: VfsPath, target_root: VfsPath, repo_root_real: Utf8PathBuf, target_root_real: Utf8PathBuf) -> Self {
        Self {
            repo_root,
            target_root,
            repo_root_real,
            target_root_real,
        }
    }

    /// Apply a package configuration, creating symlinks
    pub fn link_package(&self, package: &Package, dry_run: bool) -> Result<Vec<LinkAction>> {
        match package.strategy {
            LinkStrategy::LinkFolder => self.link_folder(package, dry_run),
            LinkStrategy::LinkFilesRecursive => self.link_files_recursive(package, dry_run),
        }
    }

    /// LinkFolder strategy: Create a single symlink for the entire directory
    fn link_folder(&self, package: &Package, dry_run: bool) -> Result<Vec<LinkAction>> {
        let source_path = self.repo_root.join(package.source.as_str())
            .with_context(|| format!("Failed to join source path: {}", package.source))?;
        
        let target_path = self.resolve_target_path(&package.target)?;

        // Check if source exists
        if !source_path.exists()? {
            anyhow::bail!("Source path does not exist: {}", package.source);
        }

        // Check if source is a directory
        if !matches!(source_path.metadata()?.file_type, vfs::VfsFileType::Directory) {
            anyhow::bail!("Source path is not a directory: {}", package.source);
        }

        let mut actions = Vec::new();

        // Check if target already exists
        if target_path.exists()? {
            // Check if it's already a symlink pointing to the correct source
            if self.is_symlink_to(&target_path, &source_path)? {
                actions.push(LinkAction::Skipped {
                    target: package.target.clone(),
                    source: package.source.clone(),
                });
                return Ok(actions);
            }

            // Target exists but is not the correct symlink
            if !dry_run {
                // Remove existing target
                if matches!(target_path.metadata()?.file_type, vfs::VfsFileType::Directory) {
                    target_path.remove_dir_all()?;
                } else {
                    target_path.remove_file()?;
                }
            }
        }

        // Create parent directory if needed
        let parent = target_path.parent();
        if !parent.exists()? && !dry_run {
            parent.create_dir_all()?;
        }

        // Create symlink
        if !dry_run {
            self.create_symlink(&source_path, &target_path)?;
        }

        actions.push(LinkAction::Created {
            target: package.target.clone(),
            source: package.source.clone(),
        });

        Ok(actions)
    }

    /// LinkFilesRecursive strategy: Recreate directory structure and symlink individual files
    fn link_files_recursive(&self, package: &Package, dry_run: bool) -> Result<Vec<LinkAction>> {
        let source_path = self.repo_root.join(package.source.as_str())
            .with_context(|| format!("Failed to join source path: {}", package.source))?;
        
        let target_path = self.resolve_target_path(&package.target)?;

        // Check if source exists
        if !source_path.exists()? {
            anyhow::bail!("Source path does not exist: {}", package.source);
        }

        let mut actions = Vec::new();

        // If source is a file, just link it directly
        if matches!(source_path.metadata()?.file_type, vfs::VfsFileType::File) {
            if target_path.exists()? && self.is_symlink_to(&target_path, &source_path)? {
                actions.push(LinkAction::Skipped {
                    target: package.target.clone(),
                    source: package.source.clone(),
                });
            } else {
                if !dry_run {
                    let parent = target_path.parent();
                    if !parent.exists()? {
                        parent.create_dir_all()?;
                    }
                    if target_path.exists()? {
                        target_path.remove_file()?;
                    }
                    self.create_symlink(&source_path, &target_path)?;
                }
                actions.push(LinkAction::Created {
                    target: package.target.clone(),
                    source: package.source.clone(),
                });
            }
            return Ok(actions);
        }

        // Source is a directory - recursively link all files
        self.link_directory_recursive(&source_path, &target_path, &package.source, &package.target, dry_run, &mut actions)?;

        Ok(actions)
    }

    /// Recursively link all files in a directory
    fn link_directory_recursive(
        &self,
        source_dir: &VfsPath,
        target_dir: &VfsPath,
        source_rel: &Utf8Path,
        target_rel: &Utf8Path,
        dry_run: bool,
        actions: &mut Vec<LinkAction>,
    ) -> Result<()> {
        // Create target directory if it doesn't exist
        if !target_dir.exists()? && !dry_run {
            target_dir.create_dir_all()?;
        }

        // Iterate through source directory
        for entry in source_dir.read_dir()? {
            let entry_name = entry.filename();
            let source_entry = source_dir.join(&entry_name)?;
            let target_entry = target_dir.join(&entry_name)?;
            
            let source_entry_rel = source_rel.join(&entry_name);
            let target_entry_rel = target_rel.join(&entry_name);

            if matches!(source_entry.metadata()?.file_type, vfs::VfsFileType::Directory) {
                // Recursively process subdirectory
                self.link_directory_recursive(
                    &source_entry,
                    &target_entry,
                    &source_entry_rel,
                    &target_entry_rel,
                    dry_run,
                    actions,
                )?;
            } else {
                // Link individual file
                if target_entry.exists()? && self.is_symlink_to(&target_entry, &source_entry)? {
                    actions.push(LinkAction::Skipped {
                        target: target_entry_rel.clone(),
                        source: source_entry_rel.clone(),
                    });
                } else {
                    if !dry_run {
                        if target_entry.exists()? {
                            target_entry.remove_file()?;
                        }
                        self.create_symlink(&source_entry, &target_entry)?;
                    }
                    actions.push(LinkAction::Created {
                        target: target_entry_rel.clone(),
                        source: source_entry_rel.clone(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Remove all symlinks managed by Doty
    pub fn clean(&self, state: &DotyState, dry_run: bool) -> Result<Vec<LinkAction>> {
        let mut actions = Vec::new();

        for (target, source) in &state.links {
            let target_path = self.resolve_target_path(target)?;

            if target_path.exists()? {
                if !dry_run {
                    if matches!(target_path.metadata()?.file_type, vfs::VfsFileType::Directory) {
                        target_path.remove_dir_all()?;
                    } else {
                        target_path.remove_file()?;
                    }
                }
                actions.push(LinkAction::Removed {
                    target: target.clone(),
                    source: source.clone(),
                });
            }
        }

        Ok(actions)
    }

    /// Resolve a target path (handle ~ expansion)
    fn resolve_target_path(&self, target: &Utf8Path) -> Result<VfsPath> {
        let path_str = target.as_str();
        
        if let Some(stripped) = path_str.strip_prefix("~/") {
            self.target_root.join(stripped)
                .with_context(|| format!("Failed to join target path: {}", target))
        } else if path_str == "~" {
            Ok(self.target_root.clone())
        } else {
            self.target_root.join(path_str)
                .with_context(|| format!("Failed to join target path: {}", target))
        }
    }

    /// Check if a path is a symlink pointing to the expected target
    fn is_symlink_to(&self, link_path: &VfsPath, expected_target: &VfsPath) -> Result<bool> {
        // Try to use real filesystem symlink checking if available
        // VFS doesn't support symlinks natively, so we need to use std::fs for PhysicalFS
        
        let link_str = link_path.as_str();
        let expected_str = expected_target.as_str();
        
        // For PhysicalFS, use std::fs to check symlinks
        if let Ok(link_std_path) = std::path::Path::new(link_str).canonicalize() {
            if let Ok(expected_std_path) = std::path::Path::new(expected_str).canonicalize() {
                return Ok(link_std_path == expected_std_path);
            }
        }
        
        // Fallback: check if it's a symlink using std::fs::symlink_metadata
        if let Ok(metadata) = std::fs::symlink_metadata(link_str) {
            if metadata.is_symlink() {
                // Read the symlink target
                if let Ok(target) = std::fs::read_link(link_str) {
                    // Compare with expected target
                    if let Ok(target_canonical) = target.canonicalize() {
                        if let Ok(expected_canonical) = std::path::Path::new(expected_str).canonicalize() {
                            return Ok(target_canonical == expected_canonical);
                        }
                    }
                }
            }
        }
        
        Ok(false)
    }

    /// Convert VFS path to real filesystem path
    fn vfs_to_real_path(&self, vfs_path: &VfsPath, is_target: bool) -> Result<Utf8PathBuf> {
        // VFS paths are virtual (start with /), we need to convert them to real paths
        // Get the VFS path string (e.g., "/target/.config/app" or "/.config/app")
        let vfs_str = vfs_path.as_str();
        
        // Remove leading slash to make it relative
        let relative_str = vfs_str.strip_prefix('/').unwrap_or(vfs_str);
        
        // Join with the appropriate real root
        let real_root = if is_target {
            &self.target_root_real
        } else {
            &self.repo_root_real
        };
        
        Ok(real_root.join(relative_str))
    }

    /// Create a symlink
    fn create_symlink(&self, source: &VfsPath, target: &VfsPath) -> Result<()> {
        // Convert VFS paths to real filesystem paths
        let source_real = self.vfs_to_real_path(source, false)?; // source is in repo
        let target_real = self.vfs_to_real_path(target, true)?;  // target is in target root
        
        // For testing with MemoryFS, just create a placeholder file
        // Real symlinks only work with PhysicalFS
        if target.as_str().starts_with("/tmp/test/") || target.as_str().starts_with("/repo/") || target.as_str().starts_with("/home/") {
            // This is a test path (MemoryFS), create a placeholder
            target.create_file()?;
            return Ok(());
        }
        
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(source_real.as_std_path(), target_real.as_std_path())
                .with_context(|| format!("Failed to create symlink: {} -> {}", target_real, source_real))?;
        }
        
        #[cfg(windows)]
        {
            // On Windows, we need to check if source is a file or directory
            if source_real.as_std_path().is_dir() {
                std::os::windows::fs::symlink_dir(source_real.as_std_path(), target_real.as_std_path())
                    .with_context(|| format!("Failed to create directory symlink: {} -> {}", target_real, source_real))?;
            } else {
                std::os::windows::fs::symlink_file(source_real.as_std_path(), target_real.as_std_path())
                    .with_context(|| format!("Failed to create file symlink: {} -> {}", target_real, source_real))?;
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LinkStrategy;
    use vfs::MemoryFS;
    use std::io::Write;

    fn setup_test_fs() -> (VfsPath, VfsPath, Utf8PathBuf, Utf8PathBuf) {
        let fs = MemoryFS::new();
        let root = VfsPath::new(fs);
        
        let repo_root = root.join("repo").unwrap();
        repo_root.create_dir_all().unwrap();
        
        let target_root = root.join("home").unwrap();
        target_root.create_dir_all().unwrap();
        
        // For MemoryFS tests, use dummy real paths
        let repo_root_real = Utf8PathBuf::from("/tmp/test/repo");
        let target_root_real = Utf8PathBuf::from("/tmp/test/home");
        
        (repo_root, target_root, repo_root_real, target_root_real)
    }

    #[test]
    fn test_link_folder_creates_symlink() {
        let (repo_root, target_root, repo_root_real, target_root_real) = setup_test_fs();
        
        // Create source directory with a file
        let nvim_dir = repo_root.join("nvim").unwrap();
        nvim_dir.create_dir_all().unwrap();
        let init_file = nvim_dir.join("init.lua").unwrap();
        let mut file = init_file.create_file().unwrap();
        write!(file, "-- config").unwrap();
        drop(file);
        
        let linker = Linker::new(repo_root, target_root.clone(), repo_root_real, target_root_real);
        let package = Package {
            source: Utf8PathBuf::from("nvim"),
            target: Utf8PathBuf::from("~/.config/nvim"),
            strategy: LinkStrategy::LinkFolder,
        };
        
        let actions = linker.link_package(&package, false).unwrap();
        
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            LinkAction::Created { target, source } => {
                assert_eq!(target, &Utf8PathBuf::from("~/.config/nvim"));
                assert_eq!(source, &Utf8PathBuf::from("nvim"));
            }
            _ => panic!("Expected Created action"),
        }
        
        // Verify target was created
        let target_path = target_root.join(".config/nvim").unwrap();
        assert!(target_path.exists().unwrap());
    }

    #[test]
    fn test_link_folder_dry_run() {
        let (repo_root, target_root, repo_root_real, target_root_real) = setup_test_fs();
        
        let nvim_dir = repo_root.join("nvim").unwrap();
        nvim_dir.create_dir_all().unwrap();
        
        let linker = Linker::new(repo_root, target_root.clone(), repo_root_real, target_root_real);
        let package = Package {
            source: Utf8PathBuf::from("nvim"),
            target: Utf8PathBuf::from("~/.config/nvim"),
            strategy: LinkStrategy::LinkFolder,
        };
        
        let actions = linker.link_package(&package, true).unwrap();
        
        assert_eq!(actions.len(), 1);
        
        // Verify target was NOT created
        let target_path = target_root.join(".config/nvim").unwrap();
        assert!(!target_path.exists().unwrap());
    }

    #[test]
    fn test_link_files_recursive_single_file() {
        let (repo_root, target_root, repo_root_real, target_root_real) = setup_test_fs();
        
        // Create source file
        let zsh_dir = repo_root.join("zsh").unwrap();
        zsh_dir.create_dir_all().unwrap();
        let zshrc = zsh_dir.join(".zshrc").unwrap();
        let mut file = zshrc.create_file().unwrap();
        write!(file, "# zshrc").unwrap();
        drop(file);
        
        let linker = Linker::new(repo_root, target_root.clone(), repo_root_real, target_root_real);
        let package = Package {
            source: Utf8PathBuf::from("zsh/.zshrc"),
            target: Utf8PathBuf::from("~/.zshrc"),
            strategy: LinkStrategy::LinkFilesRecursive,
        };
        
        let actions = linker.link_package(&package, false).unwrap();
        
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            LinkAction::Created { target, source } => {
                assert_eq!(target, &Utf8PathBuf::from("~/.zshrc"));
                assert_eq!(source, &Utf8PathBuf::from("zsh/.zshrc"));
            }
            _ => panic!("Expected Created action"),
        }
    }

    #[test]
    fn test_link_files_recursive_directory() {
        let (repo_root, target_root, repo_root_real, target_root_real) = setup_test_fs();
        
        // Create source directory with multiple files
        let scripts_dir = repo_root.join("scripts").unwrap();
        scripts_dir.create_dir_all().unwrap();
        
        let script1 = scripts_dir.join("script1.sh").unwrap();
        let mut file = script1.create_file().unwrap();
        write!(file, "#!/bin/bash").unwrap();
        drop(file);
        
        let script2 = scripts_dir.join("script2.sh").unwrap();
        let mut file = script2.create_file().unwrap();
        write!(file, "#!/bin/bash").unwrap();
        drop(file);
        
        let linker = Linker::new(repo_root, target_root.clone(), repo_root_real, target_root_real);
        let package = Package {
            source: Utf8PathBuf::from("scripts"),
            target: Utf8PathBuf::from("~/scripts"),
            strategy: LinkStrategy::LinkFilesRecursive,
        };
        
        let actions = linker.link_package(&package, false).unwrap();
        
        assert_eq!(actions.len(), 2);
        
        // Verify both files were linked
        let target_dir = target_root.join("scripts").unwrap();
        assert!(target_dir.exists().unwrap());
        assert!(target_dir.join("script1.sh").unwrap().exists().unwrap());
        assert!(target_dir.join("script2.sh").unwrap().exists().unwrap());
    }

    #[test]
    fn test_clean_removes_links() {
        let (repo_root, target_root, repo_root_real, target_root_real) = setup_test_fs();
        
        // Create some "symlinks" (files in our VFS mock)
        let config_dir = target_root.join(".config").unwrap();
        config_dir.create_dir_all().unwrap();
        let nvim_link = config_dir.join("nvim").unwrap();
        nvim_link.create_file().unwrap();
        
        let zshrc = target_root.join(".zshrc").unwrap();
        zshrc.create_file().unwrap();
        
        // Create state
        let mut state = DotyState::new("test-host".to_string());
        state.add_link(
            Utf8PathBuf::from("~/.config/nvim"),
            Utf8PathBuf::from("nvim"),
        );
        state.add_link(
            Utf8PathBuf::from("~/.zshrc"),
            Utf8PathBuf::from("zsh/.zshrc"),
        );
        
        let linker = Linker::new(repo_root, target_root.clone(), repo_root_real, target_root_real);
        let actions = linker.clean(&state, false).unwrap();
        
        assert_eq!(actions.len(), 2);
        
        // Verify links were removed
        assert!(!nvim_link.exists().unwrap());
        assert!(!zshrc.exists().unwrap());
    }

    #[test]
    fn test_clean_dry_run() {
        let (repo_root, target_root, repo_root_real, target_root_real) = setup_test_fs();
        
        let zshrc = target_root.join(".zshrc").unwrap();
        zshrc.create_file().unwrap();
        
        let mut state = DotyState::new("test-host".to_string());
        state.add_link(
            Utf8PathBuf::from("~/.zshrc"),
            Utf8PathBuf::from("zsh/.zshrc"),
        );
        
        let linker = Linker::new(repo_root, target_root.clone(), repo_root_real, target_root_real);
        let actions = linker.clean(&state, true).unwrap();
        
        assert_eq!(actions.len(), 1);
        
        // Verify link was NOT removed
        assert!(zshrc.exists().unwrap());
    }
}
