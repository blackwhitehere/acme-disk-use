use std::io;
use std::path::Path;

use acme_disk_use::{tui, DiskUse};
use clap::{Parser, Subcommand};

/// Format bytes into du-compatible human-readable format (e.g., 1K, 234M, 2G)
fn format_size_du(bytes: u64) -> String {
    const UNITS: &[&str] = &["", "K", "M", "G", "T", "P"];
    const THRESHOLD: f64 = 1024.0;

    if bytes == 0 {
        return "0".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= THRESHOLD && unit_index < UNITS.len() - 1 {
        size /= THRESHOLD;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{}", bytes)
    } else if size >= 10.0 {
        // For larger values, show integer (like du)
        format!("{:.0}{}", size, UNITS[unit_index])
    } else {
        // For smaller values, show one decimal (like du)
        format!("{:.1}{}", size, UNITS[unit_index])
    }
}

#[derive(Parser)]
#[command(name = "acme-disk-use")]
#[command(about = "A disk usage analyzer with caching support (du-compatible interface)")]
#[command(version = "0.1.0")]
#[command(disable_help_flag = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Directory to analyze (defaults to current directory)
    #[arg(value_name = "PATH")]
    path: Option<String>,

    /// Print sizes in human readable format (e.g., 1K 234M 2G)
    #[arg(short = 'h', long)]
    human_readable: bool,

    /// Display only a total for each argument (du compatibility, this is already the default behavior)
    #[arg(short = 's', long)]
    #[allow(dead_code)]
    summarize: bool,

    /// Equivalent to '--apparent-size --block-size=1' (show raw bytes)
    #[arg(short = 'b', long)]
    bytes: bool,

    /// Ignore cache and scan fresh
    #[arg(long)]
    ignore_cache: bool,

    /// Print help information
    #[arg(long, action = clap::ArgAction::Help)]
    help: Option<bool>,
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

            // Scan the directory with appropriate options
            let total_size = match disk_use.scan_with_options(path, cli.ignore_cache) {
                Ok(size) => size,
                Err(err) => {
                    eprintln!("Error: {}", err);
                    std::process::exit(1);
                }
            };

            // Determine output format: -b means raw bytes, -h means human-readable
            // If neither specified, default to 1K blocks like du
            let human_readable = cli.human_readable;
            let show_bytes = cli.bytes;

            // Format output in du-compatible style: SIZE\tPATH
            let size_str = if show_bytes {
                // -b: show raw bytes
                format!("{}", total_size)
            } else if human_readable {
                // -h: human readable format (e.g., 1K 234M 2G)
                format_size_du(total_size)
            } else {
                // Default: 1K blocks (like du default)
                let kb = total_size.div_ceil(1024);
                format!("{}", kb)
            };

            println!("{}\t{}", size_str, path);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size_du() {
        // Test zero bytes
        assert_eq!(format_size_du(0), "0");

        // Test bytes (no unit)
        assert_eq!(format_size_du(512), "512");
        assert_eq!(format_size_du(1023), "1023");

        // Test kilobytes
        assert_eq!(format_size_du(1024), "1.0K");
        assert_eq!(format_size_du(1536), "1.5K");
        assert_eq!(format_size_du(10240), "10K");
        assert_eq!(format_size_du(102400), "100K");

        // Test megabytes
        assert_eq!(format_size_du(1024 * 1024), "1.0M");
        assert_eq!(format_size_du(10 * 1024 * 1024), "10M");

        // Test gigabytes
        assert_eq!(format_size_du(1024 * 1024 * 1024), "1.0G");
        assert_eq!(format_size_du(10 * 1024 * 1024 * 1024), "10G");

        // Test terabytes
        assert_eq!(format_size_du(1024_u64 * 1024 * 1024 * 1024), "1.0T");
    }
}
