//! Subtex directory-plugin core (A-line).
//!
//! The project directory is always the source of truth (PRD D1). Index stores
//! live under the Subtex application-data directory, keyed by a hash of the
//! canonical root path, and are disposable. Inbox accept/undo (F6) is the
//! confirmed exception that moves a file from a drop point into an attached
//! root.

pub mod credits;
pub mod error;
pub mod inbox;
pub mod managed_section;
pub mod root;
pub mod scanner;

pub use credits::millicredits_for;
pub use error::SubtexError;
pub use inbox::{
    accept_pattern, list_drop_point_files, move_into_root, suggest, undo_move, Confidence,
    InboxConfig, InboxRule, Suggestion, INBOX_CONFIG_FILE, PROPOSE_AFTER_ACCEPTS,
};
pub use managed_section::{
    find_managed_section, managed_section_body, upsert_managed_section, ManagedSectionSpan,
    MANAGED_SECTION_BEGIN, MANAGED_SECTION_END,
};
pub use root::{
    default_data_dir, discover_roots, rel_under_root, resolve_under_root, root_hash, RootHandle,
    GLOBAL_DB_FILE, ROOT_POINTER_FILE, ROOTS_DIR_NAME, SCRATCH_DIR_NAME, STORE_DB_FILE,
    SUBTEX_DIR_NAME,
};
pub use scanner::{
    file_kind, scan_diff, scan_dir, scan_dir_cached, scan_file, CachedEntry, DirDiff, FileKind,
    ScannedFile,
};
