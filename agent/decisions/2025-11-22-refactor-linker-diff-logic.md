# Linker Refactoring - Diff Logic Update

**Date**: 2025-11-22

## Summary

Refactored the `Linker::calculate_diff` logic to use a more coherent `LinkStatus` struct, as requested. This struct provides a unified view of the "Config" (Desired), "State" (Stored), and "Filesystem" (Reality) for each target path.

## Changes

### 1. Renamed `TargetStatus` to `LinkStatus`

The struct now clearly separates the three sources of truth:

```rust
struct LinkStatus {
    // Config (Desired)
    config_resolved_source: Option<Utf8PathBuf>,
    config_resolved_target: Option<Utf8PathBuf>,
    config_is_explicit: bool,

    // State (Stored)
    state_resolved_source: Option<Utf8PathBuf>,
    state_resolved_target: Option<Utf8PathBuf>,

    // Filesystem (Reality)
    source_exists: bool,
    target_exists: bool,
    target_type: Option<FsType>,
    target_points_to: Option<Utf8PathBuf>,
}
```

### 2. Updated Logic

-   **Gather Phase**: `gather_link_statuses` populates the `LinkStatus` struct.
-   **Decision Phase**: `determine_actions` uses the new field names to decide on actions.
-   **Exists Check**: Clarified that `source_exists` refers to the *Configured Source* (the one we want to link), and `target_exists` refers to the *Filesystem Target*.

## Benefits

-   **Coherence**: The naming convention (`config_`, `state_`, `fs_`) makes it immediately obvious where each piece of data comes from.
-   **Completeness**: The struct now holds all necessary information to make linking decisions without further I/O.
