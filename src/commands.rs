use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use colored::Colorize;
use dialoguer::Confirm;
use pluralizer::pluralize;
use std::env;

use crate::config::{DotyConfig, LinkStrategy, PathResolution};
use crate::linker::{LinkAction, Linker};
use crate::scanner::{Scanner, DriftType};
use crate::state::DotyState;

/// Execute link command
pub fn link(config_path: Utf8PathBuf, dry_run: bool, force: bool) -> Result<()> {
    // Get hostname
    let hostname = hostname::get()?.to_string_lossy().to_string();

    // Load config to determine the path resolution strategy
    let config = DotyConfig::from_file(&config_path).context("Failed to load configuration")?;

    // Determine repo root based on path resolution strategy
    let config_dir_or_cwd = match config.path_resolution {
        PathResolution::Config => {
            // Resolve relative to config file location
            let config_dir = config_path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Config file has no parent directory"))?;
            
            // Canonicalize to get absolute path
            let abs_path = if config_dir.as_str().is_empty() || config_dir == "." {
                Utf8PathBuf::from_path_buf(env::current_dir()?)
                    .map_err(|_| anyhow::anyhow!("Current directory path is not valid UTF-8"))?
            } else {
                config_dir.canonicalize_utf8()?
            };
            
            abs_path
        }
        PathResolution::Cwd => {
            // Resolve relative to current working directory
            Utf8PathBuf::from_path_buf(env::current_dir()?)
                .map_err(|_| anyhow::anyhow!("Current directory path is not valid UTF-8"))?
        }
    };

    println!("{:<10} {}", "Config:", config_path);
    println!("{:<10} {}\n", "BasePath:", config_dir_or_cwd);

    // Load state
    let state_dir = config_dir_or_cwd.join(".doty/state");
    let mut state = DotyState::load(&state_dir, &hostname, config_dir_or_cwd.clone()).context("Failed to load state")?;

    // Create linker
    let linker = Linker::new(config_dir_or_cwd.clone(), config.path_resolution);

    // Calculate diff using the new linker API
    let actions = linker
        .calculate_diff(&config, &state, force)
        .context("Failed to calculate diff")?;

    // Group actions by package for output
    let mut package_actions: std::collections::HashMap<String, Vec<&LinkAction>> = std::collections::HashMap::new();
    let mut orphaned_actions = Vec::new();

    for action in &actions {
        match action {
            LinkAction::Created { target, .. } |
            LinkAction::Updated { target, .. } |
            LinkAction::Skipped { target, .. } |
            LinkAction::Warning { target, .. } => {
                // Find which package this target belongs to
                let mut found_package = false;
                for package in &config.packages {
                    if target.starts_with(&package.target) {
                        let package_key = format!("{} {} → {}",
                            match package.strategy {
                                LinkStrategy::LinkFolder => "LinkFolder",
                                LinkStrategy::LinkFilesRecursive => "LinkFilesRecursive",
                            },
                            package.source,
                            package.target
                        );
                        package_actions.entry(package_key).or_insert_with(Vec::new).push(action);
                        found_package = true;
                        break;
                    }
                }
                if !found_package {
                    orphaned_actions.push(action);
                }
            }
            LinkAction::Removed { target, .. } => {
                // Check if this target belongs to any current package
                let mut found_package = false;
                for package in &config.packages {
                    if target.starts_with(&package.target) {
                        let package_key = format!("{} {} → {}",
                            match package.strategy {
                                LinkStrategy::LinkFolder => "LinkFolder",
                                LinkStrategy::LinkFilesRecursive => "LinkFilesRecursive",
                            },
                            package.source,
                            package.target
                        );
                        package_actions.entry(package_key).or_insert_with(Vec::new).push(action);
                        found_package = true;
                        break;
                    }
                }
                if !found_package {
                    orphaned_actions.push(action);
                }
            }
        }
    }

    // Print actions grouped by package
    for (package_key, actions) in package_actions {
        // Filter out skipped actions for display
        let display_actions: Vec<&&LinkAction> = actions
            .iter()
            .filter(|a| !matches!(**a, LinkAction::Skipped { .. }))
            .collect();

        if display_actions.is_empty() {
            continue;
        }

        println!("\n{}", package_key.bold());
        for action in display_actions {
            match action {
                LinkAction::Created { target, source } => {
                    println!("  {} {} → {}", "[+]".green().bold(), target, source);
                }
                LinkAction::Updated {
                    target,
                    old_source,
                    new_source,
                } => {
                    println!(
                        "  {} {} → {} {}",
                        "[~]".yellow().bold(),
                        target,
                        new_source,
                        format!("(was: {})", old_source).dimmed()
                    );
                }
                LinkAction::Skipped { .. } => {
                    // Do not print skipped links
                }
                LinkAction::Removed { target, source } => {
                    println!("  {} {} → {}", "[-]".red().bold(), target, source);
                }
                LinkAction::Warning {
                    target,
                    source,
                    message,
                } => {
                    println!("  {} {} → {}", "[!]".yellow().bold(), target, source);
                    println!("      Warning: {}", message);
                }
            }
        }
    }

    // Print orphaned actions
    if !orphaned_actions.is_empty() {
        println!("\n{}", "Orphaned links:".bold());
        for action in orphaned_actions {
            match action {
                LinkAction::Removed { target, source } => {
                    println!("  {} {} → {}", "[-]".red().bold(), target, source);
                }
                _ => {} // Shouldn't happen for orphaned actions
            }
        }
    }

    // Execute actions and update state
    for action in &actions {
        linker.execute_action(action, dry_run)?;
        
        // Update state
        if !dry_run {
            match action {
                LinkAction::Created { target, source } => {
                    state.add_link(target.clone(), source.clone());
                }
                LinkAction::Updated { target, new_source, .. } => {
                    state.add_link(target.clone(), new_source.clone());
                }
                LinkAction::Removed { target, .. } => {
                    state.remove_link(target);
                }
                LinkAction::Warning { .. } | LinkAction::Skipped { .. } => {
                    // Don't modify state for warnings or skipped links
                }
            }
        }
    }

    // Save state
    if !dry_run {
        state.save(&state_dir).context("Failed to save state")?;
        println!(
            "\n{} State saved to .doty/state/{}.kdl",
            "✓".green().bold(),
            hostname
        );
    } else {
        println!("\n{}", "[DRY RUN] No changes were made".yellow().bold());
    }

    // Summary
    let created = actions
        .iter()
        .filter(|a| matches!(a, LinkAction::Created { .. }))
        .count();
    let updated = actions
        .iter()
        .filter(|a| matches!(a, LinkAction::Updated { .. }))
        .count();
    let skipped = actions
        .iter()
        .filter(|a| matches!(a, LinkAction::Skipped { .. }))
        .count();
    let removed = actions
        .iter()
        .filter(|a| matches!(a, LinkAction::Removed { .. }))
        .count();
    let warnings = actions
        .iter()
        .filter(|a| matches!(a, LinkAction::Warning { .. }))
        .count();

    if created > 0 || updated > 0 || removed > 0 || warnings > 0 {
        println!("\n{}", "Summary:".bold());
        if created > 0 {
            println!(
                "  {} {} added",
                "[+]".green().bold(),
                pluralize("link", created as isize, true)
            );
        }
        if updated > 0 {
            println!(
                "  {} {} updated",
                "[~]".yellow().bold(),
                pluralize("link", updated as isize, true)
            );
        }
        if removed > 0 {
            println!(
                "  {} {} removed",
                "[-]".red().bold(),
                pluralize("link", removed as isize, true)
            );
        }
        if warnings > 0 {
            println!(
                "  {} {}",
                "[!]".yellow().bold(),
                pluralize("warning", warnings as isize, true)
            );
        }
        if skipped > 0 {
            println!(
                "  {} {} unchanged",
                "[·]".dimmed(),
                pluralize("link", skipped as isize, true)
            );
        }
    } else if skipped > 0 {
        println!(
            "\n{} All {} already up to date",
            "✓".green().bold(),
            pluralize("link", skipped as isize, true)
        );
    }

    Ok(())
}

