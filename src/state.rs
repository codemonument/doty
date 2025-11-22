use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use kdl::{KdlDocument, KdlEntry, KdlNode};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Represents the state of deployed symlinks on a specific machine
#[derive(Debug, Clone, PartialEq)]
pub struct DotyState {
    pub hostname: String,
    /// Lockfile format version for future compatibility
    pub lockfile_version: u32,
    /// Base path used for resolving relative paths (config_dir_or_cwd)
    pub base_path: Utf8PathBuf,
    /// Maps target path -> source path for all managed symlinks
    pub links: HashMap<Utf8PathBuf, Utf8PathBuf>,
}

impl DotyState {
    /// Create a new empty state for the given hostname and base path
    pub fn new(hostname: String, base_path: Utf8PathBuf) -> Self {
        Self {
            hostname,
            lockfile_version: 1,
            base_path,
            links: HashMap::new(),
        }
    }

    /// Load state from directory, or create new if it doesn't exist
    pub fn load<P: AsRef<Path>>(state_dir: P, hostname: &str, base_path: Utf8PathBuf) -> Result<Self> {
        let state_file = state_dir.as_ref().join(format!("{}.kdl", hostname));

        if !state_file.exists() {
            return Ok(Self::new(hostname.to_string(), base_path));
        }

        let content = fs::read_to_string(&state_file)
            .with_context(|| format!("Failed to read state file: {}", state_file.display()))?;

        Self::from_str(&content, hostname)
    }

    /// Parse state from KDL string
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

