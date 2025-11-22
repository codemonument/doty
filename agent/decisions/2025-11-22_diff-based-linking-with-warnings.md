# Diff-Based Linking with Warnings

**Date**: 2025-11-22  
**Status**: Approved  
**Context**: Fix crash when source files are renamed/deleted, implement proper state-config diff

## Problem Statement

Current implementation crashes when a source file referenced in config doesn't exist:
```
Error: Failed to link: source/test-lfr-1-single-file.md
Caused by:
    Source path does not exist: /Users/.../source/test-lfr-1-single-file.md
```

Additionally, the system doesn't properly handle:
- Renamed source files
- Deleted files from `LinkFilesRecursive` folders
- Packages removed from config (orphaned links in state)
- State-config drift detection

## Core Principle: Explicit vs Implicit Links

### Explicit Links
Directly specified in config:
- `LinkFolder "source/folder"` → 1 explicit link for the folder
- `LinkFilesRecursive "source/file.md"` → 1 explicit link for that file
- `LinkFilesRecursive "source/folder"` → 1 explicit config entry, N implicit state entries

### Implicit Links
Created by `LinkFilesRecursive` when scanning directories:
- Not directly in config, derived from parent directory entry
- Example: Config has `"source/folder"`, creates links for `source/folder/file1.md`, `source/folder/file2.md`

## Decision: Scenarios and Behaviors

### Scenario 1: Explicit link - source deleted, config NOT updated
- **Config**: `LinkFilesRecursive "source/old-name.md" target="target/link.md"`
- **State**: `source/old-name.md` → `target/link.md`
- **Reality**: `source/old-name.md` doesn't exist
- **Action**: `[!]` Warning
- **Message**: "Source file gone, remove from config if intentional"
- **Behavior**: Do NOT remove, do NOT crash, just warn and skip
- **State**: Keep unchanged

### Scenario 1b: Implicit link - file deleted from folder
- **Config**: `LinkFilesRecursive "source/folder" target="target/folder"`
- **State**: `source/folder/file2.md` → `target/folder/file2.md`
- **Reality**: `source/folder/file2.md` deleted
- **Action**: `[-]` Schedule removal
- **Behavior**: Remove symlink, remove from state
- **Reason**: Implicit link, can't warn to "remove from config"

### Scenario 2: Link in state but NOT in config (abandoned)
- **Config**: (no entry)
- **State**: `source/old.md` → `target/old.md`
- **Action**: `[-]` Schedule removal
- **Behavior**: Remove symlink, remove from state

### Scenario 3: Link in config but NOT in state (new)
- **Config**: `LinkFilesRecursive "source/new.md" target="target/new.md"`
- **State**: (no entry)
- **Reality**: `source/new.md` exists
- **Action**: `[+]` Schedule creation
- **Behavior**: Create symlink, add to state

### Scenario 4: Link in both, same source, symlink correct
- **Config**: `LinkFilesRecursive "source/file.md" target="target/file.md"`
- **State**: `source/file.md` → `target/file.md`
- **Reality**: Symlink correct
- **Action**: Skip (no output)
- **Behavior**: No changes

### Scenario 5: Link in both, different source
- **Config**: `LinkFilesRecursive "source/new.md" target="target/link.md"`
- **State**: `source/old.md` → `target/link.md`
- **Reality**: `source/new.md` exists
- **Action**: `[~]` Schedule update
- **Behavior**: Update symlink, update state

## Implementation Design

### 1. Add `LinkAction::Warning` Variant

```rust
pub enum LinkAction {
    Created { target: Utf8PathBuf, source: Utf8PathBuf },
    Updated { target: Utf8PathBuf, old_source: Utf8PathBuf, new_source: Utf8PathBuf },
    Skipped { target: Utf8PathBuf, source: Utf8PathBuf },
    Removed { target: Utf8PathBuf, source: Utf8PathBuf },
    Warning { target: Utf8PathBuf, source: Utf8PathBuf, message: String },  // NEW
}
```

