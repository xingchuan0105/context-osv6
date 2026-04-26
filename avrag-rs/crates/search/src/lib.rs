mod config;
mod executor;
mod lexical;
mod provider;
mod types;

pub use config::SearchConfig;
pub use executor::SearchExecutor;
pub use lexical::{LexicalChunkDocument, LexicalSearchHit, TantivyLexicalIndex};
pub use types::{SearchResponse, SearchResult, SearchStreamUpdate};

#[cfg(test)]
mod tests_impl;
