# Test Case LFR-3: Recursive Directory with Structure Recreation

**Strategy:** LinkFilesRecursive (Dotter-like)

**What this tests:**
- Recursive directory linking that recreates directory structure
- Individual file symlinks within recreated directory tree
- Demonstrates difference from LinkFolder (no directory symlink)

**Configuration:**
```kdl
LinkFilesRecursive "source/test-lfr-3-recursive" {
    target "target/test-lfr-3-recursive"
}
```

**Expected Result:**
- `target/test-lfr-3-recursive/` is created as a real directory (not symlink)
- `target/test-lfr-3-recursive/nested/` is created as a real directory
- Each file becomes an individual symlink:
  - `target/test-lfr-3-recursive/README.md` → symlink to this file
  - `target/test-lfr-3-recursive/nested/file.md` → symlink to nested file
- Directory structure is recreated, but only files are symlinked

**Use Case:**
- Applications that don't work with symlinked directory roots
- When you want to manage specific files but preserve directory structure
- Mixed ownership scenarios (some files tracked, some not)
