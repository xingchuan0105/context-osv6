/// Phase 0 极简路由分发（仅收敛于 /chat 最小垂直验证）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppRoute {
    Chat { session_id: Option<String> },
    NotFound,
}

impl AppRoute {
    pub fn parse(path: &str) -> Self {
        let trimmed = path.trim_matches('/');
        let segments: Vec<&str> = if trimmed.is_empty() {
            vec![]
        } else {
            trimmed.split('/').collect()
        };

        match segments.as_slice() {
            ["chat"] => Self::Chat { session_id: None },
            ["chat", sid] => Self::Chat {
                session_id: Some(sid.to_string()),
            },
            _ => Self::NotFound,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppRoute;

    #[test]
    fn chat_routes_are_canonical_and_root_is_not_chat() {
        assert_eq!(AppRoute::parse("/"), AppRoute::NotFound);
        assert_eq!(
            AppRoute::parse("/chat"),
            AppRoute::Chat { session_id: None }
        );
        assert_eq!(
            AppRoute::parse("/chat/session-1"),
            AppRoute::Chat {
                session_id: Some("session-1".to_string())
            }
        );
    }
}
