mod config;
mod crw;
mod executor;
mod provider;
mod proxy;
mod types;

pub use config::SearchConfig;
pub use crw::{DEFAULT_CRW_BASE_URL, auto_scrape_enrich_results, scrape_url};
pub use executor::{SearchExecutor, SearchProvider};
pub use proxy::{build_http_client_with_proxy, resolved_proxy_url, sync_resolved_proxy_env};
pub use types::{SearchResponse, SearchResult, SearchStreamUpdate};

#[cfg(test)]
mod tests_impl;
