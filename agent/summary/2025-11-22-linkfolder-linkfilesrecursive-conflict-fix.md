# LinkFolder/LinkFilesRecursive Conflict Detection Fix

**Date**: 2025-11-22  
**Status**: ✅ Fixed  
**Tests**: 35/35 passing

## Problem

When running the playground configuration, a **critical bug** was discovered:

### The Bug
`LinkFilesRecursive` could create symlinks inside directories that were already symlinked by `LinkFolder`, resulting in:
1. **Circular symlinks** in the source directory
2. **Corrupted source files** (symlinks pointing to themselves)
3. **Silent failures** (no error, but broken behavior)

### Example Scenario
```kdl
// First: LinkFolder creates a directory symlink
LinkFolder "source/docs" {
    target "target/documentation"
}
// Result: target/documentation → source/docs

// Then: LinkFilesRecursive tries to create a file symlink inside
LinkFilesRecursive "source/docs/nested/guide.md" {
    target "target/documentation/nested/guide.md"
}
// Problem: target/documentation is a symlink!
// So this actually creates: source/docs/nested/guide.md → source/docs/nested/guide.md
// (circular symlink in SOURCE directory!)
```

### Actual Damage Found
In the playground, `source/docs/nested/guide.md` was corrupted:
```bash
$ readlink source/docs/nested/guide.md
/Users/.../source/docs/nested/guide.md  # Points to itself!
```

## Solution

Added **conflict detection** in the linker to prevent creating symlinks inside already-symlinked directories:

### Implementation
1. **New helper method**: `check_target_path_conflicts()`
   - Walks up the target path checking each parent
   - Detects if any parent is a symlink
   - Returns error with helpful message

2. **Integration points**:
   - `link_files_recursive()`: Check before processing
   - `link_directory_recursive()`: Check before creating directories

3. **Error message**:
   ```
   Cannot create LinkFilesRecursive at 'target/documentation/nested/guide.md': 
   Parent directory '/path/to/target/documentation' is a symlink (created by LinkFolder)
   This usually happens when a parent directory is already managed by LinkFolder.
   Consider using only LinkFolder or only LinkFilesRecursive for overlapping paths.
   ```

### Code Changes
**File**: `src/linker.rs`

1. Added `check_target_path_conflicts()` method (lines ~260-280)
2. Modified `link_files_recursive()` to call conflict check (lines ~120-135)
3. Modified `link_directory_recursive()` to call conflict check (lines ~171-185)
4. Added test case `test_conflict_linkfolder_and_linkfilesrecursive()` (lines ~620-660)

## Testing

### New Test Case
```rust
#[test]
fn test_conflict_linkfolder_and_linkfilesrecursive() {
    // 1. Create LinkFolder symlink
    // 2. Try to create LinkFilesRecursive inside it
    // 3. Verify error is raised with correct message
}
```

### Test Results
- **Before fix**: Would create circular symlink (bug)
- **After fix**: Raises error with helpful message ✅
- **All tests**: 35/35 passing ✅

## Verification

### Playground Test
```bash
$ cd playground && ../target/release/doty link
...
LinkFilesRecursive source/docs/nested/guide.md → target/documentation/nested/guide.md
Error: Failed to link: source/docs/nested/guide.md

Caused by:
    Cannot create LinkFilesRecursive at 'target/documentation/nested/guide.md': 
    Parent directory '.../target/documentation' is a symlink (created by LinkFolder)
    ...
```

✅ **Conflict detected and prevented!**

## User Guidance

When users encounter this error, they should:

1. **Review their config** for overlapping paths
2. **Choose one strategy**:
   - Use `LinkFolder` for the parent directory (simpler, tracks all files)
   - Use `LinkFilesRecursive` for specific files (more granular control)
3. **Avoid mixing** `LinkFolder` and `LinkFilesRecursive` for overlapping paths

### Example Fix
**Before** (causes conflict):
```kdl
LinkFolder "source/docs" target="target/documentation"
LinkFilesRecursive "source/docs/nested/guide.md" target="target/documentation/nested/guide.md"
```

**After** (option 1 - use LinkFolder only):
```kdl
LinkFolder "source/docs" target="target/documentation"
// Remove the LinkFilesRecursive - LinkFolder handles everything
```

**After** (option 2 - use LinkFilesRecursive only):
```kdl
// Remove LinkFolder, use LinkFilesRecursive for specific files
LinkFilesRecursive "source/docs/api.md" target="target/documentation/api.md"
LinkFilesRecursive "source/docs/nested/guide.md" target="target/documentation/nested/guide.md"
```

## Impact

### Security
- ✅ Prevents source directory corruption
- ✅ Prevents circular symlinks
- ✅ Prevents silent failures

### User Experience
- ✅ Clear error messages
- ✅ Helpful guidance on how to fix
- ✅ Fails fast (before creating broken symlinks)

### Backward Compatibility
- ⚠️ **Breaking change**: Configs with overlapping paths will now fail
- ✅ **Good thing**: Previously broken configs are now caught early
- ✅ **Migration**: Users get clear error messages explaining the issue

## Next Steps

1. ✅ Fix implemented
2. ✅ Tests added
3. ✅ Documentation written
4. 🔲 Update playground config to remove conflicting entries
5. 🔲 Consider adding a `--force` flag for advanced users (future)

## Files Modified

- `src/linker.rs`: Added conflict detection logic and test
- `agent/summary/2025-11-22-linkfolder-linkfilesrecursive-conflict-fix.md`: This document
