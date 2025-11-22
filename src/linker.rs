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

#[derive(Debug, Clone, PartialEq)]
pub enum FsType {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone)]
struct LinkStatus {
    // Config (Desired state)
    config_resolved_source: Option<Utf8PathBuf>,
    config_resolved_target: Option<Utf8PathBuf>,
    config_is_explicit: bool,

    // State (Stored cache)
    state_resolved_source: Option<Utf8PathBuf>,
    state_resolved_target: Option<Utf8PathBuf>,

    // Filesystem (Reality)
    source_exists: bool,         //checked via config_resolved_source
    target_exists: bool,         //checked via target_points_to
    target_type: Option<FsType>, //checked via target_points_to
    target_points_to: Option<Utf8PathBuf>,
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
        let link_states = self.gather_link_states(config, state)?;
        Ok(self.determine_actions(link_states, force))
    }

    /// Gather information about all relevant targets from Config, State, and Filesystem
    fn gather_link_states(
        &self,
        config: &DotyConfig,
        state: &DotyState,
    ) -> Result<HashMap<Utf8PathBuf, LinkStatus>> {
        let mut link_states: HashMap<Utf8PathBuf, LinkStatus> = HashMap::new();
        let mut explicit_sources: HashSet<Utf8PathBuf> = HashSet::new();

        // 1. Identify explicit sources
        for package in &config.packages {
            explicit_sources.insert(package.source.clone());
        }

        // 2. Process Config (Desired State)
        for package in &config.packages {
            let source_path = self.config_dir_or_cwd.join(&package.source);
            let source_exists = source_path.exists();

            if source_exists {
                if source_path.is_file() {
                    self.add_config_status(
                        &mut link_states,
                        package.target.clone(),
                        package.source.clone(),
                        true, // explicit
                        true, // exists
                    )?;
                } else if source_path.is_dir() {
                    match package.strategy {
                        LinkStrategy::LinkFolder => {
                            self.add_config_status(
                                &mut link_states,
                                package.target.clone(),
                                package.source.clone(),
                                true, // explicit
                                true, // exists
                            )?;
                        }
                        LinkStrategy::LinkFilesRecursive => {
                            for file in self.scan_directory_recursive(&source_path)? {
                                let relative = file.strip_prefix(&source_path)?;
                                let target_path = package.target.join(relative);
                                let source_rel = package.source.join(relative);
                                self.add_config_status(
                                    &mut link_states,
                                    target_path,
                                    source_rel,
                                    false, // implicit
                                    true,  // exists
                                )?;
                            }
                        }
                    }
                }
            } else {
                // Source doesn't exist
                if self.is_explicit(&package.source, &explicit_sources) {
                    self.add_config_status(
                        &mut link_states,
                        package.target.clone(),
                        package.source.clone(),
                        true,  // explicit
                        false, // !exists
                    )?;
                }
            }
        }

        // 3. Process State (Stored State)
        for (target, source) in &state.links {
            let status = link_states
                .entry(target.clone())
                .or_insert_with(|| LinkStatus {
                    config_resolved_source: None,
                    config_resolved_target: None, // Will be filled in step 4 if not already
                    config_is_explicit: false,
                    state_resolved_source: None,
                    state_resolved_target: Some(target.clone()),
                    source_exists: false,
                    target_exists: false,
                    target_type: None,
                    target_points_to: None,
                });
            status.state_resolved_source = Some(source.clone());
            status.state_resolved_target = Some(target.clone());
        }

        // 4. Process Reality (Filesystem)
        for (target, status) in link_states.iter_mut() {
            // Ensure config_resolved_target is set (it might be None if only in State)
            if status.config_resolved_target.is_none() {
                status.config_resolved_target = Some(target.clone());
            }

            let target_path = self.resolve_target_path(target)?;

            if let Ok(metadata) = fs::symlink_metadata(&target_path) {
                status.target_exists = true;
                if metadata.is_symlink() {
                    status.target_type = Some(FsType::Symlink);
                    if let Ok(target) = fs::read_link(&target_path) {
                        if let Ok(canonical) = target.canonicalize() {
                            status.target_points_to =
                                Some(Utf8PathBuf::from_path_buf(canonical).unwrap_or_default());
                        }
                    }
                } else if metadata.is_dir() {
                    status.target_type = Some(FsType::Directory);
                } else {
                    status.target_type = Some(FsType::File);
                }
            }
        }

        Ok(link_states)
    }

    /// Helper to update status with desired info
    fn add_config_status(
        &self,
        link_states: &mut HashMap<Utf8PathBuf, LinkStatus>,
        target: Utf8PathBuf,
        source_rel: Utf8PathBuf,
        is_explicit: bool,
        source_exists: bool,
    ) -> Result<()> {
        let status = link_states
            .entry(target.clone())
            .or_insert_with(|| LinkStatus {
                config_resolved_source: None,
                config_resolved_target: None,
                config_is_explicit: false,
                state_resolved_source: None,
                state_resolved_target: None,
                source_exists: false,
                target_exists: false,
                target_type: None,
                target_points_to: None,
            });

        status.config_resolved_source = Some(source_rel);
        status.config_resolved_target = Some(target);
        status.config_is_explicit = is_explicit;
        status.source_exists = source_exists;
        Ok(())
    }

    /// Determine actions based on gathered status
    fn determine_actions(
        &self,
        statuses: HashMap<Utf8PathBuf, LinkStatus>,
        force: bool,
    ) -> Vec<LinkAction> {
        let mut actions = Vec::new();

        for status in statuses.values() {
            // We use the target from the map key (which is in config_resolved_target or state_resolved_target)
            let target = status
                .config_resolved_target
                .as_ref()
                .or(status.state_resolved_target.as_ref())
                .expect("Target must exist in either config or state");

            match (
                &status.config_resolved_source,
                &status.state_resolved_source,
            ) {
                // Case 1: Configured & Stored
                (Some(desired), Some(stored)) => {
                    if !status.source_exists {
                        if status.config_is_explicit {
                            if force {
                                actions.push(LinkAction::Removed {
                                    target: target.clone(),
                                    source: stored.clone(),
                                });
                            } else {
                                actions.push(LinkAction::Warning {
                                    target: target.clone(),
                                    source: desired.clone(),
                                    message: "Source file gone, remove from config if intentional"
                                        .to_string(),
                                });
                            }
                        }
                    } else if desired != stored {
                        actions.push(LinkAction::Updated {
                            target: target.clone(),
                            old_source: stored.clone(),
                            new_source: desired.clone(),
                        });
                    } else {
                        // Same source, check reality
                        // Calculate absolute desired path for comparison
                        let desired_abs = self
                            .config_dir_or_cwd
                            .join(desired)
                            .canonicalize()
                            .map(|p| Utf8PathBuf::from_path_buf(p).unwrap_or_default())
                            .unwrap_or_else(|_| self.config_dir_or_cwd.join(desired));

                        let is_correct = if let Some(actual) = &status.target_points_to {
                            *actual == desired_abs
                        } else {
                            false
                        };

                        if is_correct {
                            actions.push(LinkAction::Skipped {
                                target: target.clone(),
                                source: desired.clone(),
                            });
                        } else {
                            actions.push(LinkAction::Created {
                                target: target.clone(),
                                source: desired.clone(),
                            });
                        }
                    }
                }
                // Case 2: Configured Only (New)
                (Some(desired), None) => {
                    if status.source_exists {
                        actions.push(LinkAction::Created {
                            target: target.clone(),
                            source: desired.clone(),
                        });
                    } else if status.config_is_explicit {
                        actions.push(LinkAction::Warning {
                            target: target.clone(),
                            source: desired.clone(),
                            message: "Source file gone, remove from config if intentional"
                                .to_string(),
                        });
                    }
                }
                // Case 3: Stored Only (Removed)
                (None, Some(stored)) => {
                    actions.push(LinkAction::Removed {
                        target: target.clone(),
                        source: stored.clone(),
                    });
                }
                // Case 4: Neither (Impossible)
                (None, None) => {}
            }
        }

        actions
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
            LinkAction::Updated {
                target, new_source, ..
            } => {
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

    /// Resolve a target path (handle ~ expansion, absolute paths, and relative paths)
    fn resolve_target_path(&self, target: &Utf8Path) -> Result<Utf8PathBuf> {
        let path_str = target.as_str();

        // Handle ~ expansion (relative to HOME)
        if let Some(stripped) = path_str.strip_prefix("~/") {
            let home_dir = std::env::var("HOME").context("HOME environment variable not set")?;
            return Ok(Utf8PathBuf::from(home_dir).join(stripped));
        } else if path_str == "~" {
            let home_dir = std::env::var("HOME").context("HOME environment variable not set")?;
            return Ok(Utf8PathBuf::from(home_dir));
        }

        // Handle absolute paths
        if target.is_absolute() {
            return Ok(target.to_path_buf());
        }

        // Handle relative paths - config_dir_or_cwd already contains the resolved directory
        Ok(self.config_dir_or_cwd.join(target))
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
