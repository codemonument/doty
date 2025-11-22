# Linker Refactoring - Completed

**Date**: 2025-11-22

## Summary

Refactored the `Linker::calculate_diff` logic to separate the "Gathering" phase from the "Decision" phase. This simplifies the logic, improves maintainability, and makes the decision process more transparent.

## Changes

### 1. New Data Structure: `TargetStatus`

Introduced `TargetStatus` struct to hold all relevant information for a target path:
-   **Desired State**: Source path from config, explicit vs implicit.
-   **Stored State**: Source path from state file.
-   **Reality**: Actual target on disk, is it a symlink, where does it point.

### 2. Two-Phase Algorithm

1.  **Gather Phase (`gather_target_statuses`)**:
    -   Iterates Config to build desired state (expanding recursive folders).
    -   Iterates State to build stored state.
    -   Checks Filesystem to build reality state.
    -   Returns `HashMap<Utf8PathBuf, TargetStatus>`.

2.  **Decision Phase (`determine_actions`)**:
    -   Iterates over the gathered statuses.
    -   Applies a pure function to determine `LinkAction` (Created, Updated, Removed, Skipped, Warning).
    -   Handles edge cases like missing sources or broken links.

### 3. Code Cleanup

-   Removed unused methods `check_target_path_conflicts` and `is_symlink_to` (logic moved to gather phase).
-   Fixed `resolve_target_path` implementation.

## Benefits

-   **Clarity**: The logic for "what to do" is now centralized in one match statement in `determine_actions`.
-   **Robustness**: All information is gathered upfront, reducing the risk of partial state updates or inconsistent checks.
-   **Testability**: The decision logic can be tested independently of the filesystem (in future tests).
