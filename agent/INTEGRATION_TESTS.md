# Integration Test Guide

This guide explains how to write integration tests for Doty, following the established patterns from `tests/01_link_folder.rs`.

## Overview

Integration tests in Doty:
- Test the full CLI workflow by executing the `doty` binary
- Use real filesystem operations (not mocked)
- Are organized in the `tests/` directory (Rust's integration test convention)
- Each test case has its own directory with source files, target directory, and config

## Directory Structure

### Test File Location

Integration test files must be placed directly in the `tests/` directory:

```
tests/
├── 01_link_folder.rs          # Test file (must be in tests/)
├── cli_test_utils.rs          # Shared helper functions
└── 01_link_folder/            # Test case directories
    └── simple/
        ├── doty.kdl
        ├── source/
        └── target/
```

**Important**: Rust requires integration test files (`*.rs`) to be directly in `tests/`, but test case data can be organized in subdirectories.

### Test Case Directory Structure

Each test case should have its own directory with this structure:

```
tests/<test_group>/<test_case_name>/
├── doty.kdl          # Configuration file for this test case
├── source/           # Source files to be linked
│   └── ...           # Test files and directories
└── target/           # Target directory (initially empty, created by test)
```
<!-- TODO: build support for lockifle path in doty.kdl to avoid this machine specificity here -->
Some test cases may also have a `.doty/` directory with a `state/` directory and a `.doty/state/zephir-m3.lock.kdl` file besides 
(because the tests are run on my machine for now and the lockfile name is the hostname).
This is important for test cases that need to test the lockfile functionality.

```
.doty/
├── state/
    └── <hostname>.lock.kdl
```

## Test Case Setup

### 1. Create Test Case Directory

Create a new directory under the appropriate test group:

```bash
mkdir -p tests/01_link_folder/my_test_case/{source,target}
```

### 2. Create Source Files

Add test files to the `source/` directory that represent what should be linked:

```
tests/01_link_folder/my_test_case/source/
└── my_config/
    └── config.txt
```

CAUTION: NEVER link the source or target folders themselves, only the files and folders inside them.

### 3. Create Configuration File

Create a `doty.kdl` file that defines the linking strategy to test:

```kdl
defaults {
    pathResolution "config"
}

LinkFolder "source/my_config" {
    target "target/my_config"
}
```

**Note**: Use relative paths in the config that match your test case directory structure.
**Note**: Always use `pathResolution "config"` to make the paths relative to the doty.kdl file location.

## Test Function Template

Follow this structure for each test function:

### Step 1: Setup Paths

```rust
use std::fs;
use std::path::Path;

mod cli_test_utils;
// if you want to test another doty command, use the run_doty helper function (see Helper Functions section)
use crate::cli_test_utils::{is_symlink_to, run_doty_link};

#[test]
fn test_my_feature() {
    // Get absolute path to test case directory
    let test_case_dir = Path::new("tests/01_link_folder/my_test_case")
        .canonicalize()
        .unwrap();
    
    // Define key paths
    let config_path = test_case_dir.join("doty.kdl");
    let source_dir = test_case_dir.join("source");
    let target_dir = test_case_dir.join("target");
```

**Key Points**:
- Always use `canonicalize()` to get absolute paths
- Store paths in variables for reuse
- Use `join()` to build paths relative to the test case directory

### Step 2: Cleanup Previous Runs / Prepare Test Environment

```rust
    // Clean up: remove symlinks from previous runs
    let expected_symlink = target_dir.join("my_config");
    if expected_symlink.exists() {
        fs::remove_file(&expected_symlink).ok();
    }
    
    // Clean up lockfile directory if it exists
    let lockfile_dir = test_case_dir.join(".doty/state");
    if lockfile_dir.exists() {
        fs::remove_dir_all(&lockfile_dir).ok();
    }
    
    // Reset source file content to known state
    fs::write(&source_dir.join("my_config/config.txt"), "Initial Content").unwrap();

    // If needed: prepare other things, like a special state of the lockfile or the target directory.
```

**Key Points**:
- Always clean up artifacts from previous test runs BEFORE running the test
  => ensures that a human can view the test case directory and its contents after the test
- Use `.ok()` for cleanup operations (don't fail if nothing to clean)
- Reset source files to known state for reproducibility

### Step 3: Prepare Target Directory

```rust
    // Ensure target directory exists and is empty
    if target_dir.exists() {
        fs::remove_dir_all(&target_dir).expect("Failed to clean target directory");
    }
    fs::create_dir_all(&target_dir).expect("Failed to create target directory");
```

**Key Points**:
- The target dir should be prepared before the test, either by cleaning it or creating a specific state in it.
- Use `expect()` with descriptive error messages
- Create the directory even if it doesn't exist

### Step 4: Execute Command

```rust
    // Run doty link
    run_doty_link(&config_path).expect("doty link should succeed");
```

**Key Points**:
- Use helper functions from `cli_test_utils` module
- Use `expect()` with descriptive error messages
- The helper function handles command execution and error reporting

### Step 5: Validate Results

```rust
    // Validate: symlink exists
    let expected_symlink = target_dir.join("my_config");
    assert!(
        expected_symlink.exists(),
        "Symlink 'my_config' should exist in target directory"
    );
    
    // Validate: symlink points to correct target
    assert!(
        is_symlink_to(&expected_symlink, &source_dir.join("my_config")),
        "Symlink 'my_config' should point to the source directory"
    );
    
    // Validate: file content is accessible through symlink (only for file symlinks)
    let expected_file = target_dir.join("my_config/config.txt");
    assert!(
        expected_file.exists(),
        "config.txt should exist in target/my_config directory"
    );
    assert!(
        fs::read_to_string(&expected_file).unwrap() == "Initial Content",
        "config.txt should contain 'Initial Content'"
    );
```

**Key Points**:
- Test both symlink existence and correctness
- Verify file content is accessible through the symlink
- Use descriptive assertion messages

### Step 6: Test Side Effects (Optional)

```rust
    // Validate: changing the source file updates the target file (since target file is symlinked to the source file)
    fs::write(&source_dir.join("my_config/config.txt"), "Updated Content").unwrap();
    assert!(
        fs::read_to_string(&expected_file).unwrap() == "Updated Content",
        "config.txt in target/my_config should contain 'Updated Content'"
    );
```

**Key Points**:
- Test that symlinks work correctly (changes propagate)
- Verify the linking behavior matches expectations

## Helper Functions

The `cli_test_utils` module provides these helper functions:

### `run_doty(args: &[impl AsRef<str>]) -> Result<String, String>`

Executes `doty` with arbitrary arguments and returns stdout on success.

**Usage**:
```rust
// Run any doty command with custom arguments
run_doty(&["clean", "--config", config_path.to_str().unwrap()])
    .expect("doty clean should succeed");

// Run with multiple arguments
run_doty(&["detect", "--config", config_path.to_str().unwrap()])
    .expect("doty detect should succeed");
```

**When to use**: Use this function when testing commands other than `link`, or when you need to pass additional flags or arguments.

### `run_doty_link(config_path: &Path) -> Result<String, String>`

Executes `doty link --config <path>` and returns stdout on success.

**Usage**:
```rust
run_doty_link(&config_path).expect("doty link should succeed");
```

**When to use**: Convenience wrapper for the common case of running `doty link`. For more complex `link` commands with additional flags, use `run_doty` instead.

### `is_symlink_to(sym_path: &Path, expected_target: &Path) -> bool`

Checks if a path is a symlink pointing to the expected target.

**Usage**:
```rust
assert!(
    is_symlink_to(&symlink_path, &expected_target),
    "Symlink should point to expected target"
);
```

### `get_doty_binary() -> String`

Returns the path to the `doty` binary (used internally by other helpers).

## Validation Patterns

### Check Symlink Exists

```rust
assert!(
    symlink_path.exists(),
    "Symlink should exist"
);
```

### Check Symlink Correctness

```rust
assert!(
    is_symlink_to(&symlink_path, &expected_target),
    "Symlink should point to correct target"
);
```

### Check File Content Through Symlink

```rust
let content = fs::read_to_string(&file_path).unwrap();
assert_eq!(content, "Expected Content", "File should contain expected content");
```

### Check Directory Structure

```rust
assert!(
    target_dir.join("subdir/file.txt").exists(),
    "File should exist in expected location"
);
```

## Best Practices

### 1. Test Isolation

- Each test should be independent
- Always clean up before running the test (don't assume clean state)
- Use unique test case directories
- Prepare the test environment before running the test

### 2. Descriptive Names

- Test function names: `test_<feature>_<scenario>`
- Test case directories: descriptive names like `simple`, `nested_dirs`, `conflicting_files`
- Assertion messages: explain what should be true

### 3. Path Handling

- Always use `canonicalize()` for test case directories
- Use `join()` to build paths relative to test case directory
- Store paths in variables for reuse

### 4. Error Messages

- Use descriptive `expect()` messages
- Include context in assertion messages
- Help future developers understand test failures

### 5. Documentation

- Add doc comments explaining what the test validates
- Include context about preconditions (e.g., "no lockfile present")
- Note who approved the test (if applicable)

### 6. Cleanup Strategy

- Clean up before test (for idempotency)
- Don't clean up after test (helps debugging)
- Use `.ok()` for cleanup operations that might fail

## Complete Example

Here's a complete example following all the patterns:

```rust
use std::fs;
use std::path::Path;

mod cli_test_utils;
use crate::cli_test_utils::{is_symlink_to, run_doty_link};

/// Test case: Link one folder (source/dummy) to another folder (target/dummy)
/// Context:
/// - no lockfile is present
/// Approved by: bjesuiter
#[test]
fn test_01_link_folder_simple() {
    // Step 1: Setup paths
    let test_case_dir = Path::new("tests/01_link_folder/simple")
        .canonicalize()
        .unwrap();
    let config_path = test_case_dir.join("doty.kdl");
    let source_dir = test_case_dir.join("source");
    let target_dir = test_case_dir.join("target");

    // Step 2: Cleanup previous runs
    let expected_symlink = target_dir.join("dummy");
    if expected_symlink.exists() {
        fs::remove_file(&expected_symlink).ok();
    }
    let lockfile_dir = test_case_dir.join(".doty/state");
    if lockfile_dir.exists() {
        fs::remove_dir_all(&lockfile_dir).ok();
    }
    fs::write(&source_dir.join("dummy/dummy.txt"), "Hello World").unwrap();

    // Step 3: Prepare target directory
    if target_dir.exists() {
        fs::remove_dir_all(&target_dir).expect("Failed to clean target directory");
    }
    fs::create_dir_all(&target_dir).expect("Failed to create target directory");

    // Step 4: Execute command
    run_doty_link(&config_path).expect("doty link should succeed");

    // Step 5: Validate results
    let expected_symlink = target_dir.join("dummy");
    assert!(
        expected_symlink.exists(),
        "Symlink 'dummy' should exist in target directory"
    );
    assert!(
        is_symlink_to(&expected_symlink, &source_dir.join("dummy")),
        "Symlink 'dummy' should point to the source directory/dummy"
    );

    let expected_file = target_dir.join("dummy/dummy.txt");
    assert!(
        expected_file.exists(),
        "dummy.txt should exist in target/dummy directory"
    );
    assert!(
        fs::read_to_string(&expected_file).unwrap() == "Hello World",
        "dummy.txt should contain 'Hello World'"
    );

    // Step 6: Test side effects
    fs::write(&source_dir.join("dummy/dummy.txt"), "Hello World 2").unwrap();
    assert!(
        fs::read_to_string(&expected_file).unwrap() == "Hello World 2",
        "dummy.txt in target/dummy should contain 'Hello World 2'"
    );
}
```

## Running Tests

Run all integration tests:
```bash
cargo test
```

Run a specific test:
```bash
cargo test test_01_link_folder_simple
```

Run with output:
```bash
cargo test -- --nocapture
```

## Troubleshooting

### Test Fails: "File already exists"

**Problem**: Previous test run left artifacts.

**Solution**: Ensure cleanup code runs before test execution (Step 2).

### Test Fails: "No such file or directory"

**Problem**: Path resolution issue or missing directory.

**Solution**: 
- Use `canonicalize()` for test case directory
- Ensure target directory is created before running command
- Check that source files exist

### Test Fails: "Symlink points to wrong target"

**Problem**: Path resolution mismatch between config and test.

**Solution**:
- Verify `doty.kdl` uses correct relative paths
- Check that `pathResolution` setting matches test expectations
- Ensure paths in config match test case directory structure

### Test Passes Locally but Fails in CI

**Problem**: Path differences or permissions.

**Solution**:
- Always use absolute paths (`canonicalize()`)
- Don't rely on environment-specific paths
- Ensure cleanup handles all possible states

