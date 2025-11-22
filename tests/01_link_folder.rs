use std::fs;
use std::path::Path;

mod test_lib;
use test_lib::cli_test_utils::{is_symlink_to, run_doty_link};

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

    // Clean up: remove the symlink and lockfile from previous runs
    let expected_symlink = target_dir.join("dummy");
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
}

/// Test case: Source directory deleted after linking
/// Context:
/// - Starts with final state of the "simple" test (symlink exists, source exists)
/// - Source directory is then deleted
/// - Tests that doty handles missing source gracefully
/// Approved by: bjesuiter
#[test]
fn test_01_link_folder_src_gone() {
    // Step 1: Setup paths
    let test_case_dir = Path::new("tests/01_link_folder/src_gone")
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
    // Clean up lockfile directory if it exists
    let lockfile_dir = test_case_dir.join(".doty/state");
    if lockfile_dir.exists() {
        fs::remove_dir_all(&lockfile_dir).ok();
    }

    // Reset source file content to known state
    // Ensure source directory exists
    fs::create_dir_all(&source_dir.join("dummy")).expect("Failed to create source/dummy directory");
    fs::write(&source_dir.join("dummy/dummy.txt"), "Hello World").unwrap();

    // Step 3: Prepare target directory
    if target_dir.exists() {
        fs::remove_dir_all(&target_dir).expect("Failed to clean target directory");
    }
    fs::create_dir_all(&target_dir).expect("Failed to create target directory");

    // Step 4: Execute command - initial link (like simple test)
    run_doty_link(&config_path).expect("doty link should succeed");

    // Step 5: Validate initial state (like simple test)
    let expected_symlink = target_dir.join("dummy");
    assert!(
        expected_symlink.exists(),
        "Symlink 'dummy' should exist in target directory"
    );
    assert!(
        is_symlink_to(&expected_symlink, &source_dir.join("dummy")),
        "Symlink 'dummy' should point to the source directory/dummy"
    );

    // Step 6: Delete source directory (the key difference from simple test)
    fs::remove_dir_all(&source_dir.join("dummy")).expect("Failed to remove source/dummy");

    // Step 7: Validate symlink is now broken
    // Use symlink_metadata to check if symlink exists even when broken
    assert!(
        fs::symlink_metadata(&expected_symlink).is_ok(),
        "Symlink 'dummy' should still exist after source deletion (even if broken)"
    );
    assert!(
        expected_symlink.is_symlink(),
        "Symlink 'dummy' should still be a symlink"
    );
    // The file should not be accessible through the broken symlink
    assert!(
        !expected_symlink.exists(),
        "dummy.txt should not be accessible through broken symlink"
    );

    // Step 8: Run doty link again - should warn but not crash
    let output =
        run_doty_link(&config_path).expect("doty link should succeed even with missing source");

    // Step 9: Validate warning is present in output
    assert!(
        output.contains("Source (file|dir) gone"),
        "Output should contain warning about missing source"
    );

    // Step 10: Validate symlink still exists (not removed)
    // Use symlink_metadata to check if symlink exists even when broken
    assert!(
        fs::symlink_metadata(&expected_symlink).is_ok(),
        "Symlink 'dummy' should still exist after running doty link with missing source"
    );
    assert!(
        expected_symlink.is_symlink(),
        "Symlink 'dummy' should still be a symlink"
    );
}
