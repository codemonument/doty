# Lockfile Format Improvements

**Date**: 2025-11-22

## Changes

Updated the state file (lockfile) format with three key improvements:

### 1. Source Before Target
- Changed link entries from `link target="..." source="..."` to `link source="..." target="..."`
- More intuitive: reads as "link source to target" (left-to-right)
- Matches common mental model of symlink operations

### 2. Added basePath Field
- Stores the absolute path to config_dir_or_cwd
- Enables future path recalculation if needed
- Format: `basePath "/absolute/path/to/config/dir"`
- Stored as second line after lockfileVersion

### 3. Added lockfileVersion Field
- Set to `1` for current format
- Enables future format migrations
- Format: `lockfileVersion 1`
- Stored as first line in file

## Example Lockfile

```kdl
lockfileVersion 1
basePath "/Users/bjesuiter/Develop/codemonument/doty/playground"
link source="source/test-lf-1-basic" target="target/test-lf-1-basic"
link source="source/test-lfr-1-single-file.md" target="target/test-lfr-1-single-file.md"
```

## Implementation Details

### Modified Files
- `src/state.rs`: Updated `DotyState` struct and serialization/deserialization
- `src/commands.rs`: Pass basePath to state operations, canonicalize paths
- `src/linker.rs`: Updated test cases

### Key Changes
1. **DotyState struct**: Added `lockfile_version: u32` and `base_path: Utf8PathBuf`
2. **Constructor**: `DotyState::new()` now requires `base_path` parameter
3. **Serialization**: `to_kdl()` outputs version and basePath before links
4. **Deserialization**: `from_str()` parses version and basePath nodes
5. **Path canonicalization**: Config dir paths are now canonicalized to absolute paths

### Backward Compatibility
- Old lockfiles without version/basePath will default to version 1 and "." as basePath
- Parser handles both old and new formats gracefully

## Testing
- ✅ All 35 tests pass
- ✅ Playground test confirms correct format
- ✅ Roundtrip serialization/deserialization works
