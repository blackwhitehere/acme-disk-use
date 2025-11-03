use std::fs::OpenOptions;
use std::io::Write;

/// Initialize the logger to write to a file
///
/// Logs will be written to `app.log` in the current directory.
/// The log file is automatically added to .gitignore.
///
/// Log level can be controlled via RUST_LOG environment variable:
/// - RUST_LOG=debug cargo run (shows all debug and higher)
/// - RUST_LOG=info cargo run (shows info and higher, default)
/// - RUST_LOG=warn cargo run (shows warnings and errors only)
pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("app.log")?;

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            use std::time::SystemTime;

            // Get current timestamp
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap();
            let secs = now.as_secs();

            // Format: [TIMESTAMP] LEVEL - MESSAGE
            writeln!(
                buf,
                "[{}] {} - {}",
                format_timestamp(secs),
                record.level(),
                record.args()
            )
        })
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    Ok(())
}

/// Initialize the logger with a custom log file path
pub fn init_with_path(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let log_file = OpenOptions::new().create(true).append(true).open(path)?;

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            use std::time::SystemTime;

            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap();
            let secs = now.as_secs();

            writeln!(
                buf,
                "[{}] {} - {}",
                format_timestamp(secs),
                record.level(),
                record.args()
            )
        })
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    Ok(())
}

/// Simple timestamp formatter (Unix timestamp to readable format)
fn format_timestamp(secs: u64) -> String {
    // Simple ISO-like format without external dependencies
    // For production use, consider adding chrono crate for better formatting
    format!("timestamp:{}", secs)
}
