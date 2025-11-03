use std::io;
use std::path::Path;

use acme_disk_use::{format_size, DiskUse};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "acme-disk-use")]
#[command(about = "A disk usage analyzer with caching support")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Directory to analyze (defaults to current directory)
    #[arg(value_name = "PATH")]
    path: Option<String>,

    /// Show raw bytes instead of human-readable sizes
    #[arg(long)]
    non_human_readable: bool,

    /// Ignore cache and scan fresh
    #[arg(long)]
    ignore_cache: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Clean the cache contents
    Clean,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let mut disk_use = DiskUse::new_with_default_cache();

    match cli.command {
        Some(Commands::Clean) => {
            disk_use.clear_cache()?;
            println!("Cache cleared successfully.");
            return Ok(());
        }
        None => {
            // Default scan command
            let path = cli.path.as_deref().unwrap_or(".");

            if !Path::new(path).exists() {
                eprintln!("Error: Path '{}' does not exist", path);
                std::process::exit(1);
            }

            // Scan the directory with appropriate options
            let total_size = disk_use.scan_with_options(path, cli.ignore_cache)?;

            // Get file count using the same ignore_cache setting
            let file_count = disk_use.get_file_count(path, cli.ignore_cache)?;

            // Format output based on user preference
            println!(
                "Found {} files, total size: {}",
                file_count,
                format_size(total_size, !cli.non_human_readable)
            );

            // Explicitly save cache before exiting (Drop will save too, but be explicit)
            if !cli.ignore_cache {
                disk_use.save_cache()?;
            }
        }
    }

    Ok(())
}
