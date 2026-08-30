// software/mod.rs — Software management module organization
//
// Re-exports all software commands and types for the parent module.

// Submodules
pub mod process_manager;

// Core software logic (temporary - will be split into submodules)
#[path = "software_core.rs"]
mod software_core;

// Re-export all public items from core module
pub use software_core::*;

// Re-export process manager functions for use by other modules
pub use process_manager::{kill_processes_by_name, process_matches_keyword};

// TODO: As functions are extracted to submodules, remove them from software_core.rs
// Final structure will be:
// pub mod scanner;
// pub mod version_manager;
// pub mod installer;
// pub mod uninstaller;
// pub mod residue_cleaner;
