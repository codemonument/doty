use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use kdl::{KdlDocument, KdlEntry, KdlNode};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::fs_utils::resolve_target_path;

/// Represents the lockfile of deployed symlinks on a specific machine
#[derive(Debug, Clone, PartialEq)]
pub struct Lockfile {
    pub hostname: String,
    /// Lockfile format version for future compatibility
    pub lockfile_version: u32,
    /// Base path used for resolving relative paths (config_dir_or_cwd)
    pub base_path: Utf8PathBuf,
    /// Maps target path -> source path for all managed symlinks
    pub links: HashMap<Utf8PathBuf, Utf8PathBuf>,
}

impl Lockfile {
    /// Create a new empty lockfile for the given hostname and base path
    pub fn new(hostname: String, base_path: Utf8PathBuf) -> Self {
        Self {
            hostname,
            lockfile_version: 1,
            base_path,
            links: HashMap::new(),
        }
    }

    /// Load lockfile from directory, or create new if it doesn't exist
    pub fn load<P: AsRef<Path>>(
        lockfile_dir: P,
        hostname: &str,
        base_path: Utf8PathBuf,
    ) -> Result<Self> {
        let lockfile_path = lockfile_dir.as_ref().join(format!("{}.lock.kdl", hostname));

        if !lockfile_path.exists() {
            return Ok(Self::new(hostname.to_string(), base_path));
        }

        let content = fs::read_to_string(&lockfile_path)
            .with_context(|| format!("Failed to read lockfile: {}", lockfile_path.display()))?;

        Self::from_str(&content, hostname)
    }

    /// Parse lockfile from KDL string
    pub fn from_str(content: &str, hostname: &str) -> Result<Self> {
        let doc: KdlDocument = content
            .parse()
            .context("Failed to parse KDL state document")?;

        let mut lockfile_version = 1; // Default to version 1
        let mut base_path = Utf8PathBuf::from("."); // Default base path
        let mut links = HashMap::new();

        for node in doc.nodes() {
            match node.name().value() {
                "lockfileVersion" => {
                    if let Some(entry) = node.entries().first() {
                        if let Some(version) = entry.value().as_integer() {
                            lockfile_version = version as u32;
                        }
                    }
                }
                "basePath" => {
                    if let Some(path) = node.entries().first().and_then(|e| e.value().as_string()) {
                        base_path = Utf8PathBuf::from(path);
                    }
                }
                "link" => {
                    let (source, target) = Self::parse_link_node(node)?;
                    links.insert(target, source);
                }
                _ => {}
            }
        }

        Ok(Lockfile {
            hostname: hostname.to_string(),
            lockfile_version,
            base_path,
            links,
        })
    }

    /// Parse a single link node (returns source, target - note the order!)
    fn parse_link_node(node: &KdlNode) -> Result<(Utf8PathBuf, Utf8PathBuf)> {
        let mut target = None;
        let mut source = None;

        for entry in node.entries() {
            if let Some(name) = entry.name() {
                match name.value() {
                    "target" => {
                        target = entry.value().as_string().map(|s| Utf8PathBuf::from(s));
                    }
                    "source" => {
                        source = entry.value().as_string().map(|s| Utf8PathBuf::from(s));
                    }
                    _ => {}
                }
            }
        }

        let target = target.context("Missing 'target' in link node")?;
        let source = source.context("Missing 'source' in link node")?;

        // Return (source, target) - source first!
        Ok((source, target))
    }

    /// Save lockfile to directory
    pub fn save<P: AsRef<Path>>(&self, lockfile_dir: P) -> Result<()> {
        // Ensure lockfile directory exists
        fs::create_dir_all(&lockfile_dir).with_context(|| {
            format!(
                "Failed to create lockfile directory: {}",
                lockfile_dir.as_ref().display()
            )
        })?;

        let lockfile_path = lockfile_dir
            .as_ref()
            .join(format!("{}.lock.kdl", self.hostname));

        let content = self.to_kdl();
        fs::write(&lockfile_path, content)
            .with_context(|| format!("Failed to write lockfile: {}", lockfile_path.display()))?;

        Ok(())
    }

    /// Convert Lockfile struct to KDL format string
    pub fn to_kdl(&self) -> String {
        let mut doc = KdlDocument::new();

        // Add lockfileVersion
        let mut version_node = KdlNode::new("lockfileVersion");
        version_node.push(KdlEntry::new(self.lockfile_version as i128));
        doc.nodes_mut().push(version_node);

        // Add basePath
        let mut base_path_node = KdlNode::new("basePath");
        base_path_node.push(KdlEntry::new(self.base_path.as_str()));
        doc.nodes_mut().push(base_path_node);

        // Sort links for consistent output
        let mut sorted_links: Vec<_> = self.links.iter().collect();
        sorted_links.sort_by_key(|(target, _)| target.as_str());

        // Output source before target
        for (target, source) in sorted_links {
            let mut node = KdlNode::new("link");
            node.push(KdlEntry::new_prop("source", source.as_str()));
            node.push(KdlEntry::new_prop("target", target.as_str()));
            doc.nodes_mut().push(node);
        }

        doc.to_string()
    }

