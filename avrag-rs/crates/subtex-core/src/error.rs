use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SubtexError {
    #[error("root path does not exist or is not a directory: {0}")]
    RootNotFound(PathBuf),

    #[error("failed to canonicalize root path {path}: {source}")]
    Canonicalize {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("cannot resolve home directory (set HOME or SUBTEX_DATA_DIR)")]
    HomeNotSet,

    #[error("path is not inside the project root {root}: {path}")]
    PathOutsideRoot { root: PathBuf, path: PathBuf },
}
