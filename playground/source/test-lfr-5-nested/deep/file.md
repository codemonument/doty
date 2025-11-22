# Test Case LFR-5: Nested Source to Nested Target

**Strategy:** LinkFilesRecursive (Dotter-like)

**What this tests:**
- Preserving nested directory structure from source to target
- Creating nested directories in target location
- Demonstrates path structure preservation

**Configuration:**
```kdl
LinkFilesRecursive "source/test-lfr-5-nested/deep/file.md" {
    target "target/test-lfr-5-nested/deep/file.md"
}
```

**Expected Result:**
- `target/test-lfr-5-nested/` is created as a real directory
- `target/test-lfr-5-nested/deep/` is created as a real directory
- `target/test-lfr-5-nested/deep/file.md` becomes a symlink pointing to this file
- Full nested structure is preserved in target

**Use Case:**
- Maintaining directory hierarchy in target location
- When target structure should mirror source structure
- Organizing linked files in nested target directories
