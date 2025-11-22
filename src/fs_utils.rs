use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use std::fs;

/// Filesystem type detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsType {
    File,
    Directory,
    Symlink,
}

/// Scan directory recursively and return all files
pub fn scan_directory_recursive(dir: &Utf8Path) -> Result<Vec<Utf8PathBuf>> {
    let mut files = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let entry_path = Utf8PathBuf::from_path_buf(entry.path())
            .map_err(|_| anyhow::anyhow!("Path contains invalid UTF-8"))?;

        if entry_path.is_dir() {
            files.extend(scan_directory_recursive(&entry_path)?);
        } else {
            files.push(entry_path);
        }
    }

    Ok(files)
}

/// Resolve a target path (handle ~ expansion, absolute paths, and relative paths)
pub fn resolve_target_path(target: &Utf8Path, base_path: &Utf8Path) -> Result<Utf8PathBuf> {
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

    // Handle relative paths - relative to base_path
    Ok(base_path.join(target))
}

/// Get filesystem type for a given path
pub fn get_fs_type(path: &Utf8Path) -> Result<Option<FsType>> {
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.is_symlink() {
            Ok(Some(FsType::Symlink))
        } else if metadata.is_dir() {
            Ok(Some(FsType::Directory))
        } else {
            Ok(Some(FsType::File))
        }
    } else {
        Ok(None) // Path doesn't exist
    }
}

/// Read where a symlink points to (canonical path)
/// Returns None if not a symlink or broken
pub fn read_symlink_target(path: &Utf8Path) -> Result<Option<Utf8PathBuf>> {
    if let Ok(target) = fs::read_link(path) {
        if let Ok(canonical) = target.canonicalize() {
            Ok(Some(Utf8PathBuf::from_path_buf(canonical).unwrap_or_default()))
        } else {
            Ok(None) // Broken symlink
        }
    } else {
        Ok(None) // Not a symlink
    }
}

