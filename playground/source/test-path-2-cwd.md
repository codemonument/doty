# Test Case PATH-2: CWD-Based Path Resolution

**Strategy:** LinkFilesRecursive (Dotter-like)

**What this tests:**
- Alternative path resolution strategy (relative to current working directory)
- Paths are resolved relative to where doty command is executed
- Behavior changes based on execution location

**Configuration:**
```kdl
defaults {
    pathResolution "cwd"  // Alternative strategy
}

LinkFilesRecursive "source/test-path-2-cwd.md" {
    target "target/test-path-2-cwd.md"
}
```

**Expected Result:**
- `target/test-path-2-cwd.md` becomes a symlink pointing to this file
- Paths are resolved relative to current working directory when doty is executed
- Must run from `playground/` directory: `cd playground && doty link`
- Running from other directories will fail (paths won't resolve correctly)

**Use Case:**
- When you want paths relative to execution location
- Useful for scripts that change directory before running doty
- Less common than config-based resolution
- Requires careful execution context management

**Note:** This test case is commented out by default in doty.kdl.
To test it, uncomment the configuration and change `defaults.pathResolution` to "cwd".
