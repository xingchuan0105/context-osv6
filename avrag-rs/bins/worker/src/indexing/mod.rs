mod env;
mod media;
mod multimodal;
mod ocr_gating;
mod types;
mod vlm_summary;

pub use env::{env_flag_enabled, triplet_batch_token_budget, vlm_summary_enabled};
pub use media::{MediaResolveContext, resolve_visual_chunk_image_refs};
pub use multimodal::build_multimodal_index_records;
pub use types::{StoredMultimodalChunk, record_multimodal_degrade};
pub use vlm_summary::maybe_enrich_visual_multimodal_summaries;
