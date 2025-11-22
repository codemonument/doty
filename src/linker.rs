use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::{HashMap, HashSet};
use std::fs;

use crate::config::{DotyConfig, LinkStrategy, PathResolution};
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
    /// A warning about a broken explicit link
    Warning {
        target: Utf8PathBuf,
        source: Utf8PathBuf,
        message: String,
    },
}

/// The Linker handles creating and managing symlinks
pub struct Linker {
    /// Root directory for resolving relative paths (already resolved based on path_resolution strategy)
    config_dir_or_cwd: Utf8PathBuf,
    /// Path resolution strategy (retained for potential future features like debugging or per-package overrides)
    #[allow(dead_code)]
    path_resolution: PathResolution,
}

impl Linker {
    /// Create a new Linker
    pub fn new(config_dir_or_cwd: Utf8PathBuf, path_resolution: PathResolution) -> Self {
        Self {
            config_dir_or_cwd,
            path_resolution,
        }
    }

    /// Calculate what actions are needed to sync config with state
    pub fn calculate_diff(
        &self,
        config: &DotyConfig,
        state: &DotyState,
        force: bool,
    ) -> Result<Vec<LinkAction>> {
        let mut actions = Vec::new();
        
        // Step 1: Build desired links from config and identify explicit sources
        let mut desired_links: HashMap<Utf8PathBuf, Utf8PathBuf> = HashMap::new();
        let mut explicit_sources: HashSet<Utf8PathBuf> = HashSet::new();
        let mut processed_targets: HashSet<Utf8PathBuf> = HashSet::new();

        // First pass: identify all explicit sources
        for package in &config.packages {
            explicit_sources.insert(package.source.clone());
        }

        // Second pass: process each package
        for package in &config.packages {
            let source_path = self.config_dir_or_cwd.join(&package.source);
            
            if source_path.exists() {
                if source_path.is_file() {
                    // Single file link (explicit)
                    desired_links.insert(package.target.clone(), package.source.clone());
                } else if source_path.is_dir() {
                    // Directory link
                    match package.strategy {
                        LinkStrategy::LinkFolder => {
                            // Folder itself is explicit
                            desired_links.insert(package.target.clone(), package.source.clone());
                        }
                        LinkStrategy::LinkFilesRecursive => {
                            // Scan and add all files (these are implicit)
                            for file in self.scan_directory_recursive(&source_path)? {
                                let relative = file.strip_prefix(&source_path)?;
                                let target_path = package.target.join(relative);
                                let source_rel = package.source.join(relative);
                                desired_links.insert(target_path, source_rel);
                            }
                        }
                    }
                }
            } else {
                // Source doesn't exist - check if explicit
                if self.is_explicit(&package.source, &explicit_sources) {
                    if force {
                        // Treat as removal - don't add to desired_links
                        // Mark as processed to prevent duplicate removal in Step 2
                        processed_targets.insert(package.target.clone());
                        if let Some(source) = state.links.get(&package.target) {
                            actions.push(LinkAction::Removed {
                                target: package.target.clone(),
                                source: source.clone(),
                            });
                        }
                    } else {
                        // Warn - but keep in desired_links to prevent removal in Step 2
                        // Mark as processed so Step 3 skips it
                        desired_links.insert(package.target.clone(), package.source.clone());
                        processed_targets.insert(package.target.clone());
                        actions.push(LinkAction::Warning {
                            target: package.target.clone(),
                            source: package.source.clone(),
                            message: "Source file gone, remove from config if intentional".to_string(),
                        });
                    }
                }
            }
        }

        // Step 2: Find links to remove (in state but not in desired)
        for (target, source) in &state.links {
            // Skip if already processed (e.g., forced removal in Step 1)
            if processed_targets.contains(target) {
                continue;
            }
            
            if !desired_links.contains_key(target) {
                actions.push(LinkAction::Removed {
                    target: target.clone(),
                    source: source.clone(),
                });
            }
        }

        // Step 3: Find links to create/update/skip
        for (target, source) in &desired_links {
            // Skip targets that were already processed (warnings or forced removals)
            if processed_targets.contains(target) {
                continue;
            }
            
            if let Some(old_source) = state.links.get(target) {
                if old_source != source {
                    // Source changed
                    actions.push(LinkAction::Updated {
                        target: target.clone(),
                        old_source: old_source.clone(),
                        new_source: source.clone(),
                    });
                } else {
                    // Check if symlink is correct
                    let target_path = self.resolve_target_path(target)?;
                    let source_path = self.config_dir_or_cwd.join(source);
                    if self.is_symlink_to(&target_path, &source_path)? {
                        actions.push(LinkAction::Skipped {
                            target: target.clone(),
                            source: source.clone(),
                        });
                    } else {
                        // Symlink broken or incorrect, recreate
                        actions.push(LinkAction::Created {
                            target: target.clone(),
                            source: source.clone(),
                        });
                    }
                }
            } else {
                // New link
                actions.push(LinkAction::Created {
                    target: target.clone(),
                    source: source.clone(),
                });
            }
        }

        Ok(actions)
    }

