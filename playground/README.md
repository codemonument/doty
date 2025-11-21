# Doty Playground

This directory contains a comprehensive test setup for **doty**, demonstrating all possible linking combinations and strategies.

## Directory Structure

```
playground/
├── doty.kdl               # Comprehensive configuration file
├── test.sh                # Test script with examples
├── source/                # Source files and directories
│   ├── .hidden           # Hidden file (edge case)
│   ├── config.toml       # Single configuration file
│   ├── configs/          # Configuration directory
│   │   ├── dev.toml      # Development config
│   │   └── prod.toml     # Production config
│   ├── docs/             # Documentation directory
│   │   ├── api.md        # API documentation
│   │   └── nested/       # Nested directory
│   │       └── guide.md  # Nested documentation
│   ├── images/           # Images directory
│   │   ├── banner.jpg    # Banner image
│   │   └── logo.png      # Logo image
│   ├── root.txt          # Root level file
│   └── scripts/          # Scripts directory
│       ├── build.sh      # Build script
│       └── deploy.sh     # Deploy script
└── target/               # Target directory (where links will be created)
```

## Linking Strategies Demonstrated

### 1. LinkFolder Strategy (Stow-like)
Creates a single symbolic link for the entire directory:

```kdl
LinkFolder "configs" {
    target "../target/configs"
    description "Link entire configs directory as single symlink"
}
```

**Use Cases:**
- Application configurations where you want to track ALL files
- Directories where new files should be immediately tracked
- "What you see is what you get" behavior

### 2. LinkFilesRecursive Strategy (Dotter-like)
Recreates directory structure and symlinks individual files:

```kdl
LinkFilesRecursive "config.toml" {
    target "../target/config.toml"
    description "Link single configuration file"
}
```

**Use Cases:**
- Complex configurations with mixed ownership
- Applications that dislike symlinked directory roots
- When you only want to manage specific files in a shared folder

## Path Resolution Strategies

### Config-Based Resolution (Default)
```kdl
defaults {
    pathResolution "config"  // Resolve paths relative to doty.kdl location
}
```

- Source paths are relative to the directory containing `doty.kdl`
- Consistent behavior regardless of where command is run
- Allows config file in subdirectory while managing parent directory files

### Current Working Directory Resolution
```kdl
defaults {
    pathResolution "cwd"  // Resolve paths relative to current working directory
}
```

- Source paths are relative to where `doty` command is executed
- Useful when running from different locations with same config

## Testing Scenarios

The configuration includes various edge cases and scenarios:

1. **Basic Operations**: Simple file and directory linking
2. **Nested Paths**: Deep directory structures
3. **Hidden Files**: Handling of dotfiles
4. **Name Changes**: Source and target with different names
5. **Multiple Targets**: Same source to different targets
6. **Complex Nesting**: Deep nested file structures

## Usage Examples

### Basic Testing
```bash
# From playground directory (no -c flag needed!)
cd playground

# Dry run to see what would be linked
doty link --dry-run

# Actually create the links
doty link

# Clean up all created links
doty clean

# Check status
doty status
```

### Testing from Outside Playground
```bash
# From project root or anywhere else
doty -c playground/doty.kdl link --dry-run
doty -c playground/doty.kdl link
doty -c playground/doty.kdl clean
```

### Path Resolution Testing
```bash
# With pathResolution="config" (default) - run from playground/
cd playground
doty link

# With pathResolution="cwd" - change config first, then run
# Edit doty.kdl: change pathResolution to "cwd"
cd playground
doty link
```

## Running the Test Script

The included `test.sh` script provides a comprehensive overview:

```bash
cd playground
./test.sh
```

This will:
- Show the current directory structure
- Display the configuration file
- Provide example commands for testing
- Explain path resolution strategies

## Expected Behavior

After running `doty link`, the `target/` directory should contain:

- **Symlinks to directories** (from LinkFolder strategy)
- **Recreated directory structures** with individual file symlinks (from LinkFilesRecursive strategy)
- **Proper handling** of hidden files, nested structures, and name changes

This setup allows you to verify that doty correctly handles:
- Both linking strategies
- Path resolution (config vs cwd)
- Edge cases (hidden files, deep nesting)
- Complex directory structures
- Multiple linking scenarios