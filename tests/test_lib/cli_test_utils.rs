use std::{fs, io::Write, path::Path, process::Command};

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

/// Helper function to run doty link command with --dry-run flag
pub fn run_doty_link_dry_run(config_path: &Path) -> Result<String, String> {
    let binary = get_doty_binary();
    let output = Command::new(binary)
        .arg("link")
        .arg("--config")
        .arg(config_path)
        .arg("--dry-run")
        .output()
        .map_err(|e| format!("Failed to execute doty: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("doty link --dry-run failed: {}", stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Helper function to write a log file in the test case directory
/// ## Parameters
/// test_case_dir: the test case directory path
/// logfile_name: the name of the log file (e.g., "dry-run.log")
/// content: the content to write to the log file
/// ## Behavior
/// - Creates a "logs" folder if it does not exist
/// - Adds "logs/" to .gitignore in the testcase dir, creates the .gitignore if not present
/// - Writes the log file to logs/{logfile_name}
pub fn write_logfile(
    test_case_dir: &Path,
    logfile_name: &str,
    content: &str,
) -> Result<std::path::PathBuf, std::io::Error> {
    // Create logs directory if it doesn't exist
    let logs_dir = test_case_dir.join("logs");
    fs::create_dir_all(&logs_dir)?;

    // Ensure .gitignore exists and contains "logs/"
    let gitignore_path = test_case_dir.join(".gitignore");
    let mut gitignore_content = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path).unwrap_or_default()
    } else {
        String::new()
    };

    // Check if "logs/" is already in .gitignore
    let logs_entry = "logs/\n";
    if !gitignore_content.contains("logs/\n") {
        // Append "logs/" to .gitignore
        if !gitignore_content.is_empty() && !gitignore_content.ends_with('\n') {
            gitignore_content.push('\n');
        }
        gitignore_content.push_str(logs_entry);

        // Write updated .gitignore
        let mut file = fs::File::create(&gitignore_path)?;
        file.write_all(gitignore_content.as_bytes())?;
    }

    // Write the log file
    let log_path = logs_dir.join(logfile_name);
    fs::write(&log_path, content)?;

    Ok(log_path)
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
