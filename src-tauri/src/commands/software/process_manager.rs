// software/process_manager.rs — Process management utilities
//
// Provides process killing functionality for uninstall operations.

/// Check if a byte is a word character (alphanumeric or underscore)
fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Check if target contains kw as a whole word boundary
fn contains_whole_word(target: &str, kw: &str) -> bool {
    let bytes = target.as_bytes();
    let mut from = 0;
    while let Some(rel) = target[from..].find(kw) {
        let abs = from + rel;
        let before_ok = abs == 0 || !is_word_byte(bytes[abs - 1]);
        let end = abs + kw.len();
        let after_ok = end >= bytes.len() || !is_word_byte(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        from = abs + 1;
    }
    false
}

/// Check if target matches keyword (with version suffix support)
pub fn process_matches_keyword(target: &str, kw: &str) -> bool {
    if target == kw {
        return true;
    }
    // Keyword + version/digit suffix: node → nodejs, node20, code-oss
    if let Some(rest) = target.strip_prefix(kw) {
        let first = rest.chars().next().unwrap_or(' ');
        if first.is_ascii_digit() || !(first.is_ascii_alphanumeric() || first == '_') {
            return true;
        }
    }
    // Long keywords allow word boundary matching (avoid short keyword over-matching)
    if kw.len() >= 4 && contains_whole_word(target, kw) {
        return true;
    }
    false
}

/// Kill processes by name keywords (cross-platform)
/// Returns the number of processes killed
#[cfg(unix)]
pub fn kill_processes_by_name(name_lower: &str) -> usize {
    use sysinfo::{Signal, System};
    let mut system = System::new();
    system.refresh_all();

    let keywords: Vec<String> = name_lower
        .split(|c: char| c.is_whitespace() || c == '-' || c == '_')
        .filter(|s| s.len() >= 3)
        .map(|s| s.to_lowercase())
        .collect();

    if keywords.is_empty() {
        return 0;
    }

    let mut killed = 0;
    for process in system.processes().values() {
        let pname = process.name().to_string_lossy().to_lowercase();
        let exe = process
            .exe()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        let matches = keywords
            .iter()
            .any(|kw| process_matches_keyword(&pname, kw) || process_matches_keyword(&exe, kw));
        if !matches {
            continue;
        }

        // Skip self
        if let Ok(cur) = std::env::current_exe() {
            if let Some(cur_name) = cur.file_stem().and_then(|s| s.to_str()) {
                if process_matches_keyword(&pname, &cur_name.to_lowercase()) {
                    continue;
                }
            }
        }

        if process.kill_with(Signal::Term).is_some() || process.kill_with(Signal::Kill).is_some() {
            killed += 1;
        }
    }
    killed
}

#[cfg(windows)]
pub fn kill_processes_by_name(name_lower: &str) -> usize {
    use sysinfo::System;
    let mut system = System::new();
    system.refresh_all();

    let keywords: Vec<String> = name_lower
        .split(|c: char| c.is_whitespace() || c == '-' || c == '_')
        .filter(|s| s.len() >= 3)
        .map(|s| s.to_lowercase())
        .collect();

    if keywords.is_empty() {
        return 0;
    }

    let mut killed = 0;
    for process in system.processes().values() {
        let pname = process.name().to_string_lossy().to_lowercase();
        let exe = process
            .exe()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        let matches = keywords
            .iter()
            .any(|kw| process_matches_keyword(&pname, kw) || process_matches_keyword(&exe, kw));
        if !matches {
            continue;
        }

        // Skip self
        if let Ok(cur) = std::env::current_exe() {
            if let Some(cur_name) = cur.file_stem().and_then(|s| s.to_str()) {
                if process_matches_keyword(&pname, &cur_name.to_lowercase()) {
                    continue;
                }
            }
        }

        if process.kill() {
            killed += 1;
        }
    }
    killed
}