    /// Normalize a path to absolute using base_path
    /// Handles ~ expansion, absolute paths, and relative paths
    /// Note: We don't canonicalize paths here to preserve symlink paths (including broken ones)
    fn normalize_to_absolute(path: &Utf8Path, base_path: &Utf8Path) -> Result<Utf8PathBuf> {
        // Try to resolve as target path first (handles ~ expansion)
        if let Ok(resolved) = resolve_target_path(path, base_path) {
            // Return resolved path without canonicalizing (to preserve symlink paths)
            return Ok(resolved);
        }

        // If resolve_target_path fails, try simple absolute/relative check
        if path.is_absolute() {
            return Ok(path.to_path_buf());
        }

        // Relative path - join with base_path
        Ok(base_path.join(path))
    }

    /// Add a link to the lockfile (paths are normalized to absolute)
    pub fn add_link(&mut self, target: Utf8PathBuf, source: Utf8PathBuf) {
        // Normalize both paths to absolute
        let abs_target =
            Self::normalize_to_absolute(&target, &self.base_path).unwrap_or_else(|_| target);
        let abs_source =
            Self::normalize_to_absolute(&source, &self.base_path).unwrap_or_else(|_| source);
        self.links.insert(abs_target, abs_source);
    }

    /// Remove a link from the lockfile
    /// Normalizes the target path to absolute before removing
    pub fn remove_link(&mut self, target: &Utf8Path) -> Option<Utf8PathBuf> {
        if let Ok(abs_target) = Self::normalize_to_absolute(target, &self.base_path) {
            self.links.remove(&abs_target)
        } else {
            // Fallback to direct remove if normalization fails
            self.links.remove(target)
        }
    }

    /// Check if a target is managed by Doty
    /// Normalizes the target path to absolute before checking
    pub fn is_managed(&self, target: &Utf8Path) -> bool {
        if let Ok(abs_target) = Self::normalize_to_absolute(target, &self.base_path) {
            self.links.contains_key(&abs_target)
        } else {
            // Fallback to direct check if normalization fails
            self.links.contains_key(target)
        }
    }

