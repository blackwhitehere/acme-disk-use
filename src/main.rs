use std::io;
use std::path::Path;
use std::time::Instant;

use acme_disk_use::{format_size, tui, DiskUse};
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

    /// Suppress timing statistics in output
    #[arg(short, long)]
    quiet: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Clean the cache contents
    Clean,
    /// Cache management commands
    Cache {
        #[command(subcommand)]
        action: CacheCommands,
    },
}

#[derive(Subcommand)]
enum CacheCommands {
    /// Display an interactive TUI showing cached directory sizes (similar to ncdu)
    Show {
        /// Optional path to show (if omitted, shows all cached roots)
        #[arg(value_name = "PATH")]
        path: Option<String>,
    },
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let mut disk_use = DiskUse::new_with_default_cache();

    match cli.command {
        Some(Commands::Clean) => match disk_use.clear_cache() {
            Ok(_) => {
                println!("Cache cleared successfully.");
                Ok(())
            }
            Err(err) => {
                eprintln!("Error: Failed to clear cache: {}", err);
                std::process::exit(1);
            }
        },
        Some(Commands::Cache { action }) => match action {
            CacheCommands::Show { path } => {
                if disk_use.is_cache_empty() {
                    eprintln!("Error: Cache is empty. Run a scan first to populate the cache.");
                    std::process::exit(1);
                }

                if let Some(path_str) = path {
                    // Show specific path from cache
                    let path = Path::new(&path_str);
                    match disk_use.get_stats(path) {
                        Some(stat) => {
                            if let Err(err) = tui::run_tui(stat) {
                                eprintln!("Error: Failed to run TUI: {}", err);
                                std::process::exit(1);
                            }
                        }
                        None => {
                            eprintln!(
                                "Error: Path '{}' not found in cache. Run a scan on this path first.",
                                path_str
                            );
                            std::process::exit(1);
                        }
                    }
                } else {
                    // Show all cached roots
                    let roots = disk_use.get_cached_roots();
                    if let Err(err) = tui::run_tui_with_roots(roots) {
                        eprintln!("Error: Failed to run TUI: {}", err);
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
        },
        None => {
            // Default scan command
            let path = cli.path.as_deref().unwrap_or(".");

            if !Path::new(path).exists() {
                eprintln!("Error: Path '{}' does not exist", path);
                std::process::exit(1);
            }

            // Start timing the scan
            let start_time = Instant::now();

            // Scan the directory with appropriate options
            let total_size = match disk_use.scan_with_options(path, cli.ignore_cache) {
                Ok(size) => size,
                Err(err) => {
                    eprintln!("Error: {}", err);
                    std::process::exit(1);
                }
            };

            // Get file count using the same ignore_cache setting
            let file_count = match disk_use.get_file_count(path, cli.ignore_cache) {
                Ok(count) => count,
                Err(err) => {
                    eprintln!("Warning: Failed to get file count: {}", err);
                    0 // Continue with 0 if count fails
                }
            };

            // Calculate elapsed time
            let elapsed = start_time.elapsed();

            // Format output based on user preference
            if cli.quiet {
                println!(
                    "Found {} files, total size: {}",
                    file_count,
                    format_size(total_size, !cli.non_human_readable)
                );
            } else {
                let elapsed_secs = elapsed.as_secs_f64();
                // Use a small epsilon to avoid division by zero for extremely fast scans
                let files_per_sec = file_count as f64 / elapsed_secs.max(f64::MIN_POSITIVE);

                println!(
                    "Found {} files, total size: {} (scanned in {:.2}s, {:.0} files/s)",
                    file_count,
                    format_size(total_size, !cli.non_human_readable),
                    elapsed_secs,
                    files_per_sec
                );
            }

            // Explicitly save cache before exiting (Drop will save too, but be explicit)
            if !cli.ignore_cache {
                if let Err(err) = disk_use.save_cache() {
                    eprintln!("Warning: Failed to save cache: {}", err);
                }
            }

            Ok(())
        }
    }
}
