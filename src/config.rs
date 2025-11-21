use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use kdl::{KdlDocument, KdlNode};
use std::fs;
use std::path::Path;

/// Path resolution strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathResolution {
    /// Resolve paths relative to config file location (default)
    Config,
    /// Resolve paths relative to current working directory
    Cwd,
}

impl Default for PathResolution {
    fn default() -> Self {
        PathResolution::Config
    }
}

impl std::fmt::Display for PathResolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathResolution::Config => write!(f, "config"),
            PathResolution::Cwd => write!(f, "cwd"),
        }
    }
}

/// Represents the entire Doty configuration
#[derive(Debug, Clone, PartialEq)]
pub struct DotyConfig {
    pub packages: Vec<Package>,
    pub path_resolution: PathResolution,
}

/// A package defines a source and how it should be linked
#[derive(Debug, Clone, PartialEq)]
pub struct Package {
    pub source: Utf8PathBuf,
    pub target: Utf8PathBuf,
    pub strategy: LinkStrategy,
}

/// Linking strategy for a package
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkStrategy {
    /// Create a single symlink for the entire directory (Stow-like)
    LinkFolder,
    /// Recreate directory structure and symlink individual files (Dotter-like)
    LinkFilesRecursive,
}

impl DotyConfig {
    /// Parse a KDL configuration file from a file path
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {}", path.as_ref().display()))?;
        Self::from_str(&content)
    }

    /// Parse KDL configuration from a string
    pub fn from_str(content: &str) -> Result<Self> {
        let doc: KdlDocument = content.parse().context("Failed to parse KDL document")?;

        let mut packages = Vec::new();
        let mut path_resolution = PathResolution::default();

        for node in doc.nodes() {
            if let Some(package) = Self::parse_package(node)? {
                packages.push(package);
            } else if node.name().value() == "defaults" {
                path_resolution = Self::parse_defaults(node)?;
            }
        }

        Ok(DotyConfig {
            packages,
            path_resolution,
        })
    }

    /// Parse the defaults node
    fn parse_defaults(node: &KdlNode) -> Result<PathResolution> {
        let mut path_resolution = PathResolution::default();

        if let Some(children) = node.children() {
            for child in children.nodes() {
                match child.name().value() {
                    "pathResolution" => {
                        let value = child
                            .entries()
                            .first()
                            .and_then(|e| e.value().as_string())
                            .with_context(|| "pathResolution requires a string value")?;

                        path_resolution = match value {
                            "config" => PathResolution::Config,
                            "cwd" => PathResolution::Cwd,
                            other => anyhow::bail!(
                                "Invalid pathResolution value: {}. Must be 'config' or 'cwd'",
                                other
                            ),
                        };
                    }
                    _other => {
                        // For now, we only care about pathResolution
                        // Other defaults can be added later
                    }
                }
            }
        }

        Ok(path_resolution)
    }

    /// Parse a single package node
    fn parse_package(node: &KdlNode) -> Result<Option<Package>> {
        let strategy = match node.name().value() {
            "LinkFolder" => LinkStrategy::LinkFolder,
            "LinkFilesRecursive" => LinkStrategy::LinkFilesRecursive,
            "defaults" => return Ok(None), // Handle defaults separately
            other => {
                anyhow::bail!("Unknown node type: {}", other);
            }
        };

        // Get source path from first argument
        let source = node
            .entries()
            .iter()
            .find(|e| e.name().is_none())
            .and_then(|e| e.value().as_string())
            .with_context(|| format!("Missing source path for {} node", node.name().value()))?;

        // Get target path - either from inline property or child node
        let target = Self::get_target(node)?;

        Ok(Some(Package {
            source: Utf8PathBuf::from(source),
            target: Utf8PathBuf::from(target),
            strategy,
        }))
    }

    /// Extract target path from node (inline property or child node)
    fn get_target(node: &KdlNode) -> Result<String> {
        // Try inline property first: LinkFolder "nvim" target="~/.config/nvim"
        if let Some(target_entry) = node
            .entries()
            .iter()
            .find(|e| e.name().map(|n| n.value()) == Some("target"))
        {
            if let Some(target) = target_entry.value().as_string() {
                return Ok(target.to_string());
            }
        }

        // Try child node: LinkFolder "nvim" { target "~/.config/nvim" }
        if let Some(children) = node.children() {
            for child in children.nodes() {
                if child.name().value() == "target" {
                    if let Some(target) =
                        child.entries().first().and_then(|e| e.value().as_string())
                    {
                        return Ok(target.to_string());
                    }
                }
            }
        }

        anyhow::bail!("Missing target path for {} node", node.name().value())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_parse_link_folder_inline() {
        let config = r#"
            LinkFolder "nvim" target="~/.config/nvim"
        "#;

        let result = DotyConfig::from_str(config).unwrap();
        assert_eq!(result.packages.len(), 1);

        let pkg = &result.packages[0];
        assert_eq!(pkg.source, Utf8PathBuf::from("nvim"));
        assert_eq!(pkg.target, Utf8PathBuf::from("~/.config/nvim"));
        assert_eq!(pkg.strategy, LinkStrategy::LinkFolder);
    }

    #[test]
    fn test_parse_link_folder_block() {
        let config = r#"
            LinkFolder "nvim" {
                target "~/.config/nvim"
            }
        "#;

        let result = DotyConfig::from_str(config).unwrap();
        assert_eq!(result.packages.len(), 1);

        let pkg = &result.packages[0];
        assert_eq!(pkg.source, Utf8PathBuf::from("nvim"));
        assert_eq!(pkg.target, Utf8PathBuf::from("~/.config/nvim"));
        assert_eq!(pkg.strategy, LinkStrategy::LinkFolder);
    }

    #[test]
    fn test_parse_link_files_recursive() {
        let config = r#"
            LinkFilesRecursive "zsh/.zshrc" target="~/.zshrc"
        "#;

        let result = DotyConfig::from_str(config).unwrap();
        assert_eq!(result.packages.len(), 1);

        let pkg = &result.packages[0];
        assert_eq!(pkg.source, Utf8PathBuf::from("zsh/.zshrc"));
        assert_eq!(pkg.target, Utf8PathBuf::from("~/.zshrc"));
        assert_eq!(pkg.strategy, LinkStrategy::LinkFilesRecursive);
    }

    #[test]
    fn test_parse_multiple_packages() {
        let config = r#"
            LinkFolder "nvim" target="~/.config/nvim"
            LinkFolder "alacritty" target="~/.config/alacritty"
            LinkFilesRecursive "zsh/.zshrc" target="~/.zshrc"
        "#;

        let result = DotyConfig::from_str(config).unwrap();
        assert_eq!(result.packages.len(), 3);
    }

    #[test]
    fn test_skip_defaults_node() {
        let config = r#"
            defaults {
                // Global settings
            }
            LinkFolder "nvim" target="~/.config/nvim"
        "#;

        let result = DotyConfig::from_str(config).unwrap();
        assert_eq!(result.packages.len(), 1);
    }

    #[test]
    fn test_missing_source() {
        let config = r#"
            LinkFolder target="~/.config/nvim"
        "#;

        let result = DotyConfig::from_str(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_target() {
        let config = r#"
            LinkFolder "nvim"
        "#;

        let result = DotyConfig::from_str(config);
        assert!(result.is_err());
    }

    // Integration tests with real filesystem
    #[test]
    fn test_from_file_real_fs() {
        let test_dir = "tests/tmpfs/test_from_file_real_fs";
        let _ = fs::remove_dir_all(test_dir); // Clean up any existing test dir
        fs::create_dir_all(test_dir).unwrap();

        let config_content = r#"
            LinkFolder "nvim" target="~/.config/nvim"
            LinkFilesRecursive "zsh/.zshrc" target="~/.zshrc"
        "#;

        let config_path = format!("{}/doty.kdl", test_dir);
        fs::write(&config_path, config_content).unwrap();

        let result = DotyConfig::from_file(&config_path).unwrap();
        assert_eq!(result.packages.len(), 2);
        assert_eq!(result.packages[0].strategy, LinkStrategy::LinkFolder);
        assert_eq!(
            result.packages[1].strategy,
            LinkStrategy::LinkFilesRecursive
        );

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    #[test]
    fn test_from_file_not_found() {
        let config_path = "tests/tmpfs/nonexistent.kdl";
        let result = DotyConfig::from_file(config_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_file_invalid_kdl() {
        let test_dir = "tests/tmpfs/test_from_file_invalid_kdl";
        let _ = fs::remove_dir_all(test_dir); // Clean up any existing test dir
        fs::create_dir_all(test_dir).unwrap();

        let config_path = format!("{}/doty.kdl", test_dir);
        fs::write(&config_path, "invalid {{ kdl syntax").unwrap();

        let result = DotyConfig::from_file(&config_path);
        assert!(result.is_err());

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    #[test]
    fn test_parse_defaults_config_resolution() {
        let config = r#"
            defaults {
                pathResolution "config"
            }
            LinkFolder "nvim" target="~/.config/nvim"
        "#;

        let result = DotyConfig::from_str(config).unwrap();
        assert_eq!(result.path_resolution, PathResolution::Config);
        assert_eq!(result.packages.len(), 1);
    }

    #[test]
    fn test_parse_defaults_cwd_resolution() {
        let config = r#"
            defaults {
                pathResolution "cwd"
            }
            LinkFolder "nvim" target="~/.config/nvim"
        "#;

        let result = DotyConfig::from_str(config).unwrap();
        assert_eq!(result.path_resolution, PathResolution::Cwd);
        assert_eq!(result.packages.len(), 1);
    }

    #[test]
    fn test_parse_defaults_no_path_resolution() {
        let config = r#"
            defaults {
                // No pathResolution specified
            }
            LinkFolder "nvim" target="~/.config/nvim"
        "#;

        let result = DotyConfig::from_str(config).unwrap();
        assert_eq!(result.path_resolution, PathResolution::Config); // Default
        assert_eq!(result.packages.len(), 1);
    }

    #[test]
    fn test_parse_no_defaults() {
        let config = r#"
            LinkFolder "nvim" target="~/.config/nvim"
        "#;

        let result = DotyConfig::from_str(config).unwrap();
        assert_eq!(result.path_resolution, PathResolution::Config); // Default
        assert_eq!(result.packages.len(), 1);
    }

    #[test]
    fn test_parse_invalid_path_resolution() {
        let config = r#"
            defaults {
                pathResolution "invalid"
            }
            LinkFolder "nvim" target="~/.config/nvim"
        "#;

        let result = DotyConfig::from_str(config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid pathResolution value"));
    }

    #[test]
    fn test_path_resolution_display() {
        assert_eq!(PathResolution::Config.to_string(), "config");
        assert_eq!(PathResolution::Cwd.to_string(), "cwd");
    }

    // Integration tests for path resolution with real filesystem
    #[test]
    fn test_path_resolution_config_strategy() {
        let test_dir = "tests/tmpfs/test_path_resolution_config_strategy";
        let _ = fs::remove_dir_all(test_dir); // Clean up any existing test dir
        fs::create_dir_all(format!("{}/configs/nvim", test_dir)).unwrap();

        // Create config file with config path resolution
        let config_content = r#"
            defaults {
                pathResolution "config"
            }
            LinkFolder "nvim" target="~/.config/nvim"
        "#;

        let config_path = format!("{}/configs/doty.kdl", test_dir);
        fs::write(&config_path, config_content).unwrap();

        // Load config and verify path resolution
        let config = DotyConfig::from_file(&config_path).unwrap();
        assert_eq!(config.path_resolution, PathResolution::Config);
        assert_eq!(config.packages.len(), 1);

        // The source path should be resolved relative to config file location
        // So "nvim" should resolve to "configs/nvim" (relative to config)
        assert_eq!(config.packages[0].source, Utf8PathBuf::from("nvim"));

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    #[test]
    fn test_path_resolution_cwd_strategy() {
        let test_dir = "tests/tmpfs/test_path_resolution_cwd_strategy";
        let _ = fs::remove_dir_all(test_dir); // Clean up any existing test dir
        fs::create_dir_all(format!("{}/dotfiles/configs", test_dir)).unwrap();
        fs::create_dir_all(format!("{}/dotfiles/nvim", test_dir)).unwrap();

        // Create config file with cwd path resolution
        let config_content = r#"
            defaults {
                pathResolution "cwd"
            }
            LinkFolder "nvim" target="~/.config/nvim"
        "#;

        let config_path = format!("{}/dotfiles/configs/doty.kdl", test_dir);
        fs::write(&config_path, config_content).unwrap();

        // Load config and verify path resolution
        let config = DotyConfig::from_file(&config_path).unwrap();
        assert_eq!(config.path_resolution, PathResolution::Cwd);
        assert_eq!(config.packages.len(), 1);

        // The source path should be resolved relative to current working directory
        // So "nvim" should resolve to "nvim" (relative to cwd, which would be dotfiles/)
        assert_eq!(config.packages[0].source, Utf8PathBuf::from("nvim"));

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    #[test]
    fn test_path_resolution_default_to_config() {
        let test_dir = "tests/tmpfs/test_path_resolution_default_to_config";
        let _ = fs::remove_dir_all(test_dir); // Clean up any existing test dir
        fs::create_dir_all(format!("{}/configs/nvim", test_dir)).unwrap();

        // Create config file without explicit path resolution (should default to config)
        let config_content = r#"
            LinkFolder "nvim" target="~/.config/nvim"
        "#;

        let config_path = format!("{}/configs/doty.kdl", test_dir);
        fs::write(&config_path, config_content).unwrap();

        // Load config and verify default path resolution
        let config = DotyConfig::from_file(&config_path).unwrap();
        assert_eq!(config.path_resolution, PathResolution::Config);
        assert_eq!(config.packages.len(), 1);

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }
}
