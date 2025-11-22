# Integration Tests

This directory contains integration tests for the Doty dotfiles manager.

## Test Structure

### `01_link_folder/`

Tests for the `LinkFolder` strategy, which creates a single symlink for an entire directory (Stow-like behavior).

#### Directory Structure

Each test case is organized as a subdirectory within `01_link_folder/`:

```
tests/
├── README.md                    # This file
├── 01_link_folder.rs            # Rust test file that runs all test cases
└── 01_link_folder/              # Directory containing test case folders
    └── test_case_name/           # Individual test case directory
        ├── source/              # Source directory containing files to link
        │   └── ...              # Test files (one or more)
        ├── target/              # Target directory (initially empty)
        └── doty.kdl             # Configuration file for this test case
```

**Note:** The Rust test file `01_link_folder.rs` must be directly in the `tests/` directory (Rust's integration test requirement), while the test case folders are organized in the `01_link_folder/` subdirectory.

#### Test Case Schema

Each test case folder follows this structure:

1. **`source/`** - Contains one or more test files that should be linked
2. **`target/`** - Initially empty directory that serves as the target for symlinks
3. **`doty.kdl`** - Configuration file that defines the linking strategy to test

The `doty.kdl` file should configure a `LinkFolder` package that links the `source/` directory to the `target/` directory.

#### Test Execution

The `01_link_folder.rs` file contains integration tests that:

1. Set up the test case directory structure
2. Execute the `doty link` command pointing to the test case's `doty.kdl`
3. Validate that the target directory contains the expected symlink(s)
4. Clean up test artifacts

#### Example Test Case

```
tests/01_link_folder/
└── basic_single_file/
    ├── source/
    │   └── config.txt
    ├── target/
    └── doty.kdl
```

Where `doty.kdl` contains:
```kdl
defaults {
    pathResolution "config"
}

LinkFolder "source" {
    target "target"
}
```

After running `doty link`, the `target/` directory should contain a symlink `source` pointing to the `source/` directory.

