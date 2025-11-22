# Linker Refactoring - Pipeline Architecture

**Date**: 2025-11-22

## Summary

Refactored `gather_link_states` in `src/linker.rs` to use a clear pipeline architecture and removed redundant logic.

## Changes

### 1. Pipeline Architecture
The `gather_link_states` function now orchestrates three distinct steps:
1.  `collect_config_states`: Iterates config packages and populates the map with desired states.
2.  `merge_state_states`: Iterates the state file and merges stored info into the map.
3.  `enrich_with_reality`: Iterates the map and checks the filesystem for actual status.

### 2. Logic Simplification
-   **Removed `explicit_sources` HashSet**: The check for explicit sources was redundant because we are iterating the config packages directly. Any source in `config.packages` is by definition explicit.
-   **Extracted `process_package`**: The logic for expanding packages (handling `LinkFolder` vs `LinkFilesRecursive`) is now isolated in its own method.

## Benefits
-   **Readability**: The high-level flow is immediately obvious.
-   **Maintainability**: Each step is isolated and can be modified independently.
-   **Performance**: Removed an unnecessary pass over the config to build the HashSet.
