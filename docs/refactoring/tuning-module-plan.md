# Tuning Module Refactoring Plan

## Current State
- **File**: `src/commands/tuning.rs` (1,827 lines)
- **Complexity**: Contains multiple platform-specific optimization features
- **Status**: Single monolithic file with mixed responsibilities

## Proposed Structure
```
src/commands/tuning/
├── mod.rs                  # Public API, re-exports
├── disk_cleanup.rs         # Cache scanning & path cleaning (~400 lines)
├── exclusions.rs           # Exclusion list management (~150 lines)
├── linux_tuning.rs         # Linux system tuning (Swap/DNS/Firewall/etc.) (~600 lines)
└── windows_tuning.rs       # Windows optimization (WinSxS/Startup/Hibernation) (~300 lines)
```

## Split Details

### 1. `disk_cleanup.rs` - Disk Cleanup Core
**Functions to move:**
- `scan_caches()` (tauri command)
- `clean_paths()` (tauri command)
- `get_disk_usage()` (tauri command)
- `optimize_disk()` (tauri command)
- `dir_size()` - helper
- `default_candidates()` - helper
- `is_cleanable()` - helper
- `same_path()` - helper
- `cleanup_ds_store()` - macOS specific
- Platform-specific cleanup implementations

**Rationale**: All related to scanning and cleaning temporary/cache files across platforms.

### 2. `exclusions.rs` - Exclusion Management
**Functions to move:**
- `list_exclusions()` (tauri command)
- `add_exclusion()` (tauri command)
- `remove_exclusion()` (tauri command)
- `load_exclusions_impl()` - private helper
- `save_exclusions_impl()` - private helper
- `exclusions_path()` - private helper
- `Exclusions` struct

**Rationale**: Self-contained JSON persistence for user exclusion preferences.

### 3. `linux_tuning.rs` - Linux System Tuning
**Functions to move:**
- `get_tuning_overview()` (tauri command)
- `verify_sudo_password()` (tauri command)
- `get_swap_info()` (tauri command)
- `set_swap()` (tauri command)
- `disable_swap()` (tauri command)
- `get_dns_config()` (tauri command)
- `set_dns()` (tauri command)
- `get_timezone_info()` (tauri command)
- `set_timezone()` (tauri command)
- `get_firewall_status()` (tauri command)
- `set_firewall()` (tauri command)
- `get_system_limits()` (tauri command)
- `scan_cleanup_targets()` (tauri command)
- `clean_targets()` (tauri command)
- Helper functions: `run_cmd()`, `run_privileged()`, `running_as_root()`, etc.
- Related structs: `SwapInfo`, `DnsConfig`, `TimezoneInfo`, `FirewallStatus`, etc.

**Rationale**: All Linux-specific system administration tasks requiring sudo/root access.

### 4. `windows_tuning.rs` - Windows Optimization
**Functions to move:**
- `win_scan_cleanup()` (tauri command)
- `win_clean_paths()` (tauri command)
- `win_winsxs_cleanup()` (tauri command)
- `win_get_hibernation()` (tauri command)
- `win_set_hibernation()` (tauri command)
- `win_list_startup()` (tauri command)
- `win_set_startup()` (tauri command)
- `win_storage_usage()` (tauri command)
- Helper functions and structs: `WinCleanItem`, `HibernationStatus`, `WinStartupEntry`, etc.

**Rationale**: Windows-specific optimizations using PowerShell, DISM, registry operations.

### 5. Keep in `mod.rs`
- Re-exports of all public types
- Common structs that cross module boundaries (if any)
- Module declarations

## Implementation Steps

1. **Create directory structure** 
   ```bash
   mkdir -p src/commands/tuning
   ```

2. **Extract `exclusions.rs` first** (simplest, no dependencies)
   - Move exclusion-related code
   - Update imports
   - Verify compilation

3. **Extract `disk_cleanup.rs`**
   - Move cache scanning and cleaning logic
   - Handle platform-specific helpers
   - Update imports
   - Verify compilation

4. **Extract `linux_tuning.rs`**
   - Move all Linux system tuning commands
   - Ensure proper `#[cfg(target_os = "linux")]` guards
   - Update imports
   - Verify compilation

5. **Extract `windows_tuning.rs`**
   - Move all Windows optimization commands
   - Ensure proper `#[cfg(target_os = "windows")]` guards
   - Update imports
   - Verify compilation

6. **Create `mod.rs`**
   - Add module declarations
   - Re-export public types and commands
   - Update parent module imports

7. **Run comprehensive tests**
   - `cargo test`
   - `cargo clippy -- -D warnings`
   - Manual testing on each platform

## Benefits

- **Maintainability**: Each file <600 lines, focused responsibility
- **Platform clarity**: Linux and Windows code clearly separated
- **Testability**: Easier to write platform-specific tests
- **Readability**: Clear separation between cleanup, exclusions, and system tuning
- **Compilation**: Faster incremental builds for platform-specific changes

## Estimated Effort
- **Time**: 2-3 hours
- **Risk**: Low (pure refactoring, no behavior changes)
- **Testing**: Existing functionality should work unchanged

## Dependencies
- No external dependencies added
- Internal module references only
- Maintains existing Tauri command signatures

## Notes
- Preserve all `#[cfg]` conditional compilation attributes
- Keep all `#[tauri::command]` annotations
- Maintain backward compatibility for frontend calls
- All error handling patterns remain unchanged