/// Check if path is a symlink that points nowhere
pub fn is_broken_symlink(path: &Utf8Path) -> Result<bool> {
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.is_symlink() {
            // It's a symlink, check if it's broken
            if let Ok(target) = fs::read_link(path) {
                // Try to canonicalize - if it fails, symlink is broken
                Ok(target.canonicalize().is_err())
            } else {
                Ok(true) // Can't read link target, assume broken
            }
        } else {
            Ok(false) // Not a symlink
        }
    } else {
        Ok(false) // Path doesn't exist
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn test_scan_directory_recursive() {
        let temp_dir = setup_test_dir();
        let temp_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();

        // Create nested structure
        fs::create_dir_all(temp_path.join("level1/level2")).unwrap();
        fs::write(temp_path.join("file1.txt"), "content1").unwrap();
        fs::write(temp_path.join("level1/file2.txt"), "content2").unwrap();
        fs::write(temp_path.join("level1/level2/file3.txt"), "content3").unwrap();

        let files = scan_directory_recursive(&temp_path).unwrap();

        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|f| f.ends_with("file1.txt")));
        assert!(files.iter().any(|f| f.ends_with("file2.txt")));
        assert!(files.iter().any(|f| f.ends_with("file3.txt")));
    }

    #[test]
    fn test_resolve_target_path_home_expansion() {
        // Test ~ expansion
        let target = Utf8PathBuf::from("~/test/file.txt");
        let base_path = Utf8PathBuf::from("/some/base");
        
        // We can't easily test HOME expansion without mocking env vars,
        // but we can test that it doesn't panic
        let result = resolve_target_path(&target, &base_path);
        
        // Should succeed if HOME is set
        if std::env::var("HOME").is_ok() {
            assert!(result.is_ok());
            let resolved = result.unwrap();
            assert!(resolved.starts_with(std::env::var("HOME").unwrap()));
        }
    }

    #[test]
    fn test_resolve_target_path_absolute() {
        let target = Utf8PathBuf::from("/absolute/path/file.txt");
        let base_path = Utf8PathBuf::from("/some/base");
        
        let resolved = resolve_target_path(&target, &base_path).unwrap();
        assert_eq!(resolved, Utf8PathBuf::from("/absolute/path/file.txt"));
    }

    #[test]
    fn test_resolve_target_path_relative() {
        let target = Utf8PathBuf::from("relative/path/file.txt");
        let base_path = Utf8PathBuf::from("/some/base");
        
        let resolved = resolve_target_path(&target, &base_path).unwrap();
        assert_eq!(resolved, Utf8PathBuf::from("/some/base/relative/path/file.txt"));
    }

    #[test]
    fn test_get_fs_type_file() {
        let temp_dir = setup_test_dir();
        let temp_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
        
        let file_path = temp_path.join("test.txt");
        fs::write(&file_path, "content").unwrap();
        
        let fs_type = get_fs_type(&file_path).unwrap();
        assert_eq!(fs_type, Some(FsType::File));
    }

    #[test]
    fn test_get_fs_type_directory() {
        let temp_dir = setup_test_dir();
        let temp_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
        
        let fs_type = get_fs_type(&temp_path).unwrap();
        assert_eq!(fs_type, Some(FsType::Directory));
    }

    #[test]
    fn test_get_fs_type_nonexistent() {
        let nonexistent = Utf8PathBuf::from("/nonexistent/path");
        
        let fs_type = get_fs_type(&nonexistent).unwrap();
        assert_eq!(fs_type, None);
    }

    #[test]
    fn test_get_fs_type_symlink() {
        let temp_dir = setup_test_dir();
        let temp_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
        
        // Create source file
        let source_path = temp_path.join("source.txt");
        fs::write(&source_path, "content").unwrap();
        
        // Create symlink
        let link_path = temp_path.join("link.txt");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&source_path, &link_path).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&source_path, &link_path).unwrap();
        
        let fs_type = get_fs_type(&link_path).unwrap();
        assert_eq!(fs_type, Some(FsType::Symlink));
    }

    #[test]
    fn test_read_symlink_target_valid() {
        let temp_dir = setup_test_dir();
        let temp_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
        
        // Create source file
        let source_path = temp_path.join("source.txt");
        fs::write(&source_path, "content").unwrap();
        
        // Create symlink
        let link_path = temp_path.join("link.txt");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&source_path, &link_path).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&source_path, &link_path).unwrap();
        
        let target = read_symlink_target(&link_path).unwrap();
        assert!(target.is_some());
        assert_eq!(target.unwrap(), source_path.canonicalize().unwrap());
    }

    #[test]
    fn test_read_symlink_target_broken() {
        let temp_dir = setup_test_dir();
        let temp_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
        
        // Create symlink to non-existent file
        let nonexistent_source = temp_path.join("nonexistent.txt");
        let link_path = temp_path.join("broken_link.txt");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&nonexistent_source, &link_path).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&nonexistent_source, &link_path).unwrap();
        
        let target = read_symlink_target(&link_path).unwrap();
        assert_eq!(target, None);
    }

    #[test]
    fn test_read_symlink_target_not_symlink() {
        let temp_dir = setup_test_dir();
        let temp_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
        
        // Create regular file
        let file_path = temp_path.join("regular.txt");
        fs::write(&file_path, "content").unwrap();
        
        let target = read_symlink_target(&file_path).unwrap();
        assert_eq!(target, None);
    }

    #[test]
    fn test_is_broken_symlink_valid() {
        let temp_dir = setup_test_dir();
        let temp_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
        
        // Create source file
        let source_path = temp_path.join("source.txt");
        fs::write(&source_path, "content").unwrap();
        
        // Create symlink
        let link_path = temp_path.join("link.txt");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&source_path, &link_path).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&source_path, &link_path).unwrap();
        
        let is_broken = is_broken_symlink(&link_path).unwrap();
        assert!(!is_broken);
    }

    #[test]
    fn test_is_broken_symlink_broken() {
        let temp_dir = setup_test_dir();
        let temp_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
        
        // Create symlink to non-existent file
        let nonexistent_source = temp_path.join("nonexistent.txt");
        let link_path = temp_path.join("broken_link.txt");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&nonexistent_source, &link_path).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&nonexistent_source, &link_path).unwrap();
        
        let is_broken = is_broken_symlink(&link_path).unwrap();
        assert!(is_broken);
    }

    #[test]
    fn test_is_broken_symlink_not_symlink() {
        let temp_dir = setup_test_dir();
        let temp_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
        
        // Create regular file
        let file_path = temp_path.join("regular.txt");
        fs::write(&file_path, "content").unwrap();
        
        let is_broken = is_broken_symlink(&file_path).unwrap();
        assert!(!is_broken);
    }

    #[test]
    fn test_is_broken_symlink_nonexistent() {
        let nonexistent = Utf8PathBuf::from("/nonexistent/path");
        
        let is_broken = is_broken_symlink(&nonexistent).unwrap();
        assert!(!is_broken);
    }
}