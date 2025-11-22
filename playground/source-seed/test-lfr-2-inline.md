# Test Case LFR-2: Single File Linking with Inline Syntax

**Strategy:** LinkFilesRecursive (Dotter-like)

**What this tests:**
- Single file linking using inline/single-line syntax
- Compact configuration format for file linking
- Creates individual file symlink

**Configuration:**
```kdl
LinkFilesRecursive "source/test-lfr-2-inline.md" target="target/test-lfr-2-inline.md"
```

**Expected Result:**
- `target/test-lfr-2-inline.md` becomes a symlink pointing to this file
- Functionally identical to block syntax, just more compact
- Individual file is symlinked

**Use Case:**
- Cleaner configuration for simple file links
- Quick one-liner for individual file management
- Minimal configuration syntax
