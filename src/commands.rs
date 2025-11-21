use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use vfs::{PhysicalFS, VfsPath};

use crate::config::DotyConfig;
use crate::linker::{LinkAction, Linker};
use crate::state::DotyState;

/// Execute the link command
pub fn link(repo_root: Utf8PathBuf, dry_run: bool) -> Result<()> {
    // Get hostname
    let hostname = hostname::get()?
        .to_string_lossy()
        .to_string();

    // Setup VFS with physical filesystem
    let fs = PhysicalFS::new(repo_root.as_std_path());
    let vfs_root = VfsPath::new(fs);

    // Load config
    let config_path = vfs_root.join("doty.kdl")?;
    let config = DotyConfig::from_vfs(&config_path)
        .context("Failed to load configuration")?;

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
pub fn clean(repo_root: Utf8PathBuf, dry_run: bool) -> Result<()> {
    // Get hostname
    let hostname = hostname::get()?
        .to_string_lossy()
        .to_string();

    // Setup VFS with physical filesystem
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
