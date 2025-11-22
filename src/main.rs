mod commands;
mod config;
mod linker;
mod state;

use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use colored::Colorize;
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
        
        /// Treat warnings as removals (useful for automation)
        #[arg(long)]
        force: bool,
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

    // Check if config file exists
    if !config_path.as_std_path().exists() {
        anyhow::bail!("Config file not found: {}", config_path);
    }

    match cli.command {
        Commands::Link { dry_run, force } => {
            if dry_run {
                println!("\n{} {}", "Linking 🔗".bold(), "[DRY RUN]".yellow().bold());
            } else {
                println!("\n{}", "Linking 🔗".bold());
            }
            if force {
                println!("{} {}", "Mode:".bold(), "FORCE (warnings become removals)".red().bold());
            }
            println!("Config: {}\n", config_path);
            commands::link(config_path, dry_run, force)?;
        }
        Commands::Clean { dry_run } => {
            if dry_run {
                println!("\n{} {}", "Cleaning 🧹".bold(), "[DRY RUN]".yellow().bold());
            } else {
                println!("\n{}", "Cleaning 🧹".bold());
            }
            println!("Using config: {}", config_path);
            commands::clean(config_path, dry_run)?;
        }
        Commands::Adopt { path } => {
            println!("\n{} {}: {}", "Adopting 📦".bold(), "for path".bold(), path);
            println!("Not yet implemented");
        }
        Commands::Detect => {
            println!("\n{}", "Detecting unmonitored files 🔍".bold());
            println!("Not yet implemented");
        }
        Commands::Status => {
            println!("\n{}", "Status 📊".bold());
            println!("Not yet implemented");
        }
    }

    Ok(())
}