/// Execute clean command
pub fn clean(config_path: Utf8PathBuf, dry_run: bool) -> Result<()> {
    // Get hostname
    let hostname = hostname::get()?.to_string_lossy().to_string();

    // Load config to determine the path resolution strategy
    let config = DotyConfig::from_file(&config_path).context("Failed to load configuration")?;

    // Determine repo root based on path resolution strategy
    let config_dir_or_cwd = match config.path_resolution {
        PathResolution::Config => {
            // Resolve relative to config file location
            let config_dir = config_path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Config file has no parent directory"))?;
            
            // Canonicalize to get absolute path
            let abs_path = if config_dir.as_str().is_empty() || config_dir == "." {
                Utf8PathBuf::from_path_buf(env::current_dir()?)
                    .map_err(|_| anyhow::anyhow!("Current directory path is not valid UTF-8"))?
            } else {
                config_dir.canonicalize_utf8()?
            };
            
            abs_path
        }
        PathResolution::Cwd => {
            // Resolve relative to current working directory
            Utf8PathBuf::from_path_buf(env::current_dir()?)
                .map_err(|_| anyhow::anyhow!("Current directory path is not valid UTF-8"))?
        }
    };

    println!("{:<10} {}", "Config:", config_path);
    println!("{:<10} {}\n", "BasePath:", config_dir_or_cwd);

    // Load state
    let state_dir = config_dir_or_cwd.join(".doty/state");
    let state = DotyState::load(&state_dir, &hostname, config_dir_or_cwd.clone()).context("Failed to load state")?;

    if state.links.is_empty() {
        println!("No managed links found for host: {}", hostname);
        return Ok(());
    }

    // Create linker
    let linker = Linker::new(config_dir_or_cwd.clone(), config.path_resolution);

    // Clean all links
    println!(
        "Removing {} managed {}...\n",
        state.links.len(),
        pluralize("link", state.links.len() as isize, false)
    );
    let actions = linker
        .clean(&state, dry_run)
        .context("Failed to clean links")?;

    for action in &actions {
        if let LinkAction::Removed { target, source } = action {
            println!("  {} {} → {}", "[-]".red().bold(), target, source);
        }
    }

    // Clear state
    if !dry_run {
        let empty_state = DotyState::new(hostname.clone(), config_dir_or_cwd);
        empty_state
            .save(&state_dir)
            .context("Failed to save state")?;
        println!(
            "\n{} State cleared for host: {}",
            "✓".green().bold(),
            hostname
        );
    } else {
        println!("\n{}", "[DRY RUN] No changes were made".yellow().bold());
    }

    println!(
        "\n{} {} {} removed",
        "Summary:".bold(),
        "[-]".red().bold(),
        pluralize("link", actions.len() as isize, true)
    );

    Ok(())
}

