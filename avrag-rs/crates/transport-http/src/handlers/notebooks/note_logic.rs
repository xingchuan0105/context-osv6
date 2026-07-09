//! Pure domain logic for notebook notes.
//!
//! Extracted from `notes.rs` to keep the HTTP handler focused on transport
//! concerns (auth, request parsing, response serialization). These functions
//! are pure transformations and business rules over contract/preference types.

use uuid::Uuid;

/// Build a preview string from note content: collapse whitespace and truncate
/// to 140 characters with an ellipsis.
pub(super) fn note_preview(content: &str) -> String {
    let collapsed = content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();
    if collapsed.chars().count() <= 140 {
        return collapsed;
    }
    let preview = collapsed.chars().take(140).collect::<String>();
    format!("{preview}...")
}

/// Normalize an optional title into a non-empty string, defaulting to
/// "Untitled note".
pub(super) fn normalize_note_title(title: Option<String>) -> String {
    title
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Untitled note".to_string())
}

/// Convert a note title into a URL/filename-safe slug.
pub(super) fn slugify_note_filename(title: &str) -> String {
    let slug = title
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let collapsed = slug
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if collapsed.is_empty() {
        "note".to_string()
    } else {
        collapsed
    }
}

/// Convert a stored preference into the API-facing note type.
pub(super) fn notebook_note_from_pref(
    note: &contracts::preferences::NotebookNotePreference,
) -> contracts::notebooks::NotebookNote {
    contracts::notebooks::NotebookNote {
        id: note.note_id.clone(),
        notebook_id: note.notebook_id.clone(),
        title: note.title.clone(),
        content: note.content.clone(),
        preview: note_preview(&note.content),
        created_at: note.created_at.clone(),
        updated_at: note.updated_at.clone(),
        promoted_document_id: note.promoted_document_id.clone(),
        promoted_at: note.promoted_at.clone(),
    }
}

/// One-time migration: if a notebook has notes in the legacy
/// `workspace_drafts` format but none in the new `notebook_notes` format,
/// import the draft. Returns `true` if a migration happened (caller should
/// persist the updated preferences).
pub(super) fn migrate_workspace_draft_to_note(
    preferences: &mut contracts::preferences::UserPreferences,
    notebook_id: &str,
) -> bool {
    let has_notes = preferences
        .dashboard
        .notebook_notes
        .iter()
        .any(|note| note.notebook_id == notebook_id);
    if has_notes {
        return false;
    }

    let Some(index) = preferences
        .dashboard
        .workspace_drafts
        .iter()
        .position(|draft| draft.notebook_id == notebook_id && !draft.notes.trim().is_empty())
    else {
        return false;
    };

    let legacy = preferences.dashboard.workspace_drafts.remove(index);
    let now = chrono::Utc::now().to_rfc3339();
    preferences
        .dashboard
        .notebook_notes
        .push(contracts::preferences::NotebookNotePreference {
            note_id: Uuid::new_v4().to_string(),
            notebook_id: notebook_id.to_string(),
            title: "Imported Notes".to_string(),
            content: legacy.notes,
            created_at: now.clone(),
            updated_at: now,
            promoted_document_id: None,
            promoted_at: None,
        });
    true
}
