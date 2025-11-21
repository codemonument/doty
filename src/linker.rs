use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use std::fs;

use crate::config::{LinkStrategy, Package, PathResolution};
use crate::state::DotyState;

/// Represents the result of a linking operation
#[derive(Debug, Clone, PartialEq)]
pub enum LinkAction {
    /// A new symlink was created
    Created {
        target: Utf8PathBuf,
        source: Utf8PathBuf,
    },
    /// An existing symlink was updated
    Updated {
        target: Utf8PathBuf,
        old_source: Utf8PathBuf,
        new_source: Utf8PathBuf,
    },
    /// A symlink was skipped (already correct)
    Skipped {
        target: Utf8PathBuf,
        source: Utf8PathBuf,
    },
    /// A symlink was removed
    Removed {
        target: Utf8PathBuf,
        source: Utf8PathBuf,
    },
}

/// The Linker handles creating and managing symlinks
pub struct Linker {
    /// Root directory of the dotfiles repository (or cwd, depending on path_resolution)
    repo_root: Utf8PathBuf,
    /// Path resolution strategy
    path_resolution: PathResolution,
}

impl Linker {
    /// Create a new Linker
    pub fn new(repo_root: Utf8PathBuf, path_resolution: PathResolution) -> Self {
        Self {
            repo_root,
            path_resolution,
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
        let source_path = self.repo_root.join(&package.source);
        let target_path = self.resolve_target_path(&package.target)?;

        // Check if source exists
        if !source_path.exists() {
            anyhow::bail!("Source path does not exist: {}", source_path);
        }

        // Check if source is a directory
        if !source_path.is_dir() {
            anyhow::bail!("Source path is not a directory: {}", source_path);
        }

        let mut actions = Vec::new();

        // Check if target already exists
        if target_path.exists() {
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
                if target_path.is_dir() {
                    fs::remove_dir_all(&target_path)?;
                } else {
                    fs::remove_file(&target_path)?;
                }
            }
        }

        // Create parent directory if needed
        if let Some(parent) = target_path.parent() {
            if !parent.exists() && !dry_run {
                fs::create_dir_all(parent)?;
            }
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
        let source_path = self.repo_root.join(&package.source);
        let target_path = self.resolve_target_path(&package.target)?;

        // Check if source exists
        if !source_path.exists() {
            anyhow::bail!("Source path does not exist: {}", source_path);
        }

        let mut actions = Vec::new();

        // If source is a file, just link it directly
        if source_path.is_file() {
            if target_path.exists() && self.is_symlink_to(&target_path, &source_path)? {
                actions.push(LinkAction::Skipped {
                    target: package.target.clone(),
                    source: package.source.clone(),
                });
            } else {
                if !dry_run {
                    if let Some(parent) = target_path.parent() {
                        if !parent.exists() {
                            fs::create_dir_all(parent)?;
                        }
                    }
                    if target_path.exists() {
                        fs::remove_file(&target_path)?;
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
        self.link_directory_recursive(
            &source_path,
            &target_path,
            &package.source,
            &package.target,
            dry_run,
            &mut actions,
        )?;

        Ok(actions)
    }

    /// Recursively link all files in a directory
    fn link_directory_recursive(
        &self,
        source_dir: &Utf8Path,
        target_dir: &Utf8Path,
        source_rel: &Utf8Path,
        target_rel: &Utf8Path,
        dry_run: bool,
        actions: &mut Vec<LinkAction>,
    ) -> Result<()> {
        // Create target directory if it doesn't exist
        if !target_dir.exists() && !dry_run {
            fs::create_dir_all(target_dir)?;
        }

        // Iterate through source directory
        for entry in fs::read_dir(source_dir)? {
            let entry = entry?;
            let entry_name = entry.file_name();
            let entry_name_str = entry_name.to_string_lossy();

            let source_entry = source_dir.join(entry_name_str.as_ref());
            let target_entry = target_dir.join(entry_name_str.as_ref());

            let source_entry_rel = source_rel.join(entry_name_str.as_ref());
            let target_entry_rel = target_rel.join(entry_name_str.as_ref());

            if source_entry.is_dir() {
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
                if target_entry.exists() && self.is_symlink_to(&target_entry, &source_entry)? {
                    actions.push(LinkAction::Skipped {
                        target: target_entry_rel.clone(),
                        source: source_entry_rel.clone(),
                    });
                } else {
                    if !dry_run {
                        if target_entry.exists() {
                            fs::remove_file(&target_entry)?;
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

            // Check if the symlink exists (using symlink_metadata to handle broken symlinks)
            if let Ok(metadata) = fs::symlink_metadata(&target_path) {
                if !dry_run {
                    if metadata.is_dir() {
                        fs::remove_dir_all(&target_path)?;
                    } else {
                        fs::remove_file(&target_path)?;
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

    /// Resolve a target path (handle ~ expansion, absolute paths, and relative paths)
    fn resolve_target_path(&self, target: &Utf8Path) -> Result<Utf8PathBuf> {
        let path_str = target.as_str();
        
        // Handle ~ expansion (relative to HOME)
        if let Some(stripped) = path_str.strip_prefix("~/") {
            let home_dir = std::env::var("HOME")
                .context("HOME environment variable not set")?;
            return Ok(Utf8PathBuf::from(home_dir).join(stripped));
        } else if path_str == "~" {
            let home_dir = std::env::var("HOME")
                .context("HOME environment variable not set")?;
            return Ok(Utf8PathBuf::from(home_dir));
        }
        
        // Handle absolute paths
        if target.is_absolute() {
            return Ok(target.to_path_buf());
        }
        
        // Handle relative paths based on path resolution strategy
        match self.path_resolution {
            PathResolution::Config => {
                // Resolve relative to repo_root (which is the config file's directory)
                Ok(self.repo_root.join(target))
            }
            PathResolution::Cwd => {
                // Resolve relative to current working directory
                let cwd = std::env::current_dir()
                    .context("Failed to get current working directory")?;
                let absolute_path = cwd.join(target.as_std_path());
                Utf8PathBuf::from_path_buf(absolute_path)
                    .map_err(|_| anyhow::anyhow!("Failed to convert path to UTF-8"))
            }
        }
    }

    /// Check if a path is a symlink pointing to the expected target
    fn is_symlink_to(&self, link_path: &Utf8Path, expected_target: &Utf8Path) -> Result<bool> {
        // Check if it's a symlink using std::fs::symlink_metadata
        if let Ok(metadata) = fs::symlink_metadata(link_path) {
            if metadata.is_symlink() {
                // Read the symlink target
                if let Ok(target) = fs::read_link(link_path) {
                    // Compare with expected target
                    if let Ok(target_canonical) = target.canonicalize() {
                        if let Ok(expected_canonical) = expected_target.as_std_path().canonicalize()
                        {
                            return Ok(target_canonical == expected_canonical);
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    /// Create a symlink
    fn create_symlink(&self, source: &Utf8Path, target: &Utf8Path) -> Result<()> {
        // Convert source to absolute path to avoid broken symlinks
        let absolute_source = if source.is_absolute() {
            source.to_path_buf()
        } else {
            // Make source relative to current working directory
            let cwd = std::env::current_dir()
                .map_err(|e| anyhow::anyhow!("Failed to get current directory: {}", e))?;
            let absolute_path = cwd.join(source.as_std_path());
            Utf8PathBuf::from_path_buf(absolute_path)
                .map_err(|_| anyhow::anyhow!("Failed to convert path to UTF-8"))?
        };

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&absolute_source, target).with_context(|| {
                format!(
                    "Failed to create symlink: {} -> {}",
                    target, absolute_source
                )
            })?;
        }

        #[cfg(windows)]
        {
            // On Windows, we need to check if source is a file or directory
            if absolute_source.is_dir() {
                std::os::windows::fs::symlink_dir(&absolute_source, target).with_context(|| {
                    format!(
                        "Failed to create directory symlink: {} -> {}",
                        target, absolute_source
                    )
                })?;
            } else {
                std::os::windows::fs::symlink_file(&absolute_source, target).with_context(
                    || {
                        format!(
                            "Failed to create file symlink: {} -> {}",
                            target, absolute_source
                        )
                    },
                )?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LinkStrategy, PathResolution};
    use std::fs;

    fn setup_test_fs(test_name: &str) -> Utf8PathBuf {
        let test_dir = format!("tests/tmpfs/{}", test_name);
        let _ = fs::remove_dir_all(&test_dir); // Clean up any existing test dir
        
        let repo_root = format!("{}/repo", test_dir);
        
        fs::create_dir_all(&repo_root).unwrap();
        
        // Convert to absolute path
        let cwd = std::env::current_dir().unwrap();
        let absolute_repo_root = cwd.join(&repo_root);
        Utf8PathBuf::from_path_buf(absolute_repo_root).unwrap()
    }

    #[test]
    fn test_link_folder_creates_symlink() {
        let repo_root = setup_test_fs("test_link_folder_creates_symlink");
        
        // Create source directory with a file
        let nvim_dir = repo_root.join("nvim");
        fs::create_dir_all(&nvim_dir).unwrap();
        fs::write(nvim_dir.join("init.lua"), "-- config").unwrap();
        
        // Create target directory for testing (relative path)
        let target_dir = repo_root.parent().unwrap().join("target");
        fs::create_dir_all(&target_dir).unwrap();
        
        let linker = Linker::new(repo_root.clone(), PathResolution::Config);
        let package = Package {
            source: Utf8PathBuf::from("nvim"),
            target: target_dir.join(".config/nvim"),
            strategy: LinkStrategy::LinkFolder,
        };

        let actions = linker.link_package(&package, false).unwrap();

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            LinkAction::Created { target, source } => {
                assert_eq!(target, &target_dir.join(".config/nvim"));
                assert_eq!(source, &Utf8PathBuf::from("nvim"));
            }
            _ => panic!("Expected Created action"),
        }

        // Verify target was created and is a symlink
        let target_path = target_dir.join(".config/nvim");
        assert!(target_path.exists());
        assert!(target_path.is_symlink());

        // Clean up
        let _ = fs::remove_dir_all(format!("tests/tmpfs/test_link_folder_creates_symlink"));
    }

    #[test]
    fn test_link_folder_dry_run() {
        let repo_root = setup_test_fs("test_link_folder_dry_run");
        
        let nvim_dir = repo_root.join("nvim");
        fs::create_dir_all(&nvim_dir).unwrap();
        
        // Create target directory for testing (relative path)
        let target_dir = repo_root.parent().unwrap().join("target");
        fs::create_dir_all(&target_dir).unwrap();
        
        let linker = Linker::new(repo_root.clone(), PathResolution::Config);
        let package = Package {
            source: Utf8PathBuf::from("nvim"),
            target: target_dir.join(".config/nvim"),
            strategy: LinkStrategy::LinkFolder,
        };

        let actions = linker.link_package(&package, true).unwrap();

        assert_eq!(actions.len(), 1);

        // Verify target was NOT created
        let target_path = target_dir.join(".config/nvim");
        assert!(!target_path.exists());

        // Clean up
        let _ = fs::remove_dir_all(format!("tests/tmpfs/test_link_folder_dry_run"));
    }

    #[test]
    fn test_link_files_recursive_single_file() {
        let repo_root = setup_test_fs("test_link_files_recursive_single_file");
        
        // Create source file
        let zsh_dir = repo_root.join("zsh");
        fs::create_dir_all(&zsh_dir).unwrap();
        fs::write(zsh_dir.join(".zshrc"), "# zshrc").unwrap();
        
        // Create target directory for testing (relative path)
        let target_dir = repo_root.parent().unwrap().join("target");
        fs::create_dir_all(&target_dir).unwrap();
        
        let linker = Linker::new(repo_root.clone(), PathResolution::Config);
        let package = Package {
            source: Utf8PathBuf::from("zsh/.zshrc"),
            target: target_dir.join(".zshrc"),
            strategy: LinkStrategy::LinkFilesRecursive,
        };

        let actions = linker.link_package(&package, false).unwrap();

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            LinkAction::Created { target, source } => {
                assert_eq!(target, &target_dir.join(".zshrc"));
                assert_eq!(source, &Utf8PathBuf::from("zsh/.zshrc"));
            }
            _ => panic!("Expected Created action"),
        }

        // Verify target was created and is a symlink
        let target_path = target_dir.join(".zshrc");
        assert!(target_path.exists());
        assert!(target_path.is_symlink());

        // Clean up
        let _ = fs::remove_dir_all(format!("tests/tmpfs/test_link_files_recursive_single_file"));
    }

    #[test]
    fn test_link_files_recursive_directory() {
        let repo_root = setup_test_fs("test_link_files_recursive_directory");
        
        // Create source directory with multiple files
        let scripts_dir = repo_root.join("scripts");
        fs::create_dir_all(&scripts_dir).unwrap();
        fs::write(scripts_dir.join("script1.sh"), "#!/bin/bash").unwrap();
        fs::write(scripts_dir.join("script2.sh"), "#!/bin/bash").unwrap();
        
        // Create target directory for testing (relative path)
        let target_dir = repo_root.parent().unwrap().join("target");
        fs::create_dir_all(&target_dir).unwrap();
        
        let linker = Linker::new(repo_root.clone(), PathResolution::Config);
        let package = Package {
            source: Utf8PathBuf::from("scripts"),
            target: target_dir.join("scripts"),
            strategy: LinkStrategy::LinkFilesRecursive,
        };

        let actions = linker.link_package(&package, false).unwrap();

        assert_eq!(actions.len(), 2);

        // Verify both files were linked
        let target_scripts_dir = target_dir.join("scripts");
        assert!(target_scripts_dir.exists());
        assert!(target_scripts_dir.join("script1.sh").exists());
        assert!(target_scripts_dir.join("script1.sh").is_symlink());
        assert!(target_scripts_dir.join("script2.sh").exists());
        assert!(target_scripts_dir.join("script2.sh").is_symlink());

        // Clean up
        let _ = fs::remove_dir_all(format!("tests/tmpfs/test_link_files_recursive_directory"));
    }

    #[test]
    fn test_clean_removes_links() {
        let repo_root = setup_test_fs("test_clean_removes_links");

        // Create target directory for testing
        let target_dir = repo_root.parent().unwrap().join("target");
        fs::create_dir_all(&target_dir).unwrap();

        // Create some symlinks
        let config_dir = target_dir.join(".config");
        fs::create_dir_all(&config_dir).unwrap();
        let nvim_link = config_dir.join("nvim");
        let source_path = repo_root.join("nvim");
        fs::create_dir_all(&source_path).unwrap();

        // Create actual symlinks
        #[cfg(unix)]
        std::os::unix::fs::symlink(&source_path, &nvim_link).unwrap();
        #[cfg(windows)]
        if source_path.is_dir() {
            std::os::windows::fs::symlink_dir(&source_path, &nvim_link).unwrap();
        }

        let zshrc = target_dir.join(".zshrc");
        let zsh_source = repo_root.join("zsh/.zshrc");
        fs::create_dir_all(zsh_source.parent().unwrap()).unwrap();
        fs::write(&zsh_source, "# zshrc").unwrap();

        #[cfg(unix)]
        std::os::unix::fs::symlink(&zsh_source, &zshrc).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&zsh_source, &zshrc).unwrap();

        // Create state with absolute paths
        let mut state = DotyState::new("test-host".to_string());
        state.add_link(nvim_link.clone(), Utf8PathBuf::from("nvim"));
        state.add_link(zshrc.clone(), Utf8PathBuf::from("zsh/.zshrc"));

        let linker = Linker::new(repo_root.clone(), PathResolution::Config);
        let actions = linker.clean(&state, false).unwrap();

        assert_eq!(actions.len(), 2);

        // Verify links were removed
        assert!(!nvim_link.exists());
        assert!(!zshrc.exists());

        // Clean up
        let _ = fs::remove_dir_all(format!("tests/tmpfs/test_clean_removes_links"));
    }

    #[test]
    fn test_clean_dry_run() {
        let repo_root = setup_test_fs("test_clean_dry_run");

        // Create target directory for testing
        let target_dir = repo_root.parent().unwrap().join("target");
        fs::create_dir_all(&target_dir).unwrap();

        let zshrc = target_dir.join(".zshrc");
        fs::write(&zshrc, "# zshrc").unwrap();

        let mut state = DotyState::new("test-host".to_string());
        state.add_link(zshrc.clone(), Utf8PathBuf::from("zsh/.zshrc"));

        let linker = Linker::new(repo_root.clone(), PathResolution::Config);
        let actions = linker.clean(&state, true).unwrap();

        assert_eq!(actions.len(), 1);

        // Verify link was NOT removed
        assert!(zshrc.exists());

        // Clean up
        let _ = fs::remove_dir_all(format!("tests/tmpfs/test_clean_dry_run"));
    }
}
