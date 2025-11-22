# Test Case LF-1: Basic LinkFolder with Block Syntax

**Strategy:** LinkFolder (Stow-like)

**What this tests:**
- Basic LinkFolder functionality using block syntax
- Creates a single symbolic link for the entire directory
- Target directory name matches source directory name

**Configuration:**
```kdl
LinkFolder "source/test-lf-1-basic" {
    target "target/test-lf-1-basic"
}
```

**Expected Result:**
- `target/test-lf-1-basic/` becomes a symlink pointing to `source/test-lf-1-basic/`
- All files in this directory are accessible through the symlink
- No individual file symlinks are created

**Use Case:**
- Simple directory linking where you want all files tracked as a unit
- Ideal for application config directories where all files should be managed together
