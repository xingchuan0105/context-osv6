pub mod context;
pub mod merge;
pub mod retrieval; // export retrieval functions
pub mod runtime;

pub use retrieval::{
    DenseSearchHit, ScoredChunk, SparseSearchHit, run_dense_retrieval, run_sparse_retrieval,
};
pub use runtime::{RagConfig, RagRuntime};
