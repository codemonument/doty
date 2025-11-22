# Improved Broken Symlink Reporting

## Context
The user requested better visualization for broken symlinks in the `doty detect` command output.
Specifically:
-   Add a chain icon (🔗) in front of the symlink path.
-   Show an arrow (→) pointing to the target.
-   Show the target path (where the symlink points).
-   Add an icon for "item on disk" (file 📄) in front of the physical HDD path (the target).

## Changes
1.  **`src/scanner.rs`**:
    -   Updated `DriftItem` struct to include `symlink_target: Option<Utf8PathBuf>`.
    -   Updated `scan_targets` and `scan_package` to populate `symlink_target` using `std::fs::read_link` when a broken symlink is detected.
    -   This allows us to know where the broken link was pointing to.

2.  **`src/commands.rs`**:
    -   Updated `detect` command to store the full `DriftItem` in `broken_links` instead of just the path.
    -   Updated the printing logic for broken symlinks to match the requested format:
        ```
        [!] 🔗 <symlink_path> → 📄 <target_path>
        ```
    -   Updated the removal logic to access `item.target_path` since `broken_links` now contains `DriftItem`s.

## Impact
-   `doty detect` output is now more informative and visually distinct.
-   Users can see exactly where a broken symlink is pointing, which helps in debugging why it's broken.