### 2. Add `--force` Flag

```rust
// In CLI args
#[derive(Parser)]
pub struct LinkArgs {
    #[arg(long)]
    dry_run: bool,
    
    #[arg(long)]
    force: bool,  // NEW: Treat warnings as removals
}
```

**Behavior**: When `--force` is set, warnings become removals:
- Broken explicit links are removed instead of warned
- Useful for automation/CI pipelines

### 3. Module Placement Decision

**Decision**: Diff calculation will be implemented in `linker.rs`, not `commands.rs`.

**Rationale**:
1. **Shared Helpers**: Diff calculation needs the same helpers as execution (path resolution, symlink checking)
2. **Cohesion**: Diff and execution are both about "managing links" - they belong together
3. **Avoid Duplication**: Keeping helpers private in linker avoids code duplication
4. **Clean API**: Linker exposes `calculate_diff()` and `execute_action()` as public methods
5. **Future-Proof**: Can extract to separate `diff.rs` module later if linker grows too large

**Alternatives Considered**:
- **commands.rs**: Would require making linker helpers public or duplicating logic
- **Separate diff.rs**: Would need to share helpers via new `path_utils.rs` module (over-engineering)

### 4. Diff Calculation Algorithm

Located in `linker.rs::calculate_diff()`:

```rust
// Step 1: Build desired links from config
let mut desired_links: HashMap<Utf8PathBuf, Utf8PathBuf> = HashMap::new();
let mut explicit_sources: HashSet<Utf8PathBuf> = HashSet::new();

for package in &config.packages {
    let source_path = config_dir_or_cwd.join(&package.source);
    
    if source_path.exists() {
        if source_path.is_file() {
            // Single file link (explicit)
            desired_links.insert(package.target.clone(), package.source.clone());
            explicit_sources.insert(package.source.clone());
        } else if source_path.is_dir() {
            // Directory link
            match package.strategy {
                LinkStrategy::LinkFolder => {
                    // Folder itself is explicit
                    desired_links.insert(package.target.clone(), package.source.clone());
                    explicit_sources.insert(package.source.clone());
                }
                LinkStrategy::LinkFilesRecursive => {
                    // Folder is explicit, but files inside are implicit
                    explicit_sources.insert(package.source.clone());
                    // Scan and add all files (these are implicit)
                    for file in scan_directory_recursive(&source_path) {
                        let relative = file.strip_prefix(&source_path)?;
                        let target_path = resolve_target_path(&package.target)?.join(relative);
                        let source_rel = package.source.join(relative);
                        desired_links.insert(target_path, source_rel);
                    }
                }
            }
        }
    } else {
        // Source doesn't exist - check if explicit
        if is_explicit(&package.source, &explicit_sources) {
            if force {
                // Treat as removal
                if let Some(source) = state.links.get(&package.target) {
                    actions.push(LinkAction::Removed {
                        target: package.target.clone(),
                        source: source.clone(),
                    });
                }
            } else {
                // Warn
                actions.push(LinkAction::Warning {
                    target: package.target.clone(),
                    source: package.source.clone(),
                    message: "Source file gone, remove from config if intentional".to_string(),
                });
            }
        }
    }
}

// Step 2: Find links to remove (in state but not in desired)
for (target, source) in &state.links {
    if !desired_links.contains_key(target) {
        actions.push(LinkAction::Removed {
            target: target.clone(),
            source: source.clone(),
        });
    }
}

// Step 3: Find links to create/update/skip
for (target, source) in &desired_links {
    if let Some(old_source) = state.links.get(target) {
        if old_source != source {
            // Source changed
            actions.push(LinkAction::Updated {
                target: target.clone(),
                old_source: old_source.clone(),
                new_source: source.clone(),
            });
        } else {
            // Check if symlink is correct
            let target_path = linker.resolve_target_path(target)?;
            let source_path = config_dir_or_cwd.join(source);
            if linker.is_symlink_to(&target_path, &source_path)? {
                actions.push(LinkAction::Skipped {
                    target: target.clone(),
                    source: source.clone(),
                });
            } else {
                // Symlink broken or incorrect, recreate
                actions.push(LinkAction::Created {
                    target: target.clone(),
                    source: source.clone(),
                });
            }
        }
    } else {
        // New link
        actions.push(LinkAction::Created {
            target: target.clone(),
            source: source.clone(),
        });
    }
}
```

