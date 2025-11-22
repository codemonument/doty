# Fix Broken Symlink Removal in Detect Command

## Context
The `detect` command failed to remove broken symlinks when the target path was relative and the current working directory was different from the config directory (or wherever the relative path was valid from).
The error was `No such file or directory (os error 2)`.

## Root Cause
The `Scanner` was returning `DriftItem`s with `target_path` as it appeared in the config or state (often relative).
The `detect` command then tried to `remove_file` using this relative path, which failed if the CWD was not the expected base directory.
Additionally, `Scanner` was checking for broken symlinks using relative paths from `state.links` without resolving them against `config_dir`, which could lead to false negatives (failing to detect broken links) or false positives (detecting valid links as broken if checked from wrong CWD) if CWD != config_dir.

## Solution
Modified `src/scanner.rs` to:
1.  Resolve `state_target` to an absolute path before checking if it's a broken symlink in `scan_targets`.
2.  Ensure `DriftItem.target_path` is always populated with the resolved absolute path in `scan_package` and `scan_targets`.

## Impact
-   `doty detect` will now correctly identify and remove broken symlinks regardless of CWD.
-   Output of `doty detect` will now show absolute paths for broken symlinks (consistent with untracked files).