/// Execute detect command
pub fn detect(config_path: Utf8PathBuf, interactive: bool) -> Result<()> {
    // Get hostname
    let hostname = hostname::get()?.to_string_lossy().to_string();

    // Load config to determine path resolution strategy
    let config = DotyConfig::from_file(&config_path).context("Failed to load configuration")?;

    // Determine repo root based on path resolution strategy
    let config_dir_or_cwd = match config.path_resolution {
        PathResolution::Config => {
            // Resolve relative to config file location
            let config_dir = config_path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Config file has no parent directory"))?;
            
            // Canonicalize to get absolute path
            let abs_path = if config_dir.as_str().is_empty() || config_dir == "." {
                Utf8PathBuf::from_path_buf(env::current_dir()?)
                    .map_err(|_| anyhow::anyhow!("Current directory path is not valid UTF-8"))?
            } else {
                config_dir.canonicalize_utf8()?
            };
            
            abs_path
        }
        PathResolution::Cwd => {
            // Resolve relative to current working directory
            Utf8PathBuf::from_path_buf(env::current_dir()?)
                .map_err(|_| anyhow::anyhow!("Current directory path is not valid UTF-8"))?
        }
    };

    println!("{:<10} {}", "Config:", config_path);
    println!("{:<10} {}\n", "BasePath:", config_dir_or_cwd);

    // Load state
    let state_dir = config_dir_or_cwd.join(".doty/state");
    let state = DotyState::load(&state_dir, &hostname, config_dir_or_cwd.clone()).context("Failed to load state")?;

    // Create scanner
    let scanner = Scanner::new(config_dir_or_cwd.clone());

    // Run drift detection
    let drift_items = scanner.scan_targets(&config, &state).context("Failed to scan for drift")?;

    // Group drift items by type and package
    let mut untracked_by_package: std::collections::HashMap<String, Vec<Utf8PathBuf>> = std::collections::HashMap::new();
    let mut broken_links = Vec::new();

    for item in &drift_items {
        match item.drift_type {
            DriftType::Untracked => {
                if let Some(package) = &item.package {
                    let package_key = format!("{} {} → {}",
                        match package.strategy {
                            LinkStrategy::LinkFilesRecursive => "LinkFilesRecursive",
                            LinkStrategy::LinkFolder => "LinkFolder",
                        },
                        package.source,
                        package.target
                    );
                    untracked_by_package.entry(package_key).or_insert_with(Vec::new).push(item.target_path.clone());
                }
            }
            DriftType::Broken => {
                broken_links.push(item.clone());
            }
            DriftType::Modified | DriftType::Orphaned => {
                // These are handled elsewhere or not implemented yet
            }
        }
    }

    // Print results
    if untracked_by_package.is_empty() && broken_links.is_empty() {
        println!("\n{} No drift detected", "✓".green().bold());
        return Ok(());
    }

    // Print untracked files (only for LinkFilesRecursive packages)
    for (package_key, untracked_files) in &untracked_by_package {
        println!("\n{} {}:", "Untracked files in".bold(), package_key);
        for file in untracked_files {
            println!("  {} {}", "[?]".yellow().bold(), file);
        }
    }

    // Print broken symlinks
    if !broken_links.is_empty() {
        println!("\n{}", "Broken symlinks:".bold());
        for item in &broken_links {
            let link = item.target_path.strip_prefix(&config_dir_or_cwd).unwrap_or(&item.target_path);
            let target_display = if let Some(target) = &item.symlink_target {
                let relative_target = target.strip_prefix(&config_dir_or_cwd).unwrap_or(target);
                format!("{} {} {}", "📄".dimmed(), "✗".red().bold(), relative_target)
            } else {
                format!("{} {} {}", "📄".dimmed(), "✗".red().bold(), "???")
            };
            
            println!("  {} {} {} → {}", "[!]".yellow().bold(), "🔗".cyan(), link, target_display);
        }
    }

    // Interactive mode handling
    if interactive {
        println!("\n{}", "Interactive mode:".bold());
        
        // Handle untracked files
        for (package_key, untracked_files) in &untracked_by_package {
            if !untracked_files.is_empty() {
                println!("\n{} {}:", "Adopt untracked files for".bold(), package_key);
                
                let should_adopt = Confirm::new()
                    .with_prompt(format!("Adopt these {} untracked files?", untracked_files.len()))
                    .default(true)
                    .interact()?;
                
                if should_adopt {
                    // TODO: Implement actual file adoption in step 3.3
                    println!("  {} {} would be adopted", "✓".green().bold(), pluralize("file", untracked_files.len() as isize, true));
                    println!("  (File adoption will be implemented in step 3.3)");
                } else {
                    println!("  {} Skipped {} untracked files", "−".yellow().bold(), pluralize("file", untracked_files.len() as isize, true));
                }
            }
        }

        // Handle broken links
        if !broken_links.is_empty() {
            println!("\n{}", "Remove broken symlinks?".bold());
            
            let should_remove = Confirm::new()
                .with_prompt(format!("Remove {} broken symlinks?", broken_links.len()))
                .default(true)
                .interact()?;
            
            if should_remove {
                // Remove broken symlinks
                let mut removed_count = 0;
                for item in &broken_links {
                    let broken_link = &item.target_path;
                    // Try to remove the broken symlink
                    if let Err(e) = std::fs::remove_file(broken_link) {
                        println!("  {} Failed to remove {}: {}", "✗".red().bold(), broken_link, e);
                    } else {
                        println!("  {} Removed {}", "✓".green().bold(), broken_link);
                        removed_count += 1;
                    }
                }
                
                if removed_count > 0 {
                    println!("\n{} {} removed", "✓".green().bold(), pluralize("broken symlink", removed_count, true));
                }
            } else {
                println!("  {} Skipped {} broken symlinks", "−".yellow().bold(), pluralize("broken symlink", broken_links.len() as isize, true));
            }
        }
        
        if !untracked_by_package.is_empty() || !broken_links.is_empty() {
            println!("\n{} Interactive cleanup completed", "✓".green().bold());
        }
    } else {
        println!("\n{} {} to adopt or cleanup", "Run 'doty detect --interactive'".yellow().bold(), "interactive mode".yellow());
    }

    Ok(())
}