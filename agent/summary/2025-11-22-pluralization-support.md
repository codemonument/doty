# 2025-11-22 - Add Pluralization Support

## Context
The user requested proper pluralization for the output messages (e.g., "1 link added" vs "2 links added") instead of the generic "link(s)" style. They specifically asked for a Rust package for this.

## Changes
- Added `pluralizer` crate (v0.5.0) to dependencies.
- Modified `src/commands.rs`:
  - Imported `pluralizer::pluralize`.
  - Updated `link` command summary to use `pluralize("link", count, true)` and `pluralize("warning", count, true)`.
  - Updated `clean` command output to use `pluralize`.

## Verification
- Ran `cargo run -- --config playground/doty.kdl link --dry-run` and verified the output:
  ```
  Summary:
    [+] 1 link added
    [!] 1 warning
    [·] 13 links unchanged
  ```
- Verified that singular and plural forms are correctly generated.
