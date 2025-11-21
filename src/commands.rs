use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use std::env;
use vfs::{PhysicalFS, VfsPath};

use crate::config::{DotyConfig, PathResolution};
use crate::linker::{LinkAction, Linker};
use crate::state::DotyState;

/// Execute the link command
pub fn link(config_path: Utf8PathBuf, dry_run: bool) -> Result<()> {
    // Get hostname
    let hostname = hostname::get()?
        .to_string_lossy()
        .to_string();

    // Determine repo root based on path resolution strategy
    // First, load the config to determine the path resolution strategy
    let config_fs = PhysicalFS::new(config_path.parent().unwrap_or_else(|| ".".as_ref()).as_std_path());
    let config_vfs_root = VfsPath::new(config_fs);
    let config_vfs_path = config_vfs_root.join(config_path.file_name().unwrap_or("doty.kdl"))?;
    let config = DotyConfig::from_vfs(&config_vfs_path)
        .context("Failed to load configuration")?;

    // Determine repo root based on path resolution strategy
    let repo_root = match config.path_resolution {
        PathResolution::Config => {
            // Resolve relative to config file location
            config_path.parent()
                .ok_or_else(|| anyhow::anyhow!("Config file has no parent directory"))?
                .to_path_buf()
        }
        PathResolution::Cwd => {
            // Resolve relative to current working directory
            Utf8PathBuf::from_path_buf(env::current_dir()?)
                .map_err(|_| anyhow::anyhow!("Current directory path is not valid UTF-8"))?
        }
    };

    // Setup VFS with the determined repo root
    let fs = PhysicalFS::new(repo_root.as_std_path());
    let vfs_root = VfsPath::new(fs);

    // Load state
    let state_dir = vfs_root.join(".doty/state")?;
    let mut state = DotyState::load_vfs(&state_dir, &hostname)
        .context("Failed to load state")?;

    // Get home directory for target root
    let home_dir = std::env::var("HOME")
        .context("HOME environment variable not set")?;
    let home_fs = PhysicalFS::new(&home_dir);
    let target_root = VfsPath::new(home_fs);

    // Create linker
    let linker = Linker::new(vfs_root.clone(), target_root);

    // Process each package
    let mut all_actions = Vec::new();
    for package in &config.packages {
        println!("Processing package: {} -> {}", package.source, package.target);
        
        let actions = linker.link_package(package, dry_run)
            .with_context(|| format!("Failed to link package: {}", package.source))?;
        
        for action in &actions {
            match action {
                LinkAction::Created { target, source } => {
                    println!("  ✓ Created: {} -> {}", target, source);
                    if !dry_run {
                        state.add_link(target.clone(), source.clone());
                    }
                }
                LinkAction::Updated { target, old_source, new_source } => {
                    println!("  ↻ Updated: {} ({} -> {})", target, old_source, new_source);
                    if !dry_run {
                        state.add_link(target.clone(), new_source.clone());
                    }
                }
                LinkAction::Skipped { target, source } => {
                    println!("  - Skipped: {} -> {} (already linked)", target, source);
                }
                LinkAction::Removed { target, source } => {
                    println!("  ✗ Removed: {} -> {}", target, source);
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
        state.save_vfs(&state_dir)
            .context("Failed to save state")?;
        println!("\n✓ State saved to .doty/state/{}.kdl", hostname);
    } else {
        println!("\n[DRY RUN] No changes were made");
    }

    println!("\nSummary:");
    let created = all_actions.iter().filter(|a| matches!(a, LinkAction::Created { .. })).count();
    let updated = all_actions.iter().filter(|a| matches!(a, LinkAction::Updated { .. })).count();
    let skipped = all_actions.iter().filter(|a| matches!(a, LinkAction::Skipped { .. })).count();
    let removed = all_actions.iter().filter(|a| matches!(a, LinkAction::Removed { .. })).count();
    
    println!("  Created: {}", created);
    println!("  Updated: {}", updated);
    println!("  Skipped: {}", skipped);
    println!("  Removed: {}", removed);

    Ok(())
}

/// Execute the clean command
pub fn clean(config_path: Utf8PathBuf, dry_run: bool) -> Result<()> {
    // Get hostname
    let hostname = hostname::get()?
        .to_string_lossy()
        .to_string();

    // Determine repo root based on path resolution strategy
    // First, load the config to determine the path resolution strategy
    let config_fs = PhysicalFS::new(config_path.parent().unwrap_or_else(|| ".".as_ref()).as_std_path());
    let config_vfs_root = VfsPath::new(config_fs);
    let config_vfs_path = config_vfs_root.join(config_path.file_name().unwrap_or("doty.kdl"))?;
    let config = DotyConfig::from_vfs(&config_vfs_path)
        .context("Failed to load configuration")?;

    // Determine repo root based on path resolution strategy
    let repo_root = match config.path_resolution {
        PathResolution::Config => {
            // Resolve relative to config file location
            config_path.parent()
                .ok_or_else(|| anyhow::anyhow!("Config file has no parent directory"))?
                .to_path_buf()
        }
        PathResolution::Cwd => {
            // Resolve relative to current working directory
            Utf8PathBuf::from_path_buf(env::current_dir()?)
                .map_err(|_| anyhow::anyhow!("Current directory path is not valid UTF-8"))?
        }
    };

    // Setup VFS with the determined repo root
    let fs = PhysicalFS::new(repo_root.as_std_path());
    let vfs_root = VfsPath::new(fs);

    // Load state
    let state_dir = vfs_root.join(".doty/state")?;
    let state = DotyState::load_vfs(&state_dir, &hostname)
        .context("Failed to load state")?;

    if state.links.is_empty() {
        println!("No managed links found for host: {}", hostname);
        return Ok(());
    }

    // Get home directory for target root
    let home_dir = std::env::var("HOME")
        .context("HOME environment variable not set")?;
    let home_fs = PhysicalFS::new(&home_dir);
    let target_root = VfsPath::new(home_fs);

    // Create linker
    let linker = Linker::new(vfs_root.clone(), target_root);

    // Clean all links
    println!("Removing {} managed link(s)...", state.links.len());
    let actions = linker.clean(&state, dry_run)
        .context("Failed to clean links")?;

    for action in &actions {
        if let LinkAction::Removed { target, source } = action {
            println!("  ✗ Removed: {} -> {}", target, source);
        }
    }

    // Clear state
    if !dry_run {
        let empty_state = DotyState::new(hostname.clone());
        empty_state.save_vfs(&state_dir)
            .context("Failed to save state")?;
        println!("\n✓ State cleared for host: {}", hostname);
    } else {
        println!("\n[DRY RUN] No changes were made");
    }

    println!("\nSummary: {} link(s) removed", actions.len());

    Ok(())
}
