# Software Module Implementation Guide

## Overview
This document provides step-by-step instructions for splitting `src/commands/software.rs` (2,267 lines) into modular submodules.

## Prerequisites
- Current file: `src/commands/software.rs` (2,267 lines)
- Already split: `software_data.rs`, `software_pm.rs`
- Target structure: `src/commands/software/` directory with 6 submodules

## Step-by-Step Extraction Plan

### Phase 1: Extract `scanner.rs` (Lines ~634-1250)

**Functions to extract:**
```rust
// Lines 634-652
fn is_system_desktop(path: &std::path::Path) -> bool

// Lines 656-744
#[cfg(target_os = "linux")]
fn list_gui_apps() -> Vec<InstalledApp>

// Lines 748-773
#[cfg(target_os = "linux")]
fn desktop_display_name(path: &std::path::Path) -> String

// Lines 775-839
#[cfg(target_os = "linux")]
fn desktop_version(path: &std::path::Path) -> String

// Lines 841-895
#[cfg(target_os = "linux")]
fn read_desktop_icon(path: &std::path::Path) -> Option<String>

// Lines 897-997
#[cfg(target_os = "linux")]
fn desktop_display_name_for_package(package: &str) -> Option<String>

// Lines 999-1104
#[cfg(target_os = "linux")]
fn resolve_app_icon(app_name: &str, _source: &str) -> Option<String>

// Lines 1106-1119
fn desktop_name_of(path: &std::path::Path) -> String

// Lines 1121-1203
fn desktop_icon_path(desktop: &std::path::Path) -> Option<PathBuf>

// Lines 1205-1250
fn read_image_as_data_url(path: &std::path::Path) -> Option<String>
```

**Dependencies:**
- Uses `normalize_app_name()` - keep in main module or move
- Uses `merge_versions()` - keep in main module or move
- Uses `dirs_home_dir()` - helper function
- Imports: `std::fs`, `std::path::PathBuf`, `serde::{Serialize, Deserialize}`

**Create file:** `src/commands/software/scanner.rs`

---

### Phase 2: Extract `version_manager.rs` (Lines ~1252-1550)

**Functions to extract:**
```rust
// Lines ~1252-1300
async fn safe_get_version(cmd: &str) -> String

// Lines ~1302-1350
async fn get_versions_from_github(owner: &str, repo: &str) -> Result<Vec<String>, String>

// Lines ~1352-1400
async fn get_versions_from_nodejs_dist() -> Result<Vec<String>, String>

// Lines ~1402-1450
async fn get_versions_from_go_download() -> Result<Vec<String>, String>

// Lines ~1452-1500
#[tauri::command]
pub async fn fetch_software_versions(name: String) -> Result<Vec<String>, String>

// Lines ~1502-1520
fn is_valid_version(v: &str) -> bool

// Lines ~1522-1550
fn merge_versions(existing: &mut String, new_version: &str)
```

**Dependencies:**
- Uses `GUI_APPS` from `software_data`
- Requires `tokio`, `reqwest`
- Imports: `serde_json`, `regex`

**Create file:** `src/commands/software/version_manager.rs`

---

### Phase 3: Extract `installer.rs` (Lines ~1552-1850)

**Functions to extract:**
```rust
// Lines ~1552-1600
#[tauri::command]
fn get_install_base_dir() -> PathBuf

// Lines ~1602-1650
fn find_binary_in_dir(dir: &std::path::Path, name: &str) -> Option<PathBuf>

// Lines ~1652-1700
fn is_valid_version(v: &str) -> bool

// Lines ~1702-1850
#[tauri::command]
async fn install_software_from_url(
    name: String,
    version: String,
    url: String,
) -> Result<String, String>

// Helper functions for extraction and installation
fn extract_and_install(...) -> Result<String, String>
```

**Dependencies:**
- Uses platform-specific logic (Linux/macOS/Windows)
- Requires `tokio::task::spawn_blocking`
- Imports: `std::fs`, `std::process::Command`, `flate2`, `tar`

**Create file:** `src/commands/software/installer.rs`

---

