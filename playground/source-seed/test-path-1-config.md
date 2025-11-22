# Test Case PATH-1: Config-Based Path Resolution (Default)

**Strategy:** LinkFilesRecursive (Dotter-like)

**What this tests:**
- Default path resolution strategy (relative to doty.kdl location)
- Paths are resolved relative to config file location
- Consistent behavior regardless of where doty is executed from

**Configuration:**
```kdl
defaults {
    pathResolution "config"  // This is the default
}

LinkFilesRecursive "source/test-path-1-config.md" {
    target "target/test-path-1-config.md"
}
```

**Expected Result:**
- `target/test-path-1-config.md` becomes a symlink pointing to this file
- Paths are resolved relative to `playground/` (where doty.kdl is located)
- Works when running `doty link` from any directory
- Example: `cd /anywhere && doty -c playground/doty.kdl link` works correctly

**Use Case:**
- Default and recommended path resolution strategy
- Allows config file in subdirectory while managing parent directory files
- Consistent behavior across different execution contexts
- Ideal for most dotfile management scenarios