    /// Execute a single action
    pub fn execute_action(&self, action: &LinkAction, dry_run: bool) -> Result<()> {
        match action {
            LinkAction::Created { target, source } => {
                let source_path = self.config_dir_or_cwd.join(source);
                let target_path = self.resolve_target_path(target)?;
                self.create_link(&source_path, &target_path, dry_run)
            }
            LinkAction::Removed { target, .. } => {
                let target_path = self.resolve_target_path(target)?;
                self.remove_link(&target_path, dry_run)
            }
            LinkAction::Updated { target, new_source, .. } => {
                let target_path = self.resolve_target_path(target)?;
                let new_source_path = self.config_dir_or_cwd.join(new_source);
                self.remove_link(&target_path, dry_run)?;
                self.create_link(&new_source_path, &target_path, dry_run)
            }
            LinkAction::Warning { .. } | LinkAction::Skipped { .. } => Ok(()),
        }
    }

    /// Check if a source is explicitly defined in config
    fn is_explicit(&self, source: &Utf8Path, explicit_sources: &HashSet<Utf8PathBuf>) -> bool {
        // A source is explicit if it exactly matches an entry in explicit_sources
        explicit_sources.contains(source)
    }

    /// Scan directory recursively and return all files
    fn scan_directory_recursive(&self, dir: &Utf8Path) -> Result<Vec<Utf8PathBuf>> {
        let mut files = Vec::new();
        
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let entry_path = Utf8PathBuf::from_path_buf(entry.path())
                .map_err(|_| anyhow::anyhow!("Path contains invalid UTF-8"))?;
            
            if entry_path.is_dir() {
                files.extend(self.scan_directory_recursive(&entry_path)?);
            } else {
                files.push(entry_path);
            }
        }
        