### Phase 4: Extract `uninstaller.rs` (Lines ~1852-2050)

**Functions to extract:**
```rust
// Lines ~1852-1900
#[tauri::command]
pub async fn uninstall_software_deep(package_name: String) -> Result<String, String>

// Lines ~1902-1950
#[tauri::command]
pub async fn uninstall_software_deep_with_source(
    package_name: String,
    source: String,
) -> Result<String, String>

// Lines ~1952-2050
#[tauri::command]
fn force_uninstall_residues_blocking(app_name: &str, package_name: &str) -> Vec<String>
```

**Dependencies:**
- Calls `scan_app_residues()` from residue_cleaner
- May call `kill_processes_by_name()` from process_manager
- Platform-specific cleanup logic

**Create file:** `src/commands/software/uninstaller.rs`

---

### Phase 5: Extract `residue_cleaner.rs` (Lines ~2052-2200)

**Functions to extract:**
```rust
// Lines ~2052-2100
#[tauri::command]
pub fn scan_app_residues(
    app_name: String,
    package_name: String,
) -> Result<Vec<ResidueEntry>, String>

// Lines ~2102-2150
#[tauri::command]
pub fn clean_specific_residues(paths: Vec<String>) -> Result<u64, String>

// Helper functions
fn process_matches_keyword(target: &str, kw: &str) -> bool
fn contains_whole_word(target: &str, kw: &str) -> bool
fn is_word_byte(b: u8) -> bool
```

**Dependencies:**
- File system scanning
- Pattern matching utilities
- Imports: `std::fs`, `glob`, `walkdir`

**Create file:** `src/commands/software/residue_cleaner.rs`

---

### Phase 6: Extract `process_manager.rs` (Lines ~2200-2267)

**Functions to extract:**
```rust
// Lines ~2200-2267
fn kill_processes_by_name(name_lower: &str) -> usize
```

**Dependencies:**
- Platform-specific process management
- Linux: `pgrep`, `kill`
- macOS: `pkill`
- Windows: `taskkill`

**Create file:** `src/commands/software/process_manager.rs`

---

## Final Structure

After all phases, the directory should look like:

```
src/commands/software/
├── mod.rs                  # Module declarations + re-exports
├── scanner.rs              # GUI app scanning (~600 lines)
├── version_manager.rs      # Version detection (~300 lines)
├── installer.rs            # Installation logic (~300 lines)
├── uninstaller.rs          # Deep uninstall (~200 lines)
├── residue_cleaner.rs      # Residue scanning (~150 lines)
└── process_manager.rs      # Process killing (~70 lines)
```

And `src/commands/mod.rs` should have:
```rust
pub mod software;  // Changed from `pub mod tuning;` file to directory
```

## Implementation Checklist

- [ ] Create directory structure
- [ ] Extract scanner.rs (Phase 1)
- [ ] Extract version_manager.rs (Phase 2)
- [ ] Extract installer.rs (Phase 3)
- [ ] Extract uninstaller.rs (Phase 4)
- [ ] Extract residue_cleaner.rs (Phase 5)
- [ ] Extract process_manager.rs (Phase 6)
- [ ] Create mod.rs with proper re-exports
- [ ] Update parent module imports
- [ ] Run `cargo check`
- [ ] Run `cargo clippy -- -D warnings`
- [ ] Run `cargo test`
- [ ] Manual testing on each platform

## Key Considerations

1. **Preserve `#[cfg]` attributes**: All platform-specific code must keep its conditional compilation guards
2. **Maintain Tauri command signatures**: All `#[tauri::command]` functions must remain accessible
3. **Keep public API stable**: Re-export all necessary types and functions from `mod.rs`
4. **Handle cross-module dependencies**: Some functions may need to be moved together or kept as shared utilities
5. **Test thoroughly**: Each phase should compile independently before moving to the next

## Estimated Time
- **Total**: 3-4 hours
- **Per phase**: 30-45 minutes
- **Testing**: 30 minutes

## Risk Assessment
- **Risk Level**: Medium
- **Reason**: Large file with many interdependencies
- **Mitigation**: Extract one module at a time, verify compilation after each phase
