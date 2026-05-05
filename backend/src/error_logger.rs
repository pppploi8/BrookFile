use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024;
const MAX_LOG_FILES: usize = 5;

fn get_log_file_path() -> PathBuf {
    let mut path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    path.push("error.log");
    path
}

fn rotate_logs(log_path: &std::path::Path) {
    if let Ok(metadata) = std::fs::metadata(log_path) {
        if metadata.len() <= MAX_LOG_SIZE {
            return;
        }

        for i in (1..MAX_LOG_FILES).rev() {
            let old_path = log_path.with_extension(format!("log.{}", i));
            let new_path = log_path.with_extension(format!("log.{}", i + 1));
            if old_path.exists() {
                let _ = std::fs::rename(&old_path, &new_path);
            }
        }
        let _ = std::fs::rename(log_path, log_path.with_extension("log.1"));
    }
}

pub fn log_error(api_path: &str, error_message: &str) {
    let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");
    let log_entry = format!("[{}] [{}] {}\n", timestamp, api_path, error_message);

    eprint!("{}", log_entry);

    let log_path = get_log_file_path();
    rotate_logs(&log_path);
    match OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        Ok(mut file) => {
            if let Err(e) = file.write_all(log_entry.as_bytes()) {
                eprintln!("[error_logger] Failed to write to log file {:?}: {}", log_path, e);
            }
        }
        Err(e) => {
            eprintln!("[error_logger] Failed to open log file {:?}: {}", log_path, e);
        }
    }
}
