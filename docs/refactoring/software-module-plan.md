# Software Module Refactoring Plan

## Current State
- **File**: `src/commands/software.rs` (2,267 lines)
- **Already split**:
  - `software_data.rs` - Static data (URLs, software definitions)
  - `software_pm.rs` - Package manager install/uninstall logic

## Proposed Structure
```
src/commands/software/
├── mod.rs              # Public API, re-exports
├── installer.rs        # Installation logic (~400 lines)
├── uninstaller.rs      # Uninstallation logic (~350 lines)
├── scanner.rs          # App scanning & desktop file parsing (~500 lines)
├── version_manager.rs  # Version detection & management (~300 lines)
├── residue_cleaner.rs  # Residue scanning & cleanup (~400 lines)
└── process_manager.rs  # Process killing utilities (~100 lines)
```

## Split Details

### 1. `scanner.rs` - GUI Application Scanner
**Functions to move:**
- `list_gui_apps()`
- `is_system_desktop()`
- `desktop_display_name()`
- `desktop_version()`
- `read_desktop_icon()`
- `desktop_name_of()`
- `desktop_icon_path()`
- `read_image_as_data_url()`
- `resolve_app_icon()`

**Rationale**: All related to scanning `.desktop` files and building the installed apps list.

### 2. `version_manager.rs` - Version Detection
**Functions to move:**
- `safe_get_version()`
- `get_versions_from_github()`
- `get_versions_from_nodejs_dist()`
- `get_versions_from_go_download()`
- `fetch_software_versions()` (tauri command)
- `is_valid_version()`
- `merge_versions()`

**Rationale**: Centralized version detection logic across different sources.

### 3. `installer.rs` - Installation Logic
**Functions to move:**
- `extract_and_install()` (tauri command)
- `get_install_base_dir()`
- `find_binary_in_dir()`
- `install_software_from_url()` (tauri command)

**Rationale**: Download, extract, and install workflow.

### 4. `uninstaller.rs` - Deep Uninstall
**Functions to move:**
- `uninstall_software_deep()` (tauri command)
- `uninstall_software_deep_with_source()` (tauri command)
- `force_uninstall_residues_blocking()`

**Rationale**: Deep cleanup after package manager uninstall.

### 5. `residue_cleaner.rs` - Residue Scanning & Cleanup
**Functions to move:**
- `scan_app_residues()` (tauri command)
- `clean_specific_residues()` (tauri command)
- `process_matches_keyword()`
- `contains_whole_word()`
- `is_word_byte()`

**Rationale**: Finding and cleaning leftover config/cache/data files.

### 6. `process_manager.rs` - Process Management
**Functions to move:**
- `kill_processes_by_name()`

**Rationale**: Utility for killing processes before uninstall.

## Implementation Steps

1. Create directory structure
2. Extract each module into its own file
3. Update imports and visibility
4. Add comprehensive tests for each module
5. Run `cargo test` to verify no regressions
6. Update documentation

## Benefits
- **Maintainability**: Each file <500 lines, focused responsibility
- **Testability**: Easier to write unit tests for isolated modules
- **Readability**: Clear separation of concerns
- **Compilation**: Faster incremental builds

## Estimated Effort
- **Time**: 2-3 hours
- **Risk**: Low (pure refactoring, no behavior changes)
- **Testing**: Existing tests should cover most functionality
