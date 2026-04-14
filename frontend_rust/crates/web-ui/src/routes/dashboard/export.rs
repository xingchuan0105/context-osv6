#[cfg(target_arch = "wasm32")]
fn export_text_file(filename: &str, content: &str) -> Result<(), String> {
    use wasm_bindgen::JsCast;

    let array = js_sys::Array::new();
    array.push(&wasm_bindgen::JsValue::from_str(content));
    let blob = web_sys::Blob::new_with_str_sequence(&array)
        .map_err(|_| "failed to create blob".to_string())?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| "failed to create object URL".to_string())?;
    let window = web_sys::window().ok_or_else(|| "missing window".to_string())?;
    let document = window
        .document()
        .ok_or_else(|| "missing document".to_string())?;
    let anchor = document
        .create_element("a")
        .map_err(|_| "failed to create link".to_string())?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| "failed to cast link".to_string())?;
    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn export_text_file(_filename: &str, _content: &str) -> Result<(), String> {
    Err("export is only available after hydration in the browser".to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        ChatSession, sort_workspace_sessions, toggle_favorite_notebook_id, upsert_workspace_draft,
        workspace_draft_notes,
    };
    use web_sdk::dtos::DashboardPreferences;

    fn sample_session(id: &str, updated_at: &str) -> ChatSession {
        ChatSession {
            id: id.to_string(),
            notebook_id: "nb-1".to_string(),
            title: Some(format!("session-{id}")),
            agent_type: "rag".to_string(),
            summary: None,
            pinned: false,
            created_at: "2026-03-29T10:00:00Z".to_string(),
            updated_at: updated_at.to_string(),
        }
    }

    #[test]
    fn toggle_favorite_notebook_id_adds_and_removes() {
        let mut favorites = vec!["nb-1".to_string()];
        toggle_favorite_notebook_id(&mut favorites, "nb-2");
        assert_eq!(favorites, vec!["nb-1".to_string(), "nb-2".to_string()]);

        toggle_favorite_notebook_id(&mut favorites, "nb-1");
        assert_eq!(favorites, vec!["nb-2".to_string()]);
    }

    #[test]
    fn upsert_workspace_draft_adds_updates_and_removes_notes() {
        let mut preferences = DashboardPreferences::default();
        upsert_workspace_draft(
            &mut preferences.workspace_drafts,
            "nb-1",
            "First draft".to_string(),
        );
        assert_eq!(workspace_draft_notes(&preferences, "nb-1"), "First draft");

        upsert_workspace_draft(
            &mut preferences.workspace_drafts,
            "nb-1",
            "Updated draft".to_string(),
        );
        assert_eq!(workspace_draft_notes(&preferences, "nb-1"), "Updated draft");

        upsert_workspace_draft(&mut preferences.workspace_drafts, "nb-1", String::new());
        assert!(workspace_draft_notes(&preferences, "nb-1").is_empty());
    }

    #[test]
    fn sort_workspace_sessions_sorts_pinned_first() {
        let mut pinned_session = sample_session("1", "2026-03-29T09:00:00Z");
        pinned_session.pinned = true;
        let sessions = vec![pinned_session, sample_session("2", "2026-03-29T10:00:00Z")];

        let sorted = sort_workspace_sessions(&sessions);
        assert_eq!(sorted[0].id, "1");
        assert!(sorted[0].pinned);
    }
}
