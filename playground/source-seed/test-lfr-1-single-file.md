# Test Case LFR-1: Single File Linking with Block Syntax

**Strategy:** LinkFilesRecursive (Dotter-like)

**What this tests:**
- Single file linking using block syntax
- Creates individual file symlink (not directory symlink)
- Demonstrates LinkFilesRecursive for single file

**Configuration:**
```kdl
LinkFilesRecursive "source/test-lfr-1-single-file.md" {
    target "target/test-lfr-1-single-file.md"
}
```

**Expected Result:**
- `target/test-lfr-1-single-file.md` becomes a symlink pointing to this file
- No directory symlink created
- Individual file is symlinked

**Use Case:**
- Linking specific configuration files
- When you only want to manage certain files, not entire directories
- Selective file management in shared folders
