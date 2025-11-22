# Test Case LF-2: LinkFolder with Single-Line Syntax

**Strategy:** LinkFolder (Stow-like)

**What this tests:**
- LinkFolder functionality using inline/single-line syntax
- Demonstrates compact configuration format
- Creates a single symbolic link for the entire directory

**Configuration:**
```kdl
LinkFolder "source/test-lf-2-inline" target="target/test-lf-2-inline"
```

**Expected Result:**
- `target/test-lf-2-inline/` becomes a symlink pointing to `source/test-lf-2-inline/`
- Functionally identical to block syntax, just more compact
- All files accessible through the symlink

**Use Case:**
- Cleaner configuration file when you don't need additional options
- Quick one-liner for simple directory links
