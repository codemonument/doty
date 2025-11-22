# Test Case EDGE-4: Directory with Mixed Content (Selective Linking)

**Strategy:** LinkFilesRecursive (Dotter-like)

**What this tests:**
- Linking only specific files from a directory
- Demonstrates selective file management
- Shows that not all files in a directory need to be linked

**Configuration:**
```kdl
LinkFilesRecursive "source/test-edge-4-mixed/tracked.md" {
    target "target/test-edge-4-mixed/tracked.md"
}
```

**Expected Result:**
- `target/test-edge-4-mixed/` is created as a real directory
- `target/test-edge-4-mixed/tracked.md` becomes a symlink pointing to this file
- `untracked.txt` in the same source directory is NOT linked
- Only explicitly configured files are linked

**Use Case:**
- Managing specific files in shared directories
- Selective dotfile management
- When you only want to track certain files, not entire directories
- Mixed ownership scenarios
