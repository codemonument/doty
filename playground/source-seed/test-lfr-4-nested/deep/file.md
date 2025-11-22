# Test Case LFR-4: Nested Source to Flat Target

**Strategy:** LinkFilesRecursive (Dotter-like)

**What this tests:**
- Extracting a deeply nested file to a flat target location
- Flattening directory structure during linking
- Single file from nested path to root-level target

**Configuration:**
```kdl
LinkFilesRecursive "source/test-lfr-4-nested/deep/file.md" {
    target "target/test-lfr-4-flat.md"
}
```

**Expected Result:**
- `target/test-lfr-4-flat.md` becomes a symlink pointing to this deeply nested file
- Source path: `source/test-lfr-4-nested/deep/file.md` (nested)
- Target path: `target/test-lfr-4-flat.md` (flat, no nesting)
- Directory structure is flattened

**Use Case:**
- Extracting specific files from complex directory structures
- Simplifying access to deeply nested configuration files
- Creating flat target structure from nested source
