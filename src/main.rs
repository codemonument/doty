mod config;
mod state;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "doty")]
#[command(version, about = "A hybrid dotfiles manager with flexible linking strategies", long_about = None)]
struct Cli {
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
    Clean,

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

    match cli.command {
        Commands::Link { dry_run } => {
            println!("🔗 Link command (dry_run: {})", dry_run);
            println!("Not yet implemented");
        }
        Commands::Clean => {
            println!("🧹 Clean command");
            println!("Not yet implemented");
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
