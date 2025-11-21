# Architecture Update: Path Resolution Strategy & Config Flag

**Date**: 2025-11-21

## Summary

Updated architecture and CLI to use `--config` / `-c` flag instead of `--repo` flag. Added specification for `pathResolution` configuration option. This provides clearer semantics and allows flexible repository structures where the config file can be in a subdirectory.

## Changes to Architecture

### 1. CLI Flag Change: `--repo` → `--config` / `-c`

**Old (Confusing):**
```bash
doty --repo ~/dotfiles/subfolder link
# Problem: Sets subfolder as repo root, not just config location
```

**New (Clear):**
```bash
doty --config ~/dotfiles/subfolder/doty.kdl link
# Clear: Points to config file, pathResolution determines repo root
```

**Rationale:**
- `--repo` was ambiguous when config is in subdirectory
- `--config` clearly specifies config file location
- `pathResolution` setting in config determines how source paths resolve
- Supports flexible repo structures

### 2. New Section: Path Resolution Strategy (3.1)

Added detailed explanation of two resolution strategies:

**`config` (Default)**
- Source paths resolved relative to `doty.kdl` location
- Consistent behavior regardless of where command is run
- Best for most use cases
- **Key benefit**: Config can be in subdirectory, still manage parent directory

**`cwd` (Current Working Directory)**
- Source paths resolved relative to command execution location
- Flexible for running from different locations
- Advanced use case

### 3. Updated Example Config

Added `pathResolution` setting to defaults section with detailed example:

```kdl
defaults {
    pathResolution "config"  // or "cwd"
}
```

Added example scenario showing flexible repo structure:
```
~/dotfiles/
├── configs/
│   └── doty.kdl          # Config in subdirectory
├── nvim/                 # Source files in parent
├── zsh/
└── .doty/state/
```

### 4. Updated CLI Documentation

**Global Options Section:**
- Added `--config <FILE>` / `-c <FILE>` flag
- Default: `./doty.kdl` in current directory
- Can be absolute or relative path

**Config File Discovery:**
1. If `--config` / `-c` specified, use that file
2. Otherwise, search for `doty.kdl` in current directory
3. Error with helpful message if not found

**Path Resolution:**
- Behavior determined by `defaults.pathResolution` in config
- `config` mode: Resolve relative to config file directory
- `cwd` mode: Resolve relative to current working directory
- Allows flexible repo structures

### 5. New Implementation Phase

Added **Phase 2.1: Path Resolution Strategy** with tasks:
- Parse pathResolution from config
- Implement resolution logic
- Add validation
- Write tests
- Update documentation

### 6. Code Changes

**`src/main.rs`:**
- Changed `--repo` flag to `--config` / `-c`
- Updated help text
- Config file discovery logic
- Temporary config_dir_or_cwd derivation (will be enhanced in Phase 2.1)

## Rationale

### Why This Feature?

1. **Flexibility**: Users can choose workflow that fits their needs
2. **Consistency**: Default "config" mode ensures predictable behavior
3. **Compatibility**: "cwd" mode supports Stow-like workflows
4. **Explicit**: Configuration makes behavior clear and documented

### Use Cases

**Scenario 1: Config in Subdirectory (Common)**
```bash
# Repo structure:
~/dotfiles/
├── configs/doty.kdl    # Config in subdirectory
├── nvim/               # Source files in parent
└── zsh/

# With pathResolution "config" (default):
cd ~/
doty -c ~/dotfiles/configs/doty.kdl link
# → nvim resolves to ~/dotfiles/configs/nvim (relative to config)
# This is why we need pathResolution!

# Better: Use "cwd" mode and run from repo root:
cd ~/dotfiles/
doty -c configs/doty.kdl link
# → nvim resolves to ~/dotfiles/nvim (relative to cwd)
```

**Scenario 2: Config at Repo Root (Simple)**
```bash
~/dotfiles/
├── doty.kdl           # Config at root
├── nvim/
└── zsh/

# With pathResolution "config" (default):
cd ~/dotfiles/
doty link              # Finds ./doty.kdl automatically
# → nvim resolves to ~/dotfiles/nvim

cd ~/
doty -c ~/dotfiles/doty.kdl link
# → Same result, works from anywhere
```

**Scenario 3: Multiple Configs (Advanced)**
```bash
~/dotfiles/
├── work/
│   ├── doty.kdl
│   └── nvim/
└── personal/
    ├── doty.kdl
    └── nvim/

# With pathResolution "cwd":
cd ~/dotfiles/work/
doty link              # Uses work/doty.kdl, resolves to work/nvim

cd ~/dotfiles/personal/
doty link              # Uses personal/doty.kdl, resolves to personal/nvim
```

## Implementation Notes

### Config Struct Changes

```rust
pub struct Defaults {
    pub path_resolution: PathResolution,
}

pub enum PathResolution {
    Config,  // Default
    Cwd,
}
```

### CLI Logic

1. Determine config file path:
   - If `--config` / `-c` provided → use that
   - Otherwise → search for `doty.kdl` in current directory
2. Parse `defaults.pathResolution` from config
3. Apply resolution strategy:
   - `config`: Use parent directory of `doty.kdl`
   - `cwd`: Use `std::env::current_dir()`

### Validation

- Only accept "config" or "cwd" values
- Default to "config" if not specified
- Error on invalid values

## Next Steps

Implement Phase 2.1:
1. Update config parser to handle defaults section
2. Add PathResolution enum and logic
3. Integrate with commands module
4. Add comprehensive tests
5. Update CLI help text

## Related Files

- `agent/ARCHITECTURE.md` - Updated with new specification
- `src/config.rs` - Will need Defaults struct
- `src/commands.rs` - Will need resolution logic
- `src/main.rs` - CLI integration
