// software/residue_cleaner.rs — Residue Scanning and Cleanup Utilities
//
// Provides helper functions for residue scanning and cleanup.
// Note: Tauri commands remain in software_core.rs due to dependencies on other modules.

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

/// Check if a byte is a word character (alphanumeric or underscore)
fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}
