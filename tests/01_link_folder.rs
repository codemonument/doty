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
    let test_case_dir = Path::new("tests/01_link_folder/simple")
        .canonicalize()
        .unwrap();
    let config_path = test_case_dir.join("doty.kdl");
    let source_dir = test_case_dir.join("source");
    let target_dir = test_case_dir.join("target");

    // Ensure target directory exists and is empty
    if target_dir.exists() {
        fs::remove_dir_all(&target_dir).expect("Failed to clean target directory");
    }
    fs::create_dir_all(&target_dir).expect("Failed to create target directory");

    // Run doty link
    run_doty_link(&config_path).expect("doty link should succeed");

    // Validate: target should contain a symlink named "dummy" pointing to source_dir/dummy folder
    let expected_symlink = target_dir.join("dummy");
    assert!(
        expected_symlink.exists(),
        "Symlink 'dummy' should exist in target directory"
    );
    assert!(
        is_symlink_to(&expected_symlink, &source_dir.join("dummy")),
        "Symlink 'dummy' should point to the source directory/dummy"
    );

    // Validate: target/dummy should contain the dummy.txt file
    let expected_file = target_dir.join("dummy/dummy.txt");
    assert!(
        expected_file.exists(),
        "dummy.txt should exist in target/dummy directory"
    );
    assert!(
        fs::read_to_string(&expected_file).unwrap() == "Hello World",
        "dummy.txt should contain 'Hello World'"
    );

    // Validate: changing the source file should update the target file
    fs::write(&source_dir.join("dummy/dummy.txt"), "Hello World 2").unwrap();
    assert!(
        fs::read_to_string(&expected_file).unwrap() == "Hello World 2",
        "dummy.txt in target/dummy should contain 'Hello World 2'"
    );

    // Clean up: remove the symlink and lockfile
    if expected_symlink.exists() {
        fs::remove_file(&expected_symlink).ok();
    }
    // Clean up lockfile directory if it exists
    let lockfile_dir = test_case_dir.join(".doty/state");
    if lockfile_dir.exists() {
        fs::remove_dir_all(&lockfile_dir).ok();
    }
    // Clean up the file content in source/dummy/dummy.txt
    fs::write(&source_dir.join("dummy/dummy.txt"), "Hello World").unwrap();
}
