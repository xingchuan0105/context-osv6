//! Workspace-local shared state for source viewer integration

use leptos::prelude::*;
use web_sdk::dtos::{Citation, SourceRow};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CitationFocus {
    pub doc_id: String,
    pub chunk_id: Option<String>,
    pub page: Option<usize>,
    pub preview: Option<String>,
    pub content: Option<String>,
    pub chunk_type: Option<String>,
    pub asset_id: Option<String>,
    pub caption: Option<String>,
    pub image_url: Option<String>,
}

impl CitationFocus {
    pub fn from_citation(citation: &Citation) -> Self {
        Self {
            doc_id: citation.doc_id.clone(),
            chunk_id: citation.chunk_id.clone(),
            page: citation.page,
            preview: citation.preview.clone(),
            content: citation.content.clone(),
            chunk_type: citation.chunk_type.clone(),
            asset_id: citation.asset_id.clone(),
            caption: citation.caption.clone(),
            image_url: citation.image_url.clone(),
        }
    }
}

#[derive(Clone)]
pub struct WorkspaceState {
    pub sources: ReadSignal<Vec<SourceRow>>,
    pub set_sources: WriteSignal<Vec<SourceRow>>,
    pub selected_source_ids: ReadSignal<Vec<String>>,
    pub set_selected_source_ids: WriteSignal<Vec<String>>,
    pub selected_document: ReadSignal<Option<SourceRow>>,
    pub set_selected_document: WriteSignal<Option<SourceRow>>,
    pub citation_focus: ReadSignal<Option<CitationFocus>>,
    pub set_citation_focus: WriteSignal<Option<CitationFocus>>,
    pub focus_request: ReadSignal<u64>,
    pub set_focus_request: WriteSignal<u64>,
}

pub fn provide_workspace_state(
    sources: ReadSignal<Vec<SourceRow>>,
    set_sources: WriteSignal<Vec<SourceRow>>,
    selected_source_ids: ReadSignal<Vec<String>>,
    set_selected_source_ids: WriteSignal<Vec<String>>,
    selected_document: ReadSignal<Option<SourceRow>>,
    set_selected_document: WriteSignal<Option<SourceRow>>,
) -> WorkspaceState {
    let (citation_focus, set_citation_focus) = signal(Option::<CitationFocus>::None);
    let (focus_request, set_focus_request) = signal(0_u64);
    let state = WorkspaceState {
        sources,
        set_sources,
        selected_source_ids,
        set_selected_source_ids,
        selected_document,
        set_selected_document,
        citation_focus,
        set_citation_focus,
        focus_request,
        set_focus_request,
    };
    provide_context(state.clone());
    state
}

pub fn use_workspace_state() -> WorkspaceState {
    use_context().expect("WorkspaceState not provided")
}

impl WorkspaceState {
    pub fn request_citation_focus(&self, citation: &Citation) {
        self.set_citation_focus
            .set(Some(CitationFocus::from_citation(citation)));
        self.set_focus_request.update(|value| *value += 1);
    }
}
