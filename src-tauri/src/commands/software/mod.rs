// software/mod.rs — Software management module organization
//
// Re-exports all software commands and types for the parent module.

// Submodules - ALL EXTRACTED! 
pub mod installer;
pub mod process_manager;
pub mod residue_cleaner;
pub mod scanner;
pub mod uninstaller;
pub mod version_manager;

// Core software logic (minimal remaining)
#[path = "software_core.rs"]
mod software_core;

// Re-export all public items from core module
pub use software_core::*;

// Re-export submodule functions for use by other modules
pub use installer::{
    extract_and_install, find_binary_in_dir, get_install_base_dir,
    is_valid_version as is_valid_install_version,
};
pub use process_manager::{
    kill_processes_by_name, process_matches_keyword as pm_process_matches_keyword,
};
pub use residue_cleaner::process_matches_keyword as rc_process_matches_keyword;
pub use scanner::InstalledApp;
pub use uninstaller::force_uninstall_residues_blocking;
pub use version_manager::{is_valid_version, merge_versions, safe_get_version};

//  All major submodules extracted!
// Remaining in software_core.rs: Tauri commands that depend on external modules
