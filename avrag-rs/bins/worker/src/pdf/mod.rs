mod b_class;
mod context;
mod merge;
mod paddle;
mod parse;
mod plan;

#[cfg(test)]
mod tests;

pub use context::PdfParseContext;
pub use merge::document_ir_from_parsed_document;
pub use parse::execute_pdf_parse;
pub use plan::{ingestion_pdf_max_pages, maybe_truncate_pdf_plan};
