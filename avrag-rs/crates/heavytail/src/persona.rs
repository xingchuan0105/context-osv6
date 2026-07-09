//! Optional persona card injected into WriteRefine prompts.

use crate::workspace::DraftWorkspace;

/// Lightweight persona metadata for experiment / refine prompts.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PersonaCard {
    pub name: String,
    pub voice: String,
    pub notes: String,
}

/// System-prompt appendix for persona (WriteRefine).
pub fn render_persona_system_zh(card: &PersonaCard) -> String {
    render_persona_appendix_zh(card)
}

/// Render a short Chinese appendix for the refine system prompt.
pub fn render_persona_appendix_zh(card: &PersonaCard) -> String {
    let mut out = String::from("## 人设\n\n");
    if !card.name.is_empty() {
        out.push_str(&format!("- 名称：{}\n", card.name));
    }
    if !card.voice.is_empty() {
        out.push_str(&format!("- 语气：{}\n", card.voice));
    }
    if !card.notes.is_empty() {
        out.push_str(&format!("- 备注：{}\n", card.notes));
    }
    out
}

/// Detect accidental leakage of persona bio phrases into the draft.
pub fn check_persona_leakage(workspace: &DraftWorkspace, card: &PersonaCard) -> Vec<String> {
    let plain = workspace.render_plain();
    let mut leaks = Vec::new();
    for needle in [&card.name, &card.notes] {
        let n = needle.trim();
        if n.chars().count() >= 2 && plain.contains(n) {
            leaks.push(n.to_string());
        }
    }
    leaks
}

/// Hints telling the model how to revise away persona leaks.
pub fn render_leak_revise_hints(leaks: &[String]) -> Vec<String> {
    leaks
        .iter()
        .map(|l| format!("删除或改写正文中的人设泄漏词：「{l}」"))
        .collect()
}
