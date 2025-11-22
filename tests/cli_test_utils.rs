use std::{fs, path::Path, process::Command};

/// Helper function to get the path to the doty binary
pub fn get_doty_binary() -> String {
    // In integration tests, cargo sets CARGO_BIN_EXE_<name> environment variable
    env!("CARGO_BIN_EXE_doty").to_string()
}

/// Helper function to run doty with arbitrary arguments
/// ## Parameters
/// args: array of arguments to pass to the doty command
/// ## Returns
/// Ok(String) containing stdout on success, Err(String) containing stderr on failure
pub fn run_doty(args: &[impl AsRef<str>]) -> Result<String, String> {
    let binary = get_doty_binary();
    let mut cmd = Command::new(binary);

    for arg in args {
        cmd.arg(arg.as_ref());
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute doty: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("doty failed: {}", stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Helper function to run doty link command
pub fn run_doty_link(config_path: &Path) -> Result<String, String> {
    let binary = get_doty_binary();
    let output = Command::new(binary)
        .arg("link")
        .arg("--config")
        .arg(config_path)
        .output()
        .map_err(|e| format!("Failed to execute doty: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("doty link failed: {}", stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Helper function to check if a path is a symlink pointing to the expected target
/// ## Parameters
/// sym_path: the path of the symlink to check
/// expected_target: the expected target of the symlink
/// ## Returns
/// true if the symlink points to the expected target, false otherwise
pub fn is_symlink_to(sym_path: &Path, expected_target: &Path) -> bool {
    if !sym_path.exists() {
        return false;
    }

    if !sym_path.is_symlink() {
        return false;
    }

    // Try to read the symlink target
    match fs::read_link(sym_path) {
        Ok(actual_target) => {
            // Resolve both paths to absolute for comparison
            let expected_target_absolute = expected_target.canonicalize().ok();
            let actual_target_absolute = actual_target.canonicalize().ok();

            match (expected_target_absolute, actual_target_absolute) {
                (Some(expected_target_absolute), Some(actual_target_absolute)) => {
                    actual_target_absolute == expected_target_absolute
                }
                _ => {
                    // Fallback to string comparison
                    actual_target == expected_target
                }
            }
        }
        Err(_) => false,
    }
}
