use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use kdl::{KdlDocument, KdlEntry, KdlNode};
use std::collections::HashMap;
use std::fs;

/// Represents the state of deployed symlinks on a specific machine
#[derive(Debug, Clone, PartialEq)]
pub struct DotyState {
    pub hostname: String,
    /// Maps target path -> source path for all managed symlinks
    pub links: HashMap<Utf8PathBuf, Utf8PathBuf>,
}

impl DotyState {
    /// Create a new empty state for the given hostname
    pub fn new(hostname: String) -> Self {
        Self {
            hostname,
            links: HashMap::new(),
        }
    }

    /// Load state from file, or create new if it doesn't exist
    pub fn load(state_dir: &Utf8Path, hostname: &str) -> Result<Self> {
        let state_file = state_dir.join(format!("{}.kdl", hostname));

        if !state_file.exists() {
            return Ok(Self::new(hostname.to_string()));
        }

        let content = fs::read_to_string(&state_file)
            .with_context(|| format!("Failed to read state file: {}", state_file))?;

        Self::from_str(&content, hostname)
    }

    /// Parse state from KDL string
    pub fn from_str(content: &str, hostname: &str) -> Result<Self> {
        let doc: KdlDocument = content
            .parse()
            .context("Failed to parse KDL state document")?;

        let mut links = HashMap::new();

        for node in doc.nodes() {
            if node.name().value() == "link" {
                let (target, source) = Self::parse_link_node(node)?;
                links.insert(target, source);
            }
        }

        Ok(DotyState {
            hostname: hostname.to_string(),
            links,
        })
    }

    /// Parse a single link node
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

        Ok((target, source))
    }

    /// Save state to file
    pub fn save(&self, state_dir: &Utf8Path) -> Result<()> {
        // Ensure state directory exists
        fs::create_dir_all(state_dir)
            .with_context(|| format!("Failed to create state directory: {}", state_dir))?;

        let state_file = state_dir.join(format!("{}.kdl", self.hostname));
        let content = self.to_kdl();

        fs::write(&state_file, content)
            .with_context(|| format!("Failed to write state file: {}", state_file))?;

        Ok(())
    }

    /// Convert state to KDL format
    pub fn to_kdl(&self) -> String {
        let mut doc = KdlDocument::new();

        // Sort links for consistent output
        let mut sorted_links: Vec<_> = self.links.iter().collect();
        sorted_links.sort_by_key(|(target, _)| target.as_str());

        for (target, source) in sorted_links {
            let mut node = KdlNode::new("link");
            node.push(KdlEntry::new_prop("target", target.as_str()));
            node.push(KdlEntry::new_prop("source", source.as_str()));
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

    #[test]
    fn test_new_state() {
        let state = DotyState::new("test-host".to_string());
        assert_eq!(state.hostname, "test-host");
        assert_eq!(state.links.len(), 0);
    }

    #[test]
    fn test_add_remove_link() {
        let mut state = DotyState::new("test-host".to_string());

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
        let mut state = DotyState::new("test-host".to_string());
        state.add_link(
            Utf8PathBuf::from("~/.config/nvim"),
            Utf8PathBuf::from("nvim"),
        );
        state.add_link(
            Utf8PathBuf::from("~/.zshrc"),
            Utf8PathBuf::from("zsh/.zshrc"),
        );

        let kdl = state.to_kdl();
        assert!(kdl.contains("link"));
        assert!(kdl.contains("~/.config/nvim"));
        assert!(kdl.contains("~/.zshrc"));
    }

    #[test]
    fn test_from_str() {
        let kdl = r#"
            link target="~/.config/nvim" source="nvim"
            link target="~/.zshrc" source="zsh/.zshrc"
        "#;

        let state = DotyState::from_str(kdl, "test-host").unwrap();
        assert_eq!(state.hostname, "test-host");
        assert_eq!(state.links.len(), 2);
        assert!(state.is_managed(&Utf8PathBuf::from("~/.config/nvim")));
        assert!(state.is_managed(&Utf8PathBuf::from("~/.zshrc")));
    }

    #[test]
    fn test_roundtrip() {
        let mut state = DotyState::new("test-host".to_string());
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
}