### 5. Helper Function: `is_explicit()`

```rust
fn is_explicit(source: &Utf8Path, explicit_sources: &HashSet<Utf8PathBuf>) -> bool {
    // A source is explicit if it exactly matches an entry in explicit_sources
    // OR if it's a parent directory in explicit_sources (for LinkFolder)
    explicit_sources.contains(source)
}
```

### 6. Output Format

Group by package, show warnings with `[!]` symbol:

```
LinkFilesRecursive source/folder → target/folder
  [-] target/folder/file2.md → source/folder/file2.md

LinkFilesRecursive source/explicit.md → target/explicit.md
  [!] target/explicit.md → source/explicit.md
      Warning: Source file gone, remove from config if intentional

Orphaned links:
  [-] target/old-removed.md → source/old-removed.md

Summary:
  [+] 0 link(s) added
  [~] 0 link(s) updated
  [-] 2 link(s) removed
  [!] 1 warning(s)
  · 1 link(s) unchanged
```

### 7. Linker API Design

The linker will have a clean separation between calculation and execution:

**Public API**:
```rust
impl Linker {
    /// Calculate what actions are needed to sync config with state
    pub fn calculate_diff(
        &self,
        config: &DotyConfig,
        state: &DotyState,
        force: bool,
    ) -> Result<Vec<LinkAction>> {
        // All diff logic here
        // Returns list of actions to perform
    }
    
    /// Execute a single action
    pub fn execute_action(&self, action: &LinkAction, dry_run: bool) -> Result<()> {
        match action {
            LinkAction::Created { source, target } => self.create_link(source, target, dry_run),
            LinkAction::Removed { target, .. } => self.remove_link(target, dry_run),
            LinkAction::Updated { target, new_source, .. } => {
                self.remove_link(target, dry_run)?;
                self.create_link(new_source, target, dry_run)
            }
            LinkAction::Warning { .. } | LinkAction::Skipped { .. } => Ok(()),
        }
    }
}
```

**Private Helpers** (used by both diff calculation and execution):
- `resolve_target_path()` - Path resolution logic
- `is_symlink_to()` - Symlink verification
- `create_link()` - Create symlink
- `remove_link()` - Remove symlink
- `scan_directory_recursive()` - Directory scanning
- `check_target_path_conflicts()` - Conflict detection

**Changes from Current Implementation**:
- Remove `link_package()` and `link_folder()` methods (replaced by `calculate_diff()`)
- Remove source existence checks that cause crashes
- Keep all helpers private (no need to expose them)

### 8. Usage in commands.rs

```rust
pub fn link(config_path: Utf8PathBuf, dry_run: bool, force: bool) -> Result<()> {
    let config = DotyConfig::from_file(&config_path)?;
    let mut state = DotyState::load(...)?;
    let linker = Linker::new(config_dir_or_cwd, config.path_resolution);
    
    // Calculate diff
    let actions = linker.calculate_diff(&config, &state, force)?;
    
    // Group actions by package for output
    // (implementation detail)
    
    // Execute actions and update state
    for action in &actions {
        // Print action
        match action {
            LinkAction::Created { target, source } => {
                println!("  {} {} → {}", "[+]".green().bold(), target, source);
            }
            LinkAction::Warning { target, source, message } => {
                println!("  {} {} → {}", "[!]".yellow().bold(), target, source);
                println!("      Warning: {}", message);
            }
            // ... other actions
        }
        
        // Execute
        linker.execute_action(action, dry_run)?;
        
        // Update state
        if !dry_run {
            match action {
                LinkAction::Created { target, source } => state.add_link(target.clone(), source.clone()),
                LinkAction::Updated { target, new_source, .. } => state.add_link(target.clone(), new_source.clone()),
                LinkAction::Removed { target, .. } => state.remove_link(target),
                _ => {}
            }
        }
    }
    
    // Save state
    if !dry_run {
        state.save(&state_dir)?;
    }
    
    Ok(())
}
```

