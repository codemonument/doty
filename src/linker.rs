use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::HashMap;
use std::fs;

use crate::config::{DotyConfig, LinkStrategy, Package, PathResolution};
use crate::fs_utils::{
    get_fs_type, read_symlink_target, resolve_target_path, scan_directory_recursive, FsType,
};
use crate::lockfile::Lockfile;

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
    /// A broken symlink was pruned (source missing, dangling link removal)
    Pruned {
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

impl LinkStatus {
    fn from_config(
        target: Utf8PathBuf,
        source: Utf8PathBuf,
        is_explicit: bool,
        source_exists: bool,
    ) -> Self {
        Self {
            config_resolved_source: Some(source),
            config_resolved_target: Some(target),
            config_is_explicit: is_explicit,
            state_resolved_source: None,
            state_resolved_target: None,
            source_exists,
            target_exists: false,
            target_type: None,
            target_points_to: None,
        }
    }

    fn from_lockfile(target: Utf8PathBuf, source: Utf8PathBuf) -> Self {
        Self {
            config_resolved_source: None,
            config_resolved_target: None,
            config_is_explicit: false,
            state_resolved_source: Some(source),
            state_resolved_target: Some(target),
            source_exists: false,
            target_exists: false,
            target_type: None,
            target_points_to: None,
        }
    }

    fn merge(&mut self, other: LinkStatus) {
        if other.config_resolved_source.is_some() {
            self.config_resolved_source = other.config_resolved_source;
            self.config_resolved_target = other.config_resolved_target;
            self.config_is_explicit = other.config_is_explicit;
            self.source_exists = other.source_exists;
        }
        if other.state_resolved_source.is_some() {
            self.state_resolved_source = other.state_resolved_source;
            self.state_resolved_target = other.state_resolved_target;
        }
    }
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

    /// Calculate what actions are needed to sync config with lockfile
    pub fn calculate_diff(
        &self,
        config: &DotyConfig,
        lockfile: &Lockfile,
        force: bool,
    ) -> Result<Vec<LinkAction>> {
        let link_states = self.gather_link_states(config, lockfile)?;

        // Determine actions based on gathered statuses
        Ok({
            let this = &self;
            let mut actions = Vec::new();
            for status in link_states.values() {
                actions.extend(this.determine_action_for_status(status, force));
            }
            actions
        })
    }

    /// Gather information about all relevant targets from Config, Lockfile, and Filesystem
    fn gather_link_states(
        &self,
        config: &DotyConfig,
        lockfile: &Lockfile,
    ) -> Result<HashMap<Utf8PathBuf, LinkStatus>> {
        // 1. Stream Config Statuses
        let config_stream = config
            .packages
            .iter()
            .flat_map(|pkg| self.expand_package(pkg));

        // 2. Stream Lockfile Statuses
        let lockfile_stream = lockfile
            .links
            .iter()
            .map(|(target, source)| self.create_link_status_from_lockfile(target, source));

        // 3. Fold into Map
        let mut map: HashMap<Utf8PathBuf, LinkStatus> = HashMap::new();
        for (target, status) in config_stream.chain(lockfile_stream) {
            map.entry(target)
                .and_modify(|e| e.merge(status.clone()))
                .or_insert(status);
        }

        // 4. Enrich (Side Effects)
        for status in map.values_mut() {
            self.enrich_status(status)?;
        }

        Ok(map)
    }

    /// Expand a package into a stream of LinkStatuses
    fn expand_package(&self, package: &Package) -> Vec<(Utf8PathBuf, LinkStatus)> {
        let source_path = self.config_dir_or_cwd.join(&package.source);
        let mut results = Vec::new();

        // Resolve target to absolute path for use as HashMap key (lockfile uses absolute paths)
        let resolved_target = resolve_target_path(&package.target, &self.config_dir_or_cwd)
            .unwrap_or_else(|_| self.config_dir_or_cwd.join(&package.target));

        if !source_path.exists() {
            // Explicit missing source
            results.push((
                resolved_target.clone(),
                LinkStatus::from_config(
                    package.target.clone(),
                    package.source.clone(),
                    true,  // explicit
                    false, // !exists
                ),
            ));
            return results;
        }

        if source_path.is_file() {
            results.push((
                resolved_target.clone(),
                LinkStatus::from_config(
                    package.target.clone(),
                    package.source.clone(),
                    true, // explicit
                    true, // exists
                ),
            ));
        } else if source_path.is_dir() {
            match package.strategy {
                LinkStrategy::LinkFolder => {
                    results.push((
                        resolved_target.clone(),
                        LinkStatus::from_config(
                            package.target.clone(),
                            package.source.clone(),
                            true, // explicit
                            true, // exists
                        ),
                    ));
                }
                LinkStrategy::LinkFilesRecursive => {
                    if let Ok(files) = scan_directory_recursive(&source_path) {
                        for file in files {
                            if let Ok(relative) = file.strip_prefix(&source_path) {
                                let target_path = package.target.join(relative);
                                let source_rel = package.source.join(relative);
                                let resolved_target_path =
                                    resolve_target_path(&target_path, &self.config_dir_or_cwd)
                                        .unwrap_or_else(|_| {
                                            self.config_dir_or_cwd.join(&target_path)
                                        });
                                results.push((
                                    resolved_target_path,
                                    LinkStatus::from_config(
                                        target_path,
                                        source_rel,
                                        false, // implicit
                                        true,  // exists
                                    ),
                                ));
                            }
                        }
                    }
                }
            }
        }
        results
    }

    /// Create a LinkStatus from lockfile entry
    fn create_link_status_from_lockfile(
        &self,
        target: &Utf8PathBuf,
        source: &Utf8PathBuf,
    ) -> (Utf8PathBuf, LinkStatus) {
        (
            target.clone(),
            LinkStatus::from_lockfile(target.clone(), source.clone()),
        )
    }

    /// Enrich status with filesystem reality
    fn enrich_status(&self, status: &mut LinkStatus) -> Result<()> {
        // Ensure config_resolved_target is set (it might be None if only in Lockfile)
        if status.config_resolved_target.is_none() {
            status.config_resolved_target = status.state_resolved_target.clone();
        }

        let target = status
            .config_resolved_target
            .as_ref()
            .expect("Target must exist");
        let target_path = resolve_target_path(target, &self.config_dir_or_cwd)?;

        if let Some(fs_type) = get_fs_type(&target_path)? {
            status.target_exists = true;
            status.target_type = Some(fs_type);

            if fs_type == FsType::Symlink {
                status.target_points_to = read_symlink_target(&target_path)?;
            }
        }
        Ok(())
    }

    /// Determine action(s) for a single status
    /// Returns a Vec to allow multiple actions (e.g., Warning + Pruned)
    fn determine_action_for_status(&self, status: &LinkStatus, force: bool) -> Vec<LinkAction> {
        let target = status
            .config_resolved_target
            .as_ref()
            .or(status.state_resolved_target.as_ref())
            .expect("Target must exist in either config or state");

        // Case 1: Link is in Lockfile but NOT in Config -> Remove it
        if status.config_resolved_source.is_none() {
            if let Some(stored) = &status.state_resolved_source {
                return vec![LinkAction::Removed {
                    target: target.clone(),
                    source: stored.clone(),
                }];
            }
            return vec![]; // Should not happen (neither config nor lockfile)
        }

        let desired_source = status.config_resolved_source.as_ref().unwrap();

        // Case 2: Source file does not exist
        if !status.source_exists {
            if !status.config_is_explicit {
                return vec![]; // Implicit missing sources are ignored
            }

            // Explicit source missing
            // Check if target is a broken symlink that needs cleanup
            // A broken symlink is detected when: target_type is Symlink but target_points_to is None
            // (target_exists is true if get_fs_type succeeded, which it does for broken symlinks)
            let is_broken_symlink =
                status.target_type == Some(FsType::Symlink) && status.target_points_to.is_none();

            if is_broken_symlink {
                // Broken symlink with missing source - return both Warning and Pruned
                // Use state_resolved_source if available (from lockfile), otherwise use desired_source (from config)
                let source = status
                    .state_resolved_source
                    .as_ref()
                    .unwrap_or(desired_source)
                    .clone();
                return vec![
                    LinkAction::Warning {
                        target: target.clone(),
                        source: desired_source.clone(),
                        message: "Source (file|dir) gone, remove from config if intentional"
                            .to_string(),
                    },
                    LinkAction::Pruned {
                        target: target.clone(),
                        source,
                    },
                ];
            } else if force && status.state_resolved_source.is_some() {
                // If forced and we tracked it before, remove it
                return vec![LinkAction::Removed {
                    target: target.clone(),
                    source: status.state_resolved_source.as_ref().unwrap().clone(),
                }];
            } else {
                // Otherwise warn
                return vec![LinkAction::Warning {
                    target: target.clone(),
                    source: desired_source.clone(),
                    message: "Source (file|dir) gone, remove from config if intentional"
                        .to_string(),
                }];
            }
        }

        // Case 3: Link is Configured (and source exists)

        // Subcase 3a: Not in Lockfile (New link)
        if status.state_resolved_source.is_none() {
            return vec![LinkAction::Created {
                target: target.clone(),
                source: desired_source.clone(),
            }];
        }

        let stored_source = status.state_resolved_source.as_ref().unwrap();

        // Subcase 3b: In Lockfile, but source path changed
        // Normalize desired_source to absolute for comparison (lockfile stores absolute paths)
        let desired_abs_source = self
            .config_dir_or_cwd
            .join(desired_source)
            .canonicalize()
            .map(|p| Utf8PathBuf::from_path_buf(p).unwrap_or_default())
            .unwrap_or_else(|_| self.config_dir_or_cwd.join(desired_source));

        if desired_abs_source != *stored_source {
            return vec![LinkAction::Updated {
                target: target.clone(),
                old_source: stored_source.clone(),
                new_source: desired_source.clone(),
            }];
        }

        // Subcase 3c: In Lockfile, source path same -> Check Reality
        // Calculate absolute desired path for comparison
        let desired_abs = self
            .config_dir_or_cwd
            .join(desired_source)
            .canonicalize()
            .map(|p| Utf8PathBuf::from_path_buf(p).unwrap_or_default())
            .unwrap_or_else(|_| self.config_dir_or_cwd.join(desired_source));

        let is_correct = if let Some(actual) = &status.target_points_to {
            *actual == desired_abs
        } else {
            false
        };

        if is_correct {
            return vec![LinkAction::Skipped {
                target: target.clone(),
                source: desired_source.clone(),
            }];
        } else {
            return vec![LinkAction::Created {
                target: target.clone(),
                source: desired_source.clone(),
            }];
        }
    }

    /// Execute a single action
    pub fn execute_action(&self, action: &LinkAction, dry_run: bool) -> Result<()> {
        match action {
            LinkAction::Created { target, source } => {
                let source_path = self.config_dir_or_cwd.join(source);
                let target_path = resolve_target_path(target, &self.config_dir_or_cwd)?;
                self.create_link(&source_path, &target_path, dry_run)
            }
            LinkAction::Removed { target, .. } => {
                let target_path = resolve_target_path(target, &self.config_dir_or_cwd)?;
                self.remove_link(&target_path, dry_run)
            }
            LinkAction::Pruned { target, .. } => {
                // Pruned actions remove broken symlinks (same as Removed)
                let target_path = resolve_target_path(target, &self.config_dir_or_cwd)?;
                self.remove_link(&target_path, dry_run)
            }
            LinkAction::Updated {
                target, new_source, ..
            } => {
                let target_path = resolve_target_path(target, &self.config_dir_or_cwd)?;
                let new_source_path = self.config_dir_or_cwd.join(new_source);
                self.remove_link(&target_path, dry_run)?;
                self.create_link(&new_source_path, &target_path, dry_run)
            }
            LinkAction::Warning { .. } | LinkAction::Skipped { .. } => Ok(()),
        }
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
        if !dry_run {
            // Use symlink_metadata to handle broken symlinks (exists() returns false for broken symlinks)
            if let Ok(metadata) = fs::symlink_metadata(target) {
                if metadata.is_dir() {
                    fs::remove_dir_all(target)?;
                } else {
                    fs::remove_file(target)?;
                }
            } else if target.exists() {
                // Fallback for non-symlink files/directories
                if target.is_dir() {
                    fs::remove_dir_all(target)?;
                } else {
                    fs::remove_file(target)?;
                }
            }
        }
        Ok(())
    }

    /// Remove all symlinks managed by Doty
    pub fn clean(&self, lockfile: &Lockfile, dry_run: bool) -> Result<Vec<LinkAction>> {
        let mut actions = Vec::new();

        for (target, source) in &lockfile.links {
            let target_path = resolve_target_path(target, &self.config_dir_or_cwd)?;

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

        // Create lockfile with absolute paths
        let mut lockfile = Lockfile::new("test-host".to_string(), config_dir_or_cwd.clone());
        lockfile.add_link(nvim_link.clone(), Utf8PathBuf::from("nvim"));
        lockfile.add_link(zshrc.clone(), Utf8PathBuf::from("zsh/.zshrc"));

        let linker = Linker::new(config_dir_or_cwd.clone(), PathResolution::Config);
        let actions = linker.clean(&lockfile, false).unwrap();

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

        let mut lockfile = Lockfile::new("test-host".to_string(), config_dir_or_cwd.clone());
        lockfile.add_link(zshrc.clone(), Utf8PathBuf::from("zsh/.zshrc"));

        let linker = Linker::new(config_dir_or_cwd.clone(), PathResolution::Config);
        let actions = linker.clean(&lockfile, true).unwrap();

        assert_eq!(actions.len(), 1);

        // Verify link was NOT removed
        assert!(zshrc.exists());

        // Clean up
        let _ = fs::remove_dir_all(format!("tests/tmpfs/test_clean_dry_run"));
    }
}
