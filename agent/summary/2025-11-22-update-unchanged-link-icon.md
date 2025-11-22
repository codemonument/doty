# 2025-11-22 - Update Unchanged Link Icon

## Context
The user requested to update the icon for unchanged files in the summary output from `·` to `[·]`.

## Changes
- Modified `src/commands.rs`:
  - Updated the summary output for unchanged links to use `[·]` instead of `·`.

## Verification
- Ran `cargo run -- --config playground/doty.kdl link --dry-run` and verified that the summary now shows `[·] 13 link(s) unchanged`.
