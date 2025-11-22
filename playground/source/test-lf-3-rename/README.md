# Test Case LF-3: LinkFolder with Renamed Target

**Strategy:** LinkFolder (Stow-like)

**What this tests:**
- LinkFolder with different source and target names
- Demonstrates directory renaming during linking
- Creates a single symbolic link with custom target name

**Configuration:**
```kdl
LinkFolder "source/test-lf-3-rename" {
    target "target/test-lf-3-renamed-folder"
}
```

**Expected Result:**
- `target/test-lf-3-renamed-folder/` becomes a symlink pointing to `source/test-lf-3-rename/`
- Source directory name: `test-lf-3-rename`
- Target directory name: `test-lf-3-renamed-folder` (different!)
- All files accessible through the renamed symlink

**Use Case:**
- When target location requires different naming convention
- Adapting dotfiles to different system configurations
- Creating multiple links to same source with different names
