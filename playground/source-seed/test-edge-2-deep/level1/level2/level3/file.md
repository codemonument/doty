# Test Case EDGE-2: Deep Nesting - Specific File Extraction

**Strategy:** LinkFilesRecursive (Dotter-like)

**What this tests:**
- Extracting a file from very deep nesting (4 levels)
- Flattening deeply nested structure to root level
- Handling complex path traversal

**Configuration:**
```kdl
LinkFilesRecursive "source/test-edge-2-deep/level1/level2/level3/file.md" {
    target "target/test-edge-2-extracted.md"
}
```

**Expected Result:**
- `target/test-edge-2-extracted.md` becomes a symlink pointing to this deeply nested file
- Source path: `source/test-edge-2-deep/level1/level2/level3/file.md` (4 levels deep)
- Target path: `target/test-edge-2-extracted.md` (flat)
- Deep nesting is successfully flattened

**Use Case:**
- Extracting specific files from complex project structures
- Simplifying access to deeply nested configuration
- Creating shortcuts to frequently accessed nested files