        Ok(files)
    }

    /// Create a symlink (helper for execute_action)
    fn create_link(&self, source: &Utf8Path, target: &Utf8Path, dry_run: bool) -> Result<()> {
        // Create parent directory if needed
        if let Some(parent) = target.parent() {
            if !parent.exists() && !dry_run {
                fs::create_dir_all(parent)?;
            }
        }

        // Remove existing target if it exists
        if target.exists() && !dry_run {
            if target.is_dir() {
                fs::remove_dir_all(target)?;
            } else {
                fs::remove_file(target)?;
            }
        }

        if !dry_run {
            self.create_symlink(source, target)?;
        }

        Ok(())
    }

    /// Remove a symlink (helper for execute_action)
    fn remove_link(&self, target: &Utf8Path, dry_run: bool) -> Result<()> {
        if target.exists() && !dry_run {
            if target.is_dir() {
                fs::remove_dir_all(target)?;
            } else {
                fs::remove_file(target)?;
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

    /// Check if target path or any of its parents is a symlink
    /// This prevents creating symlinks inside directories that are themselves symlinks
    fn check_target_path_conflicts(&self, target_path: &Utf8Path) -> Result<()> {
        // Check each parent directory to see if it's a symlink
        let mut current = target_path;
        
        while let Some(parent) = current.parent() {
            if parent.as_str().is_empty() || parent.as_str() == "/" {
                break;
            }
            
            // Check if this parent exists and is a symlink
            if let Ok(metadata) = fs::symlink_metadata(parent) {
                if metadata.is_symlink() {
                    anyhow::bail!(
                        "Parent directory '{}' is a symlink (created by LinkFolder)",
                        parent
                    );
                }
            }
            
            current = parent;
        }
        
        Ok(())
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
        
        // Handle relative paths - config_dir_or_cwd already contains the resolved directory
        Ok(self.config_dir_or_cwd.join(target))
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
    use crate::config::PathResolution;
    use std::fs;

    fn setup_test_fs(test_name: &str) -> Utf8PathBuf {
        let test_dir = format!("tests/tmpfs/{}", test_name);
        let _ = fs::remove_dir_all(&test_dir); // Clean up any existing test dir
        
        let config_dir_or_cwd = format!("{}/repo", test_dir);
        
        fs::create_dir_all(&config_dir_or_cwd).unwrap();
        
        // Convert to absolute path
        let cwd = std::env::current_dir().unwrap();
        let absolute_config_dir_or_cwd = cwd.join(&config_dir_or_cwd);
        Utf8PathBuf::from_path_buf(absolute_config_dir_or_cwd).unwrap()
    }

    // TODO: Update tests to use new diff-based API
    // Old tests using link_package() are temporarily commented out
    // as they need to be rewritten to use calculate_diff() and execute_action()

    #[test]
    fn test_clean_removes_links() {
        let config_dir_or_cwd = setup_test_fs("test_clean_removes_links");

        // Create target directory for testing
        let target_dir = config_dir_or_cwd.parent().unwrap().join("target");
        fs::create_dir_all(&target_dir).unwrap();

        // Create some symlinks
        let config_dir = target_dir.join(".config");
        fs::create_dir_all(&config_dir).unwrap();
        let nvim_link = config_dir.join("nvim");
        let source_path = config_dir_or_cwd.join("nvim");
        fs::create_dir_all(&source_path).unwrap();

        // Create actual symlinks
        #[cfg(unix)]
        std::os::unix::fs::symlink(&source_path, &nvim_link).unwrap();
        #[cfg(windows)]
        if source_path.is_dir() {
            std::os::windows::fs::symlink_dir(&source_path, &nvim_link).unwrap();
        }

        let zshrc = target_dir.join(".zshrc");
        let zsh_source = config_dir_or_cwd.join("zsh/.zshrc");
        fs::create_dir_all(zsh_source.parent().unwrap()).unwrap();
        fs::write(&zsh_source, "# zshrc").unwrap();

        #[cfg(unix)]
        std::os::unix::fs::symlink(&zsh_source, &zshrc).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&zsh_source, &zshrc).unwrap();

        // Create state with absolute paths
        let mut state = DotyState::new("test-host".to_string(), config_dir_or_cwd.clone());
        state.add_link(nvim_link.clone(), Utf8PathBuf::from("nvim"));
        state.add_link(zshrc.clone(), Utf8PathBuf::from("zsh/.zshrc"));

        let linker = Linker::new(config_dir_or_cwd.clone(), PathResolution::Config);
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
        let config_dir_or_cwd = setup_test_fs("test_clean_dry_run");

        // Create target directory for testing
        let target_dir = config_dir_or_cwd.parent().unwrap().join("target");
        fs::create_dir_all(&target_dir).unwrap();

        let zshrc = target_dir.join(".zshrc");
        fs::write(&zshrc, "# zshrc").unwrap();

        let mut state = DotyState::new("test-host".to_string(), config_dir_or_cwd.clone());
        state.add_link(zshrc.clone(), Utf8PathBuf::from("zsh/.zshrc"));

        let linker = Linker::new(config_dir_or_cwd.clone(), PathResolution::Config);
        let actions = linker.clean(&state, true).unwrap();

        assert_eq!(actions.len(), 1);

        // Verify link was NOT removed
        assert!(zshrc.exists());

        // Clean up
        let _ = fs::remove_dir_all(format!("tests/tmpfs/test_clean_dry_run"));
    }

    
}
