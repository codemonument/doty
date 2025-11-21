use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use colored::Colorize;
use std::env;

use crate::config::{DotyConfig, LinkStrategy, PathResolution};
use crate::linker::{LinkAction, Linker};
use crate::state::DotyState;

/// Execute link command
pub fn link(config_path: Utf8PathBuf, dry_run: bool) -> Result<()> {
    // Get hostname
    let hostname = hostname::get()?.to_string_lossy().to_string();

    // Load config to determine the path resolution strategy
    let config = DotyConfig::from_file(&config_path).context("Failed to load configuration")?;

    // Determine repo root based on path resolution strategy
    let repo_root = match config.path_resolution {
        PathResolution::Config => {
            // Resolve relative to config file location
            config_path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Config file has no parent directory"))?
                .to_path_buf()
        }
        PathResolution::Cwd => {
            // Resolve relative to current working directory
            Utf8PathBuf::from_path_buf(env::current_dir()?)
                .map_err(|_| anyhow::anyhow!("Current directory path is not valid UTF-8"))?
        }
    };

    // Load state
    let state_dir = repo_root.join(".doty/state");
    let mut state = DotyState::load(&state_dir, &hostname).context("Failed to load state")?;

    // Create linker
    let linker = Linker::new(repo_root.clone());

    // Process each package
    let mut all_actions = Vec::new();
    for package in &config.packages {
        // Show strategy name instead of "package"
        let strategy_name = match package.strategy {
            LinkStrategy::LinkFolder => "LinkFolder",
            LinkStrategy::LinkFilesRecursive => "LinkFilesRecursive",
        };
        println!(
            "\n{} {} → {}",
            strategy_name.bold(),
            package.source,
            package.target
        );

        let actions = linker
            .link_package(package, dry_run)
            .with_context(|| format!("Failed to link: {}", package.source))?;

        for action in &actions {
            match action {
                LinkAction::Created { target, source } => {
                    println!("  {} {} → {}", "[+]".green().bold(), target, source);
                    if !dry_run {
                        state.add_link(target.clone(), source.clone());
                    }
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
                    if !dry_run {
                        state.add_link(target.clone(), new_source.clone());
                    }
                }
                LinkAction::Skipped { .. } => {
                    // Don't print anything for skipped links (already up to date)
                }
                LinkAction::Removed { target, source } => {
                    println!("  {} {} → {}", "[-]".red().bold(), target, source);
                    if !dry_run {
                        state.remove_link(target);
                    }
                }
            }
        }

        all_actions.extend(actions);
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
    let created = all_actions
        .iter()
        .filter(|a| matches!(a, LinkAction::Created { .. }))
        .count();
    let updated = all_actions
        .iter()
        .filter(|a| matches!(a, LinkAction::Updated { .. }))
        .count();
    let skipped = all_actions
        .iter()
        .filter(|a| matches!(a, LinkAction::Skipped { .. }))
        .count();
    let removed = all_actions
        .iter()
        .filter(|a| matches!(a, LinkAction::Removed { .. }))
        .count();

    if created > 0 || updated > 0 || removed > 0 {
        println!("\n{}", "Summary:".bold());
        if created > 0 {
            println!("  {} {} link(s) added", "[+]".green().bold(), created);
        }
        if updated > 0 {
            println!("  {} {} link(s) updated", "[~]".yellow().bold(), updated);
        }
        if removed > 0 {
            println!("  {} {} link(s) removed", "[-]".red().bold(), removed);
        }
        if skipped > 0 {
            println!("  {} {} link(s) unchanged", "·".dimmed(), skipped);
        }
    } else if skipped > 0 {
        println!(
            "\n{} All {} link(s) already up to date",
            "✓".green().bold(),
            skipped
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
    let repo_root = match config.path_resolution {
        PathResolution::Config => {
            // Resolve relative to config file location
            config_path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Config file has no parent directory"))?
                .to_path_buf()
        }
        PathResolution::Cwd => {
            // Resolve relative to current working directory
            Utf8PathBuf::from_path_buf(env::current_dir()?)
                .map_err(|_| anyhow::anyhow!("Current directory path is not valid UTF-8"))?
        }
    };

    // Load state
    let state_dir = repo_root.join(".doty/state");
    let state = DotyState::load(&state_dir, &hostname).context("Failed to load state")?;

    if state.links.is_empty() {
        println!("No managed links found for host: {}", hostname);
        return Ok(());
    }

    // Create linker
    let linker = Linker::new(repo_root);

    // Clean all links
    println!("Removing {} managed link(s)...\n", state.links.len());
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
        let empty_state = DotyState::new(hostname.clone());
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
        "\n{} {} {} link(s) removed",
        "Summary:".bold(),
        "[-]".red().bold(),
        actions.len()
    );

    Ok(())
}