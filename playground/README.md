# Doty Playground - Comprehensive Test Suite

This directory contains a comprehensive test setup for **doty**, demonstrating all linking strategies, edge cases, and configuration options.

## Quick Start

```bash
# From playground directory
cd playground

# Preview what will be linked
doty link --dry-run

# Create all symlinks
doty link

# Check status of links
doty status

# Clean up all symlinks
doty clean
```

## Test Case Structure

Each test case has dedicated files/directories with explanatory markdown content describing what it tests, expected results, and use cases.

### LinkFolder Strategy Tests (LF-*)
**Strategy:** Stow-like - creates a single symlink for entire directory

| Test | Description | Source | Target |
|------|-------------|--------|--------|
| **LF-1** | Basic block syntax | `source/test-lf-1-basic/` | `target/test-lf-1-basic/` |
| **LF-2** | Inline syntax | `source/test-lf-2-inline/` | `target/test-lf-2-inline/` |
| **LF-3** | Renamed target | `source/test-lf-3-rename/` | `target/test-lf-3-renamed-folder/` |

**Use Cases:**
- Application configs where you want ALL files tracked
- Directories where new files should be immediately tracked
- "What you see is what you get" behavior

### LinkFilesRecursive Strategy Tests (LFR-*)
**Strategy:** Dotter-like - recreates directory structure, symlinks individual files

| Test | Description | Source | Target |
|------|-------------|--------|--------|
| **LFR-1** | Single file (block syntax) | `source/test-lfr-1-single-file.md` | `target/test-lfr-1-single-file.md` |
| **LFR-2** | Single file (inline syntax) | `source/test-lfr-2-inline.md` | `target/test-lfr-2-inline.md` |
| **LFR-3** | Recursive directory | `source/test-lfr-3-recursive/` | `target/test-lfr-3-recursive/` |
| **LFR-4** | Nested to flat | `source/test-lfr-4-nested/deep/file.md` | `target/test-lfr-4-flat.md` |
| **LFR-5** | Nested to nested | `source/test-lfr-5-nested/deep/file.md` | `target/test-lfr-5-nested/deep/file.md` |

**Use Cases:**
- Complex configs with mixed ownership
- Apps that dislike symlinked directory roots
- Selective file management in shared folders

### Edge Cases (EDGE-*)

| Test | Description | What It Tests |
|------|-------------|---------------|
| **EDGE-1** | Hidden file | Dotfile handling (`.test-hidden`) |
| **EDGE-2** | Deep nesting | Extracting from 4 levels deep |
| **EDGE-3** | Multiple targets | Same source → multiple symlinks |
| **EDGE-4** | Selective linking | Only specific files from directory |

### Path Resolution Tests (PATH-*)

| Test | Strategy | Description |
|------|----------|-------------|
| **PATH-1** | `config` | Paths relative to doty.kdl (default) |
| **PATH-2** | `cwd` | Paths relative to execution directory (commented out) |

## Directory Structure

```
playground/
├── doty.kdl                          # Main configuration file
├── README.md                         # This file
├── source/                           # Source files (to be linked)
│   ├── .test-hidden                  # EDGE-1: Hidden file test
│   ├── test-lf-1-basic/              # LF-1: Basic LinkFolder
│   │   └── README.md
│   ├── test-lf-2-inline/             # LF-2: Inline syntax
│   │   └── README.md
│   ├── test-lf-3-rename/             # LF-3: Renamed target
│   │   └── README.md
│   ├── test-lfr-1-single-file.md     # LFR-1: Single file (block)
│   ├── test-lfr-2-inline.md          # LFR-2: Single file (inline)
│   ├── test-lfr-3-recursive/         # LFR-3: Recursive directory
│   │   ├── README.md
│   │   └── nested/
│   │       └── file.md
│   ├── test-lfr-4-nested/            # LFR-4: Nested to flat
│   │   └── deep/
│   │       └── file.md
│   ├── test-lfr-5-nested/            # LFR-5: Nested to nested
│   │   └── deep/
│   │       └── file.md
│   ├── test-edge-2-deep/             # EDGE-2: Deep nesting
│   │   └── level1/level2/level3/
│   │       └── file.md
│   ├── test-edge-3-multi.md          # EDGE-3: Multiple targets
│   ├── test-edge-4-mixed/            # EDGE-4: Selective linking
│   │   ├── tracked.md                # This file IS linked
│   │   └── untracked.txt             # This file is NOT linked
│   ├── test-path-1-config.md         # PATH-1: Config resolution
│   └── test-path-2-cwd.md            # PATH-2: CWD resolution
└── target/                           # Target directory (links created here)
    └── .gitkeep
```

