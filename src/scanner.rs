use anyhow::{Context, Result};
use camino::Utf8PathBuf;

use crate::config::{DotyConfig, LinkStrategy, Package};
use crate::fs_utils::{scan_directory_recursive, get_fs_type, is_broken_symlink, resolve_target_path};
use crate::state::DotyState;

/// Types of drift detected between filesystem reality and Doty's knowledge
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriftType {
    /// File exists in target but not in source (LinkFilesRecursive only)
    Untracked,
    /// Symlink exists but points nowhere
    Broken,
    /// Target file modified (not a symlink anymore)
    Modified,
    /// In state but not in config (already handled by linker, included for completeness)
    Orphaned,
}

/// Represents a drift item detected during scanning
#[derive(Debug, Clone)]
pub struct DriftItem {
    pub target_path: Utf8PathBuf,
    pub drift_type: DriftType,
    pub package: Option<Package>,
    pub symlink_target: Option<Utf8PathBuf>,
}

/// Scanner for detecting drift between filesystem reality and Doty's knowledge
pub struct Scanner {
    config_dir_or_cwd: Utf8PathBuf,
}

impl Scanner {
    /// Create a new Scanner
    pub fn new(config_dir_or_cwd: Utf8PathBuf) -> Self {
        Self { config_dir_or_cwd }
    }

    /// Scan target directories and detect differences between filesystem reality and Doty's knowledge
    pub fn scan_targets(
        &self,
        config: &DotyConfig,
        state: &DotyState,
    ) -> Result<Vec<DriftItem>> {
        let mut drift_items = Vec::new();

        // Scan each package for drift
        for package in &config.packages {
            let package_drift = self.scan_package(package, config, state)?;
            drift_items.extend(package_drift);
        }

        // Check for broken symlinks from state that aren't already covered by package scanning
        for (state_target, _) in &state.links {
            // Resolve state target to absolute path
            let resolved_target = resolve_target_path(state_target, &self.config_dir_or_cwd)?;

            // Skip if this target is already covered by a package
            let is_covered_by_package = config.packages.iter().any(|pkg| {
                let pkg_target = resolve_target_path(&pkg.target, &self.config_dir_or_cwd).unwrap_or_default();
                resolved_target.starts_with(pkg_target)
            });
            
            if !is_covered_by_package {
                if let Some(fs_type) = get_fs_type(&resolved_target)? {
                    if fs_type == crate::fs_utils::FsType::Symlink {
                        if is_broken_symlink(&resolved_target)? {
                            let symlink_target = std::fs::read_link(&resolved_target)
                                .ok()
                                .and_then(|p| Utf8PathBuf::from_path_buf(p).ok());
                                
                            drift_items.push(DriftItem {
                                target_path: resolved_target,
                                drift_type: DriftType::Broken,
                                package: None, // We don't know which package this belongs to
                                symlink_target,
                            });
                        }
                    }
                }
            }
        }

        Ok(drift_items)
    }

