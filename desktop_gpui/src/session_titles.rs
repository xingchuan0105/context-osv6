use contracts::{chat::ChatMessage, workspaces::ChatSession};

/// A conversation is named from its first user message, without a model call.
pub fn title_from_messages(messages: &[ChatMessage]) -> Option<String> {
    let query = messages
        .iter()
        .find(|message| message.role == "user" && !message.content.trim().is_empty())?;
    let normalized = query
        .content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut chars = normalized.chars();
    let mut title: String = chars.by_ref().take(48).collect();
    if chars.next().is_some() {
        title.push('…');
    }
    Some(title)
}

pub fn has_title(session: &ChatSession) -> bool {
    session
        .title
        .as_deref()
        .is_some_and(|title| !title.trim().is_empty())
}

pub fn session_label(session: &ChatSession) -> String {
    if let Some(title) = session
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
    {
        return title.to_owned();
    }
    // Empty or still-loading conversations remain distinguishable in the sidebar.
    let timestamp: String = session.created_at.chars().take(16).collect();
    let short_id: String = session.id.chars().take(8).collect();
    format!("对话 · {} · {short_id}", timestamp.replace('T', " "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn message(role: &str, content: &str) -> ChatMessage {
        serde_json::from_value(json!({
            "id":1,"session_id":"session","role":role,"content":content,"created_at":"2026-09-10"
        }))
        .unwrap()
    }

    #[test]
    fn title_uses_first_user_question_and_normalizes_whitespace() {
        let messages = [
            message("system", "system context"),
            message("user", " \n "),
            message("assistant", "previous output"),
            message("user", "  比较中文\n\t和 English 😀  "),
            message("user", "another question"),
        ];
        assert_eq!(
            title_from_messages(&messages).as_deref(),
            Some("比较中文 和 English 😀")
        );
        assert_eq!(title_from_messages(&[message("assistant", "answer")]), None);
    }

    #[test]
    fn long_titles_keep_valid_unicode_and_indicate_shortening() {
        let query = "界".repeat(48);
        assert_eq!(
            title_from_messages(&[message("user", &query)]),
            Some(query.clone())
        );
        assert_eq!(
            title_from_messages(&[message("user", &(query.clone() + "😀"))]),
            Some(query + "…")
        );
    }

    #[test]
    fn stored_titles_are_preserved_and_empty_rows_are_distinguishable() {
        let mut session: ChatSession = serde_json::from_value(json!({
            "id":"abc12345-a","owner_user_id":"user","scope_kind":"personal",
            "title":"  自定名称  ","agent_type":"chat","model_role":"quick_chat",
            "created_at":"2026-09-10T08:58:00Z","updated_at":"2026-09-10T08:58:00Z"
        }))
        .unwrap();
        assert!(has_title(&session));
        assert_eq!(session_label(&session), "自定名称");
        session.title = Some("  ".into());
        assert!(!has_title(&session));
        let first = session_label(&session);
        session.id = "def67890-b".into();
        assert_ne!(first, session_label(&session));
        assert!(first.contains("2026-09-10 08:58"));
    }
}