## Testing Workflow

### 1. Dry Run (Preview)
```bash
cd playground
doty link --dry-run
```
Shows what links would be created without actually creating them.

### 2. Create Links
```bash
doty link
```
Creates all symlinks according to doty.kdl configuration.

### 3. Verify Links
```bash
doty status
```
Shows status of all configured links.

### 4. Inspect Results
```bash
# Check target directory structure
ls -la target/

# Verify symlinks
ls -l target/test-lf-1-basic  # Should show symlink
ls -l target/test-lfr-1-single-file.md  # Should show symlink

# Read linked content
cat target/test-lfr-1-single-file.md
```

### 5. Clean Up
```bash
doty clean
```
Removes all created symlinks, leaving target directory clean.

## Path Resolution Strategies

### Config-Based (Default)
```kdl
defaults {
    pathResolution "config"
}
```
- Paths resolved relative to doty.kdl location
- Consistent behavior regardless of execution location
- **Recommended for most use cases**

Example:
```bash
# Works from anywhere
cd /anywhere
doty -c playground/doty.kdl link
```

### CWD-Based (Alternative)
```kdl
defaults {
    pathResolution "cwd"
}
```
- Paths resolved relative to current working directory
- Must run from specific location
- Useful for scripts that change directory

Example:
```bash
# Must run from playground directory
cd playground
doty link
```

## Testing Different Scenarios

### Test LinkFolder vs LinkFilesRecursive
```bash
# After running doty link:

# LinkFolder creates directory symlink
ls -ld target/test-lf-1-basic
# Output: lrwxr-xr-x ... target/test-lf-1-basic -> ../source/test-lf-1-basic

# LinkFilesRecursive creates real directory with file symlinks
ls -ld target/test-lfr-3-recursive
# Output: drwxr-xr-x ... target/test-lfr-3-recursive  (real directory)

ls -l target/test-lfr-3-recursive/README.md
# Output: lrwxr-xr-x ... README.md -> ../../source/test-lfr-3-recursive/README.md
```

### Test Edge Cases
```bash
# Hidden file
ls -la target/.test-hidden

# Deep nesting flattened
ls -l target/test-edge-2-extracted.md

# Multiple symlinks to same source
ls -l target/test-edge-3-copy1.md
ls -l target/test-edge-3-copy2.md

# Selective linking
ls -l target/test-edge-4-mixed/
# Should only show tracked.md, not untracked.txt
```

## Expected Results

After running `doty link`, the target directory should contain:

- **3 directory symlinks** (from LinkFolder tests)
- **2 single file symlinks** (from LFR-1, LFR-2)
- **1 recreated directory structure** with file symlinks (from LFR-3)
- **2 flattened file symlinks** (from LFR-4, EDGE-2)
- **1 nested file symlink** (from LFR-5)
- **1 hidden file symlink** (from EDGE-1)
- **2 duplicate symlinks** to same source (from EDGE-3)
- **1 selective file symlink** (from EDGE-4)
- **1 path resolution test symlink** (from PATH-1)

Total: **14 test cases** covering all strategies and edge cases.

## Troubleshooting

### Links not created
```bash
# Check for errors
doty link

# Verify source files exist
ls -la source/

# Check configuration syntax
cat doty.kdl
```

### Wrong paths
```bash
# Verify pathResolution setting
grep pathResolution doty.kdl

# Ensure running from correct directory (if using cwd mode)
pwd
```

### Clean up failed links
```bash
# Remove all symlinks
doty clean

# Manually remove if needed
rm -rf target/*
```

## Learning Resources

Each test case file contains:
- **What it tests** - Clear description of the test scenario
- **Configuration** - Exact KDL syntax used
- **Expected Result** - What should happen after linking
- **Use Case** - Real-world applications

Read the markdown files in `source/` to understand each test case in detail.

## Contributing Test Cases

To add new test cases:

1. Create source files/directories with descriptive names
2. Add explanatory markdown content
3. Update doty.kdl with new configuration
4. Update this README with test case documentation
5. Test with `doty link --dry-run` first

## Summary

This playground demonstrates:
- ✅ Both linking strategies (LinkFolder & LinkFilesRecursive)
- ✅ Block and inline syntax variants
- ✅ Directory and file linking
- ✅ Nested path handling
- ✅ Hidden file support
- ✅ Multiple targets for same source
- ✅ Selective file linking
- ✅ Path resolution strategies
- ✅ Real-world use cases

Perfect for testing doty functionality and learning how to use it effectively!