    /// Get the source path for a target
    /// Normalizes the target path to absolute before looking up
    pub fn get_source(&self, target: &Utf8Path) -> Option<&Utf8PathBuf> {
        if let Ok(abs_target) = Self::normalize_to_absolute(target, &self.base_path) {
            self.links.get(&abs_target)
        } else {
            // Fallback to direct lookup if normalization fails
            self.links.get(target)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_new_lockfile() {
        let lockfile = Lockfile::new("test-host".to_string(), Utf8PathBuf::from("/test/base"));
        assert_eq!(lockfile.hostname, "test-host");
        assert_eq!(lockfile.lockfile_version, 1);
        assert_eq!(lockfile.base_path, Utf8PathBuf::from("/test/base"));
        assert_eq!(lockfile.links.len(), 0);
    }

    #[test]
    fn test_add_remove_link() {
        let mut lockfile = Lockfile::new("test-host".to_string(), Utf8PathBuf::from("/test/base"));

        lockfile.add_link(
            Utf8PathBuf::from("~/.config/nvim"),
            Utf8PathBuf::from("nvim"),
        );

        // Lockfile now stores absolute paths
        assert!(lockfile.is_managed(&Utf8PathBuf::from("~/.config/nvim")));
        // Source is normalized to absolute path
        let source = lockfile.get_source(&Utf8PathBuf::from("~/.config/nvim"));
        assert!(source.is_some());
        assert!(source.unwrap().is_absolute());
        assert!(source.unwrap().ends_with("nvim"));

        let removed = lockfile.remove_link(&Utf8PathBuf::from("~/.config/nvim"));
        assert!(removed.is_some());
        assert!(removed.unwrap().is_absolute());
        assert!(!lockfile.is_managed(&Utf8PathBuf::from("~/.config/nvim")));
    }

    #[test]
    fn test_to_kdl() {
        let mut lockfile = Lockfile::new("test-host".to_string(), Utf8PathBuf::from("/test/base"));
        lockfile.add_link(
            Utf8PathBuf::from("~/.config/nvim"),
            Utf8PathBuf::from("nvim"),
        );
        lockfile.add_link(
            Utf8PathBuf::from("~/.zshrc"),
            Utf8PathBuf::from("zsh/.zshrc"),
        );

        let kdl = lockfile.to_kdl();
        assert!(kdl.contains("lockfileVersion 1"));
        assert!(kdl.contains("basePath \"/test/base\""));
        assert!(kdl.contains("link"));
        // Lockfile now stores absolute paths
        assert!(kdl.contains("source=\"/test/base/nvim\""));
        // Target is normalized (HOME expansion), so check it's absolute
        assert!(kdl.contains("target="));
        assert!(kdl.contains("source=\"/test/base/zsh/.zshrc\""));
    }

    #[test]
    fn test_from_str() {
        let kdl = r#"
            lockfileVersion 1
            basePath "/test/base"
            link source="nvim" target="~/.config/nvim"
            link source="zsh/.zshrc" target="~/.zshrc"
        "#;

        let lockfile = Lockfile::from_str(kdl, "test-host").unwrap();
        assert_eq!(lockfile.hostname, "test-host");
        assert_eq!(lockfile.lockfile_version, 1);
        assert_eq!(lockfile.base_path, Utf8PathBuf::from("/test/base"));
        assert_eq!(lockfile.links.len(), 2);
        assert!(lockfile.is_managed(&Utf8PathBuf::from("~/.config/nvim")));
        assert!(lockfile.is_managed(&Utf8PathBuf::from("~/.zshrc")));
    }

    #[test]
    fn test_roundtrip() {
        let mut lockfile = Lockfile::new("test-host".to_string(), Utf8PathBuf::from("/test/base"));
        lockfile.add_link(
            Utf8PathBuf::from("~/.config/nvim"),
            Utf8PathBuf::from("nvim"),
        );
        lockfile.add_link(
            Utf8PathBuf::from("~/.zshrc"),
            Utf8PathBuf::from("zsh/.zshrc"),
        );

        let kdl = lockfile.to_kdl();
        let parsed = Lockfile::from_str(&kdl, "test-host").unwrap();

        assert_eq!(lockfile, parsed);
    }

    // Integration tests with real filesystem
    #[test]
    fn test_save_and_load_real_fs() {
        let test_dir = "tests/tmpfs/test_save_and_load_real_fs";
        let _ = fs::remove_dir_all(test_dir); // Clean up any existing test dir
        let lockfile_dir = format!("{}/.doty/state", test_dir);

        let mut lockfile = Lockfile::new("test-host".to_string(), Utf8PathBuf::from("/test/base"));
        lockfile.add_link(
            Utf8PathBuf::from("~/.config/nvim"),
            Utf8PathBuf::from("nvim"),
        );
        lockfile.add_link(
            Utf8PathBuf::from("~/.zshrc"),
            Utf8PathBuf::from("zsh/.zshrc"),
        );

        // Save lockfile
        lockfile.save(&lockfile_dir).unwrap();

        // Load lockfile
        let loaded =
            Lockfile::load(&lockfile_dir, "test-host", Utf8PathBuf::from("/test/base")).unwrap();

        assert_eq!(lockfile, loaded);

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    #[test]
    fn test_load_real_fs_nonexistent() {
        let test_dir = "tests/tmpfs/test_load_real_fs_nonexistent";
        let _ = fs::remove_dir_all(test_dir); // Clean up any existing test dir
        let lockfile_dir = format!("{}/.doty/state", test_dir);

        // Loading non-existent lockfile should return empty lockfile
        let lockfile =
            Lockfile::load(&lockfile_dir, "test-host", Utf8PathBuf::from("/test/base")).unwrap();
        assert_eq!(lockfile.hostname, "test-host");
        assert_eq!(lockfile.lockfile_version, 1);
        assert_eq!(lockfile.base_path, Utf8PathBuf::from("/test/base"));
        assert_eq!(lockfile.links.len(), 0);

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    #[test]
    fn test_save_real_fs_creates_directory() {
        let test_dir = "tests/tmpfs/test_save_real_fs_creates_directory";
        let _ = fs::remove_dir_all(test_dir); // Clean up any existing test dir
        let lockfile_dir = format!("{}/.doty/state", test_dir);

        let lockfile = Lockfile::new("test-host".to_string(), Utf8PathBuf::from("/test/base"));

        // Directory doesn't exist yet
        assert!(!Path::new(&lockfile_dir).exists());

        // Save should create directory
        lockfile.save(&lockfile_dir).unwrap();

        // Directory should now exist
        assert!(Path::new(&lockfile_dir).exists());

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    #[test]
    fn test_real_fs_roundtrip_multiple_lockfiles() {
        let test_dir = "tests/tmpfs/test_real_fs_roundtrip_multiple_lockfiles";
        let _ = fs::remove_dir_all(test_dir); // Clean up any existing test dir
        let lockfile_dir = format!("{}/.doty/state", test_dir);

        // Create and save lockfile for host1
        let mut lockfile1 = Lockfile::new("host1".to_string(), Utf8PathBuf::from("/test/base1"));
        lockfile1.add_link(
            Utf8PathBuf::from("~/.config/nvim"),
            Utf8PathBuf::from("nvim"),
        );
        lockfile1.save(&lockfile_dir).unwrap();

        // Create and save lockfile for host2
        let mut lockfile2 = Lockfile::new("host2".to_string(), Utf8PathBuf::from("/test/base2"));
        lockfile2.add_link(
            Utf8PathBuf::from("~/.zshrc"),
            Utf8PathBuf::from("zsh/.zshrc"),
        );
        lockfile2.save(&lockfile_dir).unwrap();

        // Load both lockfiles
        let loaded1 =
            Lockfile::load(&lockfile_dir, "host1", Utf8PathBuf::from("/test/base1")).unwrap();
        let loaded2 =
            Lockfile::load(&lockfile_dir, "host2", Utf8PathBuf::from("/test/base2")).unwrap();

        assert_eq!(lockfile1, loaded1);
        assert_eq!(lockfile2, loaded2);

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }
}