    /// Scan a single package for drift
    fn scan_package(
        &self,
        package: &Package,
        _config: &DotyConfig,
        _state: &DotyState,
    ) -> Result<Vec<DriftItem>> {
        let mut drift_items = Vec::new();

        // Resolve source and target paths
        let source_path = self.config_dir_or_cwd.join(&package.source);
        let target_path = resolve_target_path(&package.target, &self.config_dir_or_cwd)?;

        match package.strategy {
            LinkStrategy::LinkFolder => {
                // Only check if the symlink itself is valid
                // No untracked file detection needed for LinkFolder
                if is_broken_symlink(&target_path)? {
                    let symlink_target = std::fs::read_link(&target_path)
                        .ok()
                        .and_then(|p| Utf8PathBuf::from_path_buf(p).ok());

                    drift_items.push(DriftItem {
                        target_path: target_path.clone(),
                        drift_type: DriftType::Broken,
                        package: Some(package.clone()),
                        symlink_target,
                    });
                }
            }
            LinkStrategy::LinkFilesRecursive => {
                // Only scan if source is a directory
                if source_path.is_dir() {
                    let _source_files = scan_directory_recursive(&source_path)?;
                    let target_files = scan_directory_recursive(&target_path)?;
                    
                    for target_file in target_files {
                        let relative_path = target_file.strip_prefix(&target_path)
                            .with_context(|| format!("Failed to get relative path for {}", target_file))?;
                        let corresponding_source = source_path.join(relative_path);
                        
                        if !corresponding_source.exists() {
                            // File in target but not in source = Untracked
                            drift_items.push(DriftItem {
                                target_path: target_file,
                                drift_type: DriftType::Untracked,
                                package: Some(package.clone()),
                                symlink_target: None,
                            });
                        }
                    }
                } else {
                    // For file sources, just check if the target is broken
                    if is_broken_symlink(&target_path)? {
                        let symlink_target = std::fs::read_link(&target_path)
                            .ok()
                            .and_then(|p| Utf8PathBuf::from_path_buf(p).ok());

                        drift_items.push(DriftItem {
                            target_path: target_path.clone(),
                            drift_type: DriftType::Broken,
                            package: Some(package.clone()),
                            symlink_target,
                        });
                    }
                }
            }
        }



        Ok(drift_items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DotyConfig, LinkStrategy, Package, PathResolution};
    use crate::state::DotyState;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_env() -> Result<(TempDir, Utf8PathBuf, DotyConfig, DotyState)> {
        let temp_dir = TempDir::new()?;
        let temp_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
            .map_err(|_| anyhow::anyhow!("Path contains invalid UTF-8"))?;

        // Create source and target directories
        let source_dir = temp_path.join("source");
        let target_dir = temp_path.join("target");
        fs::create_dir_all(&source_dir)?;
        fs::create_dir_all(&target_dir)?;

        // Create config with a LinkFilesRecursive package
        let config = DotyConfig {
            packages: vec![Package {
                source: "source/test-app".into(),
                target: "~/.config/test-app".into(),
                strategy: LinkStrategy::LinkFilesRecursive,
            }],
            path_resolution: PathResolution::Config,
        };

        // Create state
        let state = DotyState::new("test-host".to_string(), temp_path.clone());

        Ok((temp_dir, temp_path, config, state))
    }

    #[test]
    fn test_scan_link_files_recursive_untracked_files() -> Result<()> {
        let (_temp_dir, temp_path, mut config, state) = setup_test_env()?;

        // Create source directory with some files
        let source_dir = temp_path.join("source").join("test-app");
        fs::create_dir_all(&source_dir)?;
        fs::write(source_dir.join("config.txt"), "source config")?;
        fs::write(source_dir.join("settings.json"), "{}")?;

        // Create target directory with tracked files + untracked files
        let target_dir = temp_path.join("target").join(".config").join("test-app");
        fs::create_dir_all(&target_dir)?;
        fs::write(target_dir.join("config.txt"), "source config")?; // tracked
        fs::write(target_dir.join("user-custom.txt"), "custom")?; // untracked

        // Update package target to use our test target
        config.packages[0].target = target_dir.clone();

        let scanner = Scanner::new(temp_path.clone());
        let drift_items = scanner.scan_targets(&config, &state)?;

        // Should detect one untracked file
        assert_eq!(drift_items.len(), 1);
        assert_eq!(drift_items[0].drift_type, DriftType::Untracked);
        assert!(drift_items[0].target_path.ends_with("user-custom.txt"));

        Ok(())
    }

    #[test]
    fn test_scan_link_folder_no_untracked_detection() -> Result<()> {
        let (_temp_dir, temp_path, mut config, state) = setup_test_env()?;

        // Create source directory
        let source_dir = temp_path.join("source").join("test-app");
        fs::create_dir_all(&source_dir)?;
        fs::write(source_dir.join("config.txt"), "source config")?;

        // Create target directory with untracked files (shouldn't be detected)
        let target_dir = temp_path.join("target").join(".config").join("test-app");
        fs::create_dir_all(&target_dir)?;
        fs::write(target_dir.join("config.txt"), "source config")?;
        fs::write(target_dir.join("user-custom.txt"), "custom")?; // should not be detected as untracked

        // Update package to use LinkFolder strategy
        config.packages[0].strategy = LinkStrategy::LinkFolder;
        config.packages[0].target = target_dir.clone();

        let scanner = Scanner::new(temp_path.clone());
        let drift_items = scanner.scan_targets(&config, &state)?;

        // Should not detect untracked files for LinkFolder
        let untracked_count = drift_items.iter()
            .filter(|item| item.drift_type == DriftType::Untracked)
            .count();
        assert_eq!(untracked_count, 0);

        Ok(())
    }

    #[test]
    fn test_scan_broken_symlinks() -> Result<()> {
        let (_temp_dir, temp_path, mut config, mut state) = setup_test_env()?;

        // Create source file
        let source_file = temp_path.join("source").join("test-app.txt");
        fs::write(&source_file, "content")?;

        // Create target symlink
        let target_file = temp_path.join("target").join("test-app.txt");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&source_file, &target_file)?;
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&source_file, &target_file)?;

        // Add to state
        state.add_link(target_file.clone(), Utf8PathBuf::from("test-app.txt"));

        // Remove source file to break the symlink
        fs::remove_file(&source_file)?;

        let scanner = Scanner::new(temp_path.clone());
        let drift_items = scanner.scan_targets(&config, &state)?;

        // Should detect broken symlink
        assert_eq!(drift_items.len(), 1);
        assert_eq!(drift_items[0].drift_type, DriftType::Broken);
        assert_eq!(drift_items[0].target_path, target_file);

