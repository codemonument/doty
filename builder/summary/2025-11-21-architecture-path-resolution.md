# Architecture Update: Path Resolution Strategy

**Date**: 2025-11-21

## Summary

Added specification for `pathResolution` configuration option to the ARCHITECTURE.md. This feature allows users to control how source paths are resolved - either relative to the config file location or relative to the current working directory.

## Changes to Architecture

### 1. New Section: Path Resolution Strategy (3.1)

Added detailed explanation of two resolution strategies:

**`config` (Default)**
- Source paths resolved relative to `doty.kdl` location
- Consistent behavior regardless of where command is run
- Best for most use cases

**`cwd` (Current Working Directory)**
- Source paths resolved relative to command execution location
- Flexible for running from different locations
- Advanced use case

### 2. Updated Example Config

Added `pathResolution` setting to defaults section:

```kdl
defaults {
    pathResolution "config"  // or "cwd"
}
```

### 3. Updated CLI Documentation

Clarified interaction between `--repo` flag and `pathResolution`:
- `--repo` flag overrides pathResolution strategy
- Without `--repo`, behavior follows config setting
- Default is "config" if not specified

### 4. New Implementation Phase

Added **Phase 2.1: Path Resolution Strategy** with tasks:
- Parse pathResolution from config
- Implement resolution logic
- Add validation
- Write tests
- Update documentation

## Rationale

### Why This Feature?

1. **Flexibility**: Users can choose workflow that fits their needs
2. **Consistency**: Default "config" mode ensures predictable behavior
3. **Compatibility**: "cwd" mode supports Stow-like workflows
4. **Explicit**: Configuration makes behavior clear and documented

### Use Cases

**Config Mode (Default):**
```bash
# Always works the same, regardless of location
cd ~/
doty link  # Uses ~/dotfiles/doty.kdl as reference

cd ~/dotfiles/
doty link  # Same behavior
```

**CWD Mode:**
```bash
# Flexible for different project structures
cd ~/dotfiles/work/
doty link  # Uses work/ as root

cd ~/dotfiles/personal/
doty link  # Uses personal/ as root
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

1. Check if `--repo` flag provided → use that
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
