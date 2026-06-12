mod env;
mod media;
mod multimodal;
mod page_status;
mod types;
mod vlm_summary;

pub use env::env_flag_enabled;
pub use media::{resolve_visual_chunk_image_refs, MediaResolveContext};
pub use multimodal::build_multimodal_index_records;
pub use types::{record_multimodal_degrade, StoredMultimodalChunk};
pub use vlm_summary::maybe_enrich_visual_multimodal_summaries;
