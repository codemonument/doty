# 2025-11-22 - Link Command Output Cleanup

## Context
The user requested to clean up the `link` command output by hiding unchanged links (skipped actions) from the detailed view and only showing them in the summary at the end.

## Changes
- Modified `src/commands.rs`:
  - In the `link` function, filtered out `LinkAction::Skipped` from the `package_actions` loop.
  - Added a check to skip printing the package header if all actions for that package are skipped.
  - The summary at the end still counts and displays the number of unchanged links.

## Verification
- Ran `cargo run -- --config playground/doty.kdl link --dry-run` and verified that:
  - Unchanged links (marked with `·`) are no longer listed individually.
  - Packages with only unchanged links are not listed.
  - Changed links (added, removed, warnings) are still listed.
  - The summary correctly reports the number of unchanged links.
- Ran `cargo test` to ensure no regressions.