### 9. State Update Logic

```rust
// After calculating all actions, apply them
for action in &actions {
    match action {
        LinkAction::Created { target, source } => {
            if !dry_run {
                linker.create_link(source, target, false)?;
                state.add_link(target.clone(), source.clone());
            }
        }
        LinkAction::Updated { target, new_source, .. } => {
            if !dry_run {
                linker.remove_link(target, false)?;
                linker.create_link(new_source, target, false)?;
                state.add_link(target.clone(), new_source.clone());
            }
        }
        LinkAction::Removed { target, .. } => {
            if !dry_run {
                linker.remove_link(target, false)?;
                state.remove_link(target);
            }
        }
        LinkAction::Warning { .. } => {
            // Don't modify state for warnings
        }
        LinkAction::Skipped { .. } => {
            // No changes needed
        }
    }
}
```

## Migration Path

### Phase 1: Add Warning Support
- Add `LinkAction::Warning` variant
- Update output formatting to handle warnings
- Add `--force` flag

### Phase 2: Implement Diff Calculation in Linker
- Add `calculate_diff()` method to linker
- Implement explicit/implicit detection
- Handle all 5 scenarios
- Remove old `link_package()` methods

### Phase 3: Add Execution Method
- Add `execute_action()` method to linker
- Refactor existing execution logic
- Remove source existence checks that cause crashes

### Phase 4: Testing
- Update existing tests
- Add tests for each scenario
- Test with playground

## Benefits

1. **No crashes**: Gracefully handles missing sources
2. **Clear feedback**: Users know exactly what's happening
3. **Automation-friendly**: `--force` flag for CI/CD
4. **State hygiene**: Automatically cleans up orphaned links
5. **Better UX**: Warnings guide users to fix config

## Trade-offs

1. **Complexity**: Linker becomes larger (handles both calculation and execution)
2. **Performance**: Need to scan directories and compare state
3. **Breaking change**: Behavior changes for missing sources (crash → warn)
4. **Linker size**: May need to extract to separate module later if it grows too large

## Alternatives Considered

### Alternative 1: Keep crashes, add `--ignore-missing` flag
- **Rejected**: Forces users to deal with errors manually
- Less user-friendly

### Alternative 2: Always remove broken links (no warnings)
- **Rejected**: Users might accidentally lose config entries
- No way to distinguish intentional vs accidental deletions

### Alternative 3: Store explicit/implicit metadata in state
- **Rejected**: Adds complexity to state file format
- Can be calculated on-the-fly from config

### Alternative 4: Separate diff.rs module
- **Rejected for now**: Would require extracting shared helpers to path_utils.rs
- Over-engineering for current scope
- Can revisit if linker.rs grows too large (>1000 lines)

## Open Questions

None - all decisions made.

## Implementation Checklist

- [ ] Add `LinkAction::Warning` variant to `linker.rs`
- [ ] Add `--force` flag to CLI in `main.rs`
- [ ] Implement `calculate_diff()` method in `linker.rs`
- [ ] Add `execute_action()` method in `linker.rs`
- [ ] Add `is_explicit()` helper function in `linker.rs`
- [ ] Add `scan_directory_recursive()` helper in `linker.rs`
- [ ] Remove old `link_package()`, `link_folder()`, `link_files_recursive()` methods
- [ ] Update `commands.rs::link()` to use new API
- [ ] Update output formatting for warnings in `commands.rs`
- [ ] Update state update logic in `commands.rs`
- [ ] Add tests for all 5 scenarios
- [ ] Update existing tests to work with new API
- [ ] Update playground test cases
- [ ] Update documentation
