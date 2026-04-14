use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryMetadata {
    pub doc_id: String,
    pub filename: String,
    pub docname: String,
    pub language: String,
    pub domain: String,
    pub genre: String,
    pub era: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryOutput {
    pub summary_text: String,
    pub summary_metadata: SummaryMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocScopeProfile {
    pub languages: Vec<String>,
    pub domains: Vec<String>,
    pub genres: Vec<String>,
    pub eras: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocScopeMetadata {
    pub documents: Vec<SummaryMetadata>,
    pub profile: DocScopeProfile,
}
