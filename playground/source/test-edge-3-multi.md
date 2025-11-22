# Test Case EDGE-3: Same Source to Multiple Targets

**Strategy:** LinkFilesRecursive (Dotter-like)

**What this tests:**
- Creating multiple symlinks to the same source file
- Demonstrates that one source can have multiple target links
- Tests duplicate source handling

**Configuration:**
```kdl
LinkFilesRecursive "source/test-edge-3-multi.md" {
    target "target/test-edge-3-copy1.md"
}

LinkFilesRecursive "source/test-edge-3-multi.md" {
    target "target/test-edge-3-copy2.md"
}
```

**Expected Result:**
- `target/test-edge-3-copy1.md` becomes a symlink pointing to this file
- `target/test-edge-3-copy2.md` also becomes a symlink pointing to this file
- Both symlinks point to the same source
- No conflicts or errors

**Use Case:**
- Sharing configuration across multiple locations
- Creating multiple access points to same file
- Different naming conventions for same content