        Ok(DotyState {
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

    /// Save state to directory
    pub fn save<P: AsRef<Path>>(&self, state_dir: P) -> Result<()> {
        // Ensure state directory exists
        fs::create_dir_all(&state_dir)
            .with_context(|| format!("Failed to create state directory: {}", state_dir.as_ref().display()))?;

        let state_file = state_dir.as_ref().join(format!("{}.kdl", self.hostname));
        
        let content = self.to_kdl();
        fs::write(&state_file, content)
            .with_context(|| format!("Failed to write state file: {}", state_file.display()))?;

        Ok(())
    }

    /// Convert state to KDL format
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

    /// Add a link to the state
    pub fn add_link(&mut self, target: Utf8PathBuf, source: Utf8PathBuf) {
        self.links.insert(target, source);
    }

    /// Remove a link from the state
    pub fn remove_link(&mut self, target: &Utf8Path) -> Option<Utf8PathBuf> {
        self.links.remove(target)
    }

    /// Check if a target is managed by Doty
    pub fn is_managed(&self, target: &Utf8Path) -> bool {
        self.links.contains_key(target)
    }

    /// Get the source path for a target
    pub fn get_source(&self, target: &Utf8Path) -> Option<&Utf8PathBuf> {
        self.links.get(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_new_state() {
        let state = DotyState::new("test-host".to_string(), Utf8PathBuf::from("/test/base"));
        assert_eq!(state.hostname, "test-host");
        assert_eq!(state.lockfile_version, 1);
        assert_eq!(state.base_path, Utf8PathBuf::from("/test/base"));
        assert_eq!(state.links.len(), 0);
    }

    #[test]
    fn test_add_remove_link() {
        let mut state = DotyState::new("test-host".to_string(), Utf8PathBuf::from("/test/base"));

        state.add_link(
            Utf8PathBuf::from("~/.config/nvim"),
            Utf8PathBuf::from("nvim"),
        );

        assert!(state.is_managed(&Utf8PathBuf::from("~/.config/nvim")));
        assert_eq!(
            state.get_source(&Utf8PathBuf::from("~/.config/nvim")),
            Some(&Utf8PathBuf::from("nvim"))
        );

        let removed = state.remove_link(&Utf8PathBuf::from("~/.config/nvim"));
        assert_eq!(removed, Some(Utf8PathBuf::from("nvim")));
        assert!(!state.is_managed(&Utf8PathBuf::from("~/.config/nvim")));
    }

    #[test]
    fn test_to_kdl() {
        let mut state = DotyState::new("test-host".to_string(), Utf8PathBuf::from("/test/base"));
        state.add_link(
            Utf8PathBuf::from("~/.config/nvim"),
            Utf8PathBuf::from("nvim"),
        );
        state.add_link(
            Utf8PathBuf::from("~/.zshrc"),
            Utf8PathBuf::from("zsh/.zshrc"),
        );

        let kdl = state.to_kdl();
        assert!(kdl.contains("lockfileVersion 1"));
        assert!(kdl.contains("basePath \"/test/base\""));
        assert!(kdl.contains("link"));
        assert!(kdl.contains("source=nvim"));
        assert!(kdl.contains("target=\"~/.config/nvim\""));
        assert!(kdl.contains("source=\"zsh/.zshrc\""));
        assert!(kdl.contains("target=\"~/.zshrc\""));
    }

    #[test]
    fn test_from_str() {
        let kdl = r#"
            lockfileVersion 1
            basePath "/test/base"
            link source="nvim" target="~/.config/nvim"
            link source="zsh/.zshrc" target="~/.zshrc"
        "#;

        let state = DotyState::from_str(kdl, "test-host").unwrap();
        assert_eq!(state.hostname, "test-host");
        assert_eq!(state.lockfile_version, 1);
        assert_eq!(state.base_path, Utf8PathBuf::from("/test/base"));
        assert_eq!(state.links.len(), 2);
        assert!(state.is_managed(&Utf8PathBuf::from("~/.config/nvim")));
        assert!(state.is_managed(&Utf8PathBuf::from("~/.zshrc")));
    }

    #[test]
    fn test_roundtrip() {
        let mut state = DotyState::new("test-host".to_string(), Utf8PathBuf::from("/test/base"));
        state.add_link(
            Utf8PathBuf::from("~/.config/nvim"),
            Utf8PathBuf::from("nvim"),
        );
        state.add_link(
            Utf8PathBuf::from("~/.zshrc"),
            Utf8PathBuf::from("zsh/.zshrc"),
        );

        let kdl = state.to_kdl();
        let parsed = DotyState::from_str(&kdl, "test-host").unwrap();

        assert_eq!(state, parsed);
    }

    // Integration tests with real filesystem
    #[test]
    fn test_save_and_load_real_fs() {
        let test_dir = "tests/tmpfs/test_save_and_load_real_fs";
        let _ = fs::remove_dir_all(test_dir); // Clean up any existing test dir
        let state_dir = format!("{}/.doty/state", test_dir);

        let mut state = DotyState::new("test-host".to_string(), Utf8PathBuf::from("/test/base"));
        state.add_link(
            Utf8PathBuf::from("~/.config/nvim"),
            Utf8PathBuf::from("nvim"),
        );
        state.add_link(
            Utf8PathBuf::from("~/.zshrc"),
            Utf8PathBuf::from("zsh/.zshrc"),
        );

        // Save state
        state.save(&state_dir).unwrap();

        // Load state
        let loaded = DotyState::load(&state_dir, "test-host", Utf8PathBuf::from("/test/base")).unwrap();

        assert_eq!(state, loaded);

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    #[test]
    fn test_load_real_fs_nonexistent() {
        let test_dir = "tests/tmpfs/test_load_real_fs_nonexistent";
        let _ = fs::remove_dir_all(test_dir); // Clean up any existing test dir
        let state_dir = format!("{}/.doty/state", test_dir);

        // Loading non-existent state should return empty state
        let state = DotyState::load(&state_dir, "test-host", Utf8PathBuf::from("/test/base")).unwrap();
        assert_eq!(state.hostname, "test-host");
        assert_eq!(state.lockfile_version, 1);
        assert_eq!(state.base_path, Utf8PathBuf::from("/test/base"));
        assert_eq!(state.links.len(), 0);

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    #[test]
    fn test_save_real_fs_creates_directory() {
        let test_dir = "tests/tmpfs/test_save_real_fs_creates_directory";
        let _ = fs::remove_dir_all(test_dir); // Clean up any existing test dir
        let state_dir = format!("{}/.doty/state", test_dir);

        let state = DotyState::new("test-host".to_string(), Utf8PathBuf::from("/test/base"));
        
        // Directory doesn't exist yet
        assert!(!Path::new(&state_dir).exists());

        // Save should create directory
        state.save(&state_dir).unwrap();

        // Directory should now exist
        assert!(Path::new(&state_dir).exists());

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    #[test]
    fn test_real_fs_roundtrip_multiple_states() {
        let test_dir = "tests/tmpfs/test_real_fs_roundtrip_multiple_states";
        let _ = fs::remove_dir_all(test_dir); // Clean up any existing test dir
        let state_dir = format!("{}/.doty/state", test_dir);

        // Create and save state for host1
        let mut state1 = DotyState::new("host1".to_string(), Utf8PathBuf::from("/test/base1"));
        state1.add_link(
            Utf8PathBuf::from("~/.config/nvim"),
            Utf8PathBuf::from("nvim"),
        );
        state1.save(&state_dir).unwrap();

        // Create and save state for host2
        let mut state2 = DotyState::new("host2".to_string(), Utf8PathBuf::from("/test/base2"));
        state2.add_link(
            Utf8PathBuf::from("~/.zshrc"),
            Utf8PathBuf::from("zsh/.zshrc"),
        );
        state2.save(&state_dir).unwrap();

        // Load both states
        let loaded1 = DotyState::load(&state_dir, "host1", Utf8PathBuf::from("/test/base1")).unwrap();
        let loaded2 = DotyState::load(&state_dir, "host2", Utf8PathBuf::from("/test/base2")).unwrap();

        assert_eq!(state1, loaded1);
        assert_eq!(state2, loaded2);

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }
}
