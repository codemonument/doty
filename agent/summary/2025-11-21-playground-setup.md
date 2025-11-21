# Playground Setup - 2025-11-21

## Summary
Created comprehensive test playground for doty with all linking combinations and strategies.

## Changes Made

### 1. Directory Structure
```
playground/
├── doty.kdl               # Main config (renamed from doty-test.kdl)
├── README.md              # Comprehensive documentation
├── test.sh                # Test script with examples
├── source/                # Source files
│   ├── .hidden
│   ├── config.toml
│   ├── configs/           # dev.toml, prod.toml
│   ├── docs/              # api.md, nested/guide.md
│   ├── images/            # logo.png, banner.jpg
│   ├── root.txt
│   └── scripts/           # build.sh, deploy.sh
└── target/                # Empty target directory
```

### 2. Configuration (doty.kdl)
Cleaned up and organized to demonstrate:

**LinkFolder Strategy (Stow-like):**
- Basic block syntax: `LinkFolder "source/configs" { target "target/.config/app" }`
- Single line syntax: `LinkFolder "source/scripts" target="target/bin"`
- Different target names

**LinkFilesRecursive Strategy (Dotter-like):**
- Single file linking (block and single-line syntax)
- Recursive directory linking
- Nested path to flat target
- Nested path to nested path
- Hidden files
- Deep nesting scenarios
- Same source to multiple targets

**Path Resolution:**
- Default: `pathResolution "config"` (relative to doty.kdl location)
- Alternative: `pathResolution "cwd"` (relative to current working directory)

### 3. Key Improvements
- **Removed duplicates**: Original config had duplicate entries for configs, scripts, and config.toml
- **Fixed paths**: All source paths now properly prefixed with "source/"
- **Simplified targets**: Targets now relative to playground (no "../" needed when running from playground/)
- **Better organization**: Clear sections for each strategy and use case
- **Proper naming**: Renamed to doty.kdl for automatic discovery

## Usage

### From Playground Directory (Recommended)
```bash
cd playground
doty link --dry-run    # No -c flag needed!
doty link
doty clean
```

### From Any Directory
```bash
doty -c playground/doty.kdl link --dry-run
doty -c playground/doty.kdl link
```

## Architecture Compliance

✅ **LinkFolder Strategy**: Covers all cases from ARCHITECTURE.md
- Single symlink for entire directory
- Block and single-line syntax
- Different target naming

✅ **LinkFilesRecursive Strategy**: Covers all cases from ARCHITECTURE.md
- Single file linking
- Recursive directory linking
- Nested paths
- Hidden files
- Complex nesting

✅ **Path Resolution**: Both strategies documented
- `config` (default): Relative to doty.kdl location
- `cwd`: Relative to current working directory

## Testing Scenarios Covered
1. Basic operations (file and directory linking)
2. Edge cases (hidden files, deep nesting)
3. Name changes (different source and target names)
4. Multiple targets (same source to different locations)
5. Complex structures (multi-level directories)
