//! Mock HTTP servers for Product E2E (LLM, Embedding, Search).

pub(crate) use super::mock_embedding_server::start_mock_embedding_server;
pub(crate) use super::mock_llm_server::{MockLlmRoute, start_mock_llm_server};
pub(crate) use super::mock_office_server::{
    MOCK_OFFICE_DOCX_TEXT, MOCK_OFFICE_PPTX_TEXT, MOCK_OFFICE_XLSX_TEXT,
    start_mock_office_parser_server,
};
pub(crate) use super::mock_paddle_server::{
    MOCK_PADDLE_IMAGE_OCR_TEXT, start_mock_paddle_ocr_server,
};
pub(crate) use super::mock_search_server::{MockSearchControls, start_mock_search_server};

pub(crate) use super::mock_rag_codegen::{
    format_mock_rag_chunk_fetch_codegen, format_mock_rag_codegen_response,
    format_mock_rag_doc_profile_codegen,
};

pub(crate) use super::mock_rag_state::{
    reset_mock_rag_state, set_mock_emit_memory_tool, set_mock_rag_codegen_chunk_id,
    set_mock_rag_codegen_chunk_ids, set_mock_rag_codegen_doc_id, set_mock_rag_codegen_query,
    set_mock_rag_multiround_profile, set_mock_rag_skill_request_memory, set_mock_rag_skip_codegen,
};
