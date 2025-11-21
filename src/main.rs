mod commands;
mod config;
mod linker;
mod state;

use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use std::env;

#[derive(Parser)]
#[command(name = "doty")]
#[command(version, about = "A hybrid dotfiles manager with flexible linking strategies", long_about = None)]
struct Cli {
    /// Path to the config file (defaults to ./doty.kdl)
    #[arg(short, long, global = true, value_name = "FILE")]
    config: Option<Utf8PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Apply configuration and create symlinks
    #[command(visible_aliases = ["deploy", "install", "i"])]
    Link {
        /// Show what would be done without making changes
        #[arg(long)]
        dry_run: bool,
    },

    /// Remove all symlinks managed by Doty
    #[command(visible_aliases = ["unlink", "uninstall", "remove", "rm"])]
    Clean {
        /// Show what would be done without making changes
        #[arg(long)]
        dry_run: bool,
    },

    /// Import existing local configs into the Doty repo
    Adopt {
        /// Path to the config to adopt
        path: String,
    },

    /// Audit targets for untracked files or broken links
    Detect,

    /// Show current system health and mapping status
    Status,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Determine config file path
    let config_path = if let Some(config) = cli.config {
        config
    } else {
        // Default to doty.kdl in current directory
        let cwd = Utf8PathBuf::from_path_buf(env::current_dir()?)
            .map_err(|_| anyhow::anyhow!("Current directory path is not valid UTF-8"))?;
        cwd.join("doty.kdl")
    };

    // For now, derive repo_root from config file location
    // TODO: In Phase 2.1, this will respect pathResolution setting
    let repo_root = config_path.parent()
        .ok_or_else(|| anyhow::anyhow!("Config file has no parent directory"))?
        .to_path_buf();

    match cli.command {
        Commands::Link { dry_run } => {
            if dry_run {
                println!("🔗 Link command (DRY RUN)");
            } else {
                println!("🔗 Link command");
            }
            println!("Using config: {}", config_path);
            commands::link(repo_root, dry_run)?;
        }
        Commands::Clean { dry_run } => {
            if dry_run {
                println!("🧹 Clean command (DRY RUN)");
            } else {
                println!("🧹 Clean command");
            }
            println!("Using config: {}", config_path);
            commands::clean(repo_root, dry_run)?;
        }
        Commands::Adopt { path } => {
            println!("📦 Adopt command for path: {}", path);
            println!("Not yet implemented");
        }
        Commands::Detect => {
            println!("🔍 Detect command");
            println!("Not yet implemented");
        }
        Commands::Status => {
            println!("📊 Status command");
            println!("Not yet implemented");
        }
    }

    Ok(())
}
