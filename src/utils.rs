use std::time::Duration;

// Constants
pub const MAX_PROCESSES: usize = 1000;
pub const MAX_FILES: usize = 10000;
pub const MAX_NETWORKS: usize = 100;
pub const PAGE_SIZE: usize = 10;
pub const NETWORK_HISTORY_SIZE: usize = 60;
pub const UPDATE_INTERVAL: Duration = Duration::from_secs(2);
pub const MANUAL_REFRESH_COOLDOWN: Duration = Duration::from_millis(500);
pub const PROCESS_NAME_MAX_LEN: usize = 35;
pub const INTERFACE_NAME_MAX_LEN: usize = 20;
pub const FILE_NAME_MAX_LEN: usize = 40;

/// Format memory size in bytes to human-readable string
pub fn format_memory_size(bytes: u64) -> String {
    let mb = bytes / 1024 / 1024;
    if mb >= 1024 {
        let gb = mb as f64 / 1024.0;
        format!("{:.1} GB", gb)
    } else {
        format!("{} MB", mb)
    }
}

/// Format network size in bytes to human-readable string
pub fn format_network_size(bytes: u64) -> String {
    let kb = bytes / 1024;
    if kb < 1024 {
        format!("{} KB", kb)
    } else if kb < 1024 * 1024 {
        let mb = kb / 1024;
        format!("{} MB", mb)
    } else {
        let gb = kb as f64 / (1024.0 * 1024.0);
        format!("{:.1} GB", gb)
    }
}

/// Format network rate in bytes per second to human-readable string
pub fn format_network_rate(bytes_per_second: u64) -> String {
    let kb_per_sec = bytes_per_second / 1024;
    if kb_per_sec >= 1024 {
        let mb_per_sec = kb_per_sec as f64 / 1024.0;
        format!("{:.1} MB/s", mb_per_sec)
    } else {
        format!("{} KB/s", kb_per_sec)
    }
}

/// Truncate string to specified length with ellipsis if needed
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
