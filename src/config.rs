use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use kdl::{KdlDocument, KdlNode};
use std::fs;

/// Represents the entire Doty configuration
#[derive(Debug, Clone, PartialEq)]
pub struct DotyConfig {
    pub packages: Vec<Package>,
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
    /// Parse a KDL configuration file
    pub fn from_file(path: &Utf8PathBuf) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path))?;
        Self::from_str(&content)
    }

    /// Parse KDL configuration from a string
    pub fn from_str(content: &str) -> Result<Self> {
        let doc: KdlDocument = content.parse().context("Failed to parse KDL document")?;

        let mut packages = Vec::new();

        for node in doc.nodes() {
            if let Some(package) = Self::parse_package(node)? {
                packages.push(package);
            }
        }

        Ok(DotyConfig { packages })
    }

    /// Parse a single package node
    fn parse_package(node: &KdlNode) -> Result<Option<Package>> {
        let strategy = match node.name().value() {
            "LinkFolder" => LinkStrategy::LinkFolder,
            "LinkFilesRecursive" => LinkStrategy::LinkFilesRecursive,
            "defaults" => return Ok(None), // Skip defaults node for now
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
}