        Ok(())
    }

    #[test]
    fn test_scan_no_drift() -> Result<()> {
        let (_temp_dir, temp_path, mut config, state) = setup_test_env()?;

        // Create source directory with files
        let source_dir = temp_path.join("source").join("test-app");
        fs::create_dir_all(&source_dir)?;
        fs::write(source_dir.join("config.txt"), "source config")?;
        fs::write(source_dir.join("settings.json"), "{}")?;

        // Create target directory with exactly the same files
        let target_dir = temp_path.join("target").join(".config").join("test-app");
        fs::create_dir_all(&target_dir)?;
        fs::write(target_dir.join("config.txt"), "source config")?;
        fs::write(target_dir.join("settings.json"), "{}")?;

        // Update package target
        config.packages[0].target = target_dir.clone();

        let scanner = Scanner::new(temp_path.clone());
        let drift_items = scanner.scan_targets(&config, &state)?;

        // Should detect no drift
        assert_eq!(drift_items.len(), 0);

        Ok(())
    }

    #[test]
    fn test_scan_mixed_scenarios() -> Result<()> {
        let (_temp_dir, temp_path, mut config, mut state) = setup_test_env()?;

        // Add a second package with LinkFolder strategy
        config.packages.push(Package {
            source: "source/another-app".into(),
            target: "~/.config/another-app".into(),
            strategy: LinkStrategy::LinkFolder,
        });

        // Create source files for first package
        let source1_dir = temp_path.join("source").join("test-app");
        fs::create_dir_all(&source1_dir)?;
        fs::write(source1_dir.join("config.txt"), "source config")?;

        // Create source files for second package
        let source2_dir = temp_path.join("source").join("another-app");
        fs::create_dir_all(&source2_dir)?;
        fs::write(source2_dir.join("settings.json"), "{}")?;

        // Create target directories
        let target1_dir = temp_path.join("target").join(".config").join("test-app");
        let target2_dir = temp_path.join("target").join(".config").join("another-app");
        fs::create_dir_all(&target1_dir)?;
        fs::create_dir_all(&target2_dir)?;

        // Add tracked files + untracked files for first package
        fs::write(target1_dir.join("config.txt"), "source config")?; // tracked
        fs::write(target1_dir.join("untracked.txt"), "untracked")?; // untracked

        // Add tracked files for second package
        fs::write(target2_dir.join("settings.json"), "{}")?; // tracked
        fs::write(target2_dir.join("extra.txt"), "extra")?; // should not be detected as untracked (LinkFolder)

        // Create a broken symlink in state
        let broken_target = temp_path.join("target").join("broken-link");
        let missing_source = temp_path.join("missing");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&missing_source, &broken_target)?;
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&missing_source, &broken_target)?;
        state.add_link(broken_target.clone(), Utf8PathBuf::from("missing"));

        // Update package targets
        config.packages[0].target = target1_dir.clone();
        config.packages[1].target = target2_dir.clone();

        let scanner = Scanner::new(temp_path.clone());
        let drift_items = scanner.scan_targets(&config, &state)?;

        // Should detect:
        // - 1 untracked file from LinkFilesRecursive package
        // - 1 broken symlink from state
        assert_eq!(drift_items.len(), 2);

        let untracked_items: Vec<_> = drift_items.iter()
            .filter(|item| item.drift_type == DriftType::Untracked)
            .collect();
        let broken_items: Vec<_> = drift_items.iter()
            .filter(|item| item.drift_type == DriftType::Broken)
            .collect();

        assert_eq!(untracked_items.len(), 1);
        assert!(untracked_items[0].target_path.ends_with("untracked.txt"));

        assert_eq!(broken_items.len(), 1);
        assert_eq!(broken_items[0].target_path, broken_target);

        Ok(())
    }

    #[test]
    fn test_scan_broken_symlinks_relative_path_different_cwd() -> Result<()> {
        let (_temp_dir, temp_path, config, mut state) = setup_test_env()?;

        // Create source file
        let source_file = temp_path.join("source").join("test-app.txt");
        fs::write(&source_file, "content")?;

        // Create target symlink
        let target_file = temp_path.join("target").join("test-app.txt");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&source_file, &target_file)?;
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&source_file, &target_file)?;

        // Add to state using RELATIVE path
        // target/test-app.txt relative to temp_path
        let relative_target = Utf8PathBuf::from("target/test-app.txt");
        state.add_link(relative_target.clone(), Utf8PathBuf::from("test-app.txt"));

        // Remove source file to break the symlink
        fs::remove_file(&source_file)?;

        // Scanner uses temp_path as config_dir
        let scanner = Scanner::new(temp_path.clone());
        
        // We are running in project root (CWD), which is NOT temp_path.
        // So if Scanner doesn't resolve relative_target against temp_path, it will look for "target/test-app.txt" in project root and fail to find it.
        
        let drift_items = scanner.scan_targets(&config, &state)?;

        // Should detect broken symlink
        assert_eq!(drift_items.len(), 1);
        assert_eq!(drift_items[0].drift_type, DriftType::Broken);
        
        // The returned path should be ABSOLUTE (resolved)
        assert_eq!(drift_items[0].target_path, target_file);

        Ok(())
    }
}