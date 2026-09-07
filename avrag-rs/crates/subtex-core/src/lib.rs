//! Subtex directory-plugin core (A-line).
//!
//! The project directory is always the source of truth (PRD D1): everything in
//! this crate derives from it, and nothing here writes back into it. Index
//! stores live under the Subtex application-data directory, keyed by a
//! content hash of the canonical root path.

pub mod error;
pub mod managed_section;
pub mod root;
pub mod scanner;

pub use error::SubtexError;
pub use managed_section::{
    find_managed_section, managed_section_body, upsert_managed_section, ManagedSectionSpan,
    MANAGED_SECTION_BEGIN, MANAGED_SECTION_END,
};
pub use root::{
    default_data_dir, rel_under_root, resolve_under_root, root_hash, RootHandle,
    ROOT_POINTER_FILE, ROOTS_DIR_NAME, SCRATCH_DIR_NAME, STORE_DB_FILE, SUBTEX_DIR_NAME,
};
pub use scanner::{
    file_kind, scan_diff, scan_dir, scan_dir_cached, scan_file, CachedEntry, DirDiff, FileKind,
    ScannedFile,
};
