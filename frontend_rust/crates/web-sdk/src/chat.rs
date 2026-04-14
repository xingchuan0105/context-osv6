//! Chat API client

use crate::{ApiClient, dtos::*};

impl ApiClient {
    #[cfg(not(target_arch = "wasm32"))]
    fn build_chat_request(
        &self,
        req: &ChatRequest,
        request_id: Option<&str>,
    ) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, "/api/v1/chat");
        let mut builder = self.client.post(&url).json(req);
        if let Some(ref token) = self.auth_token {
            builder = builder.header("Authorization", format!("Bearer {}", token));
        }
        if let Some(request_id) = request_id {
            builder = builder.header("x-request-id", request_id);
        }
        builder
    }

    /// POST /api/v1/chat (non-streaming)
    pub async fn chat(
        &self,
        req: &ChatRequest,
        request_id: Option<&str>,
    ) -> anyhow::Result<ChatResponse> {
        #[cfg(target_arch = "wasm32")]
        {
            use gloo_net::http::Request;

            let url = format!("{}{}", self.base_url, "/api/v1/chat");
            let mut request = Request::post(&url).header("Content-Type", "application/json");
            if let Some(ref token) = self.auth_token {
                request = request.header("Authorization", &format!("Bearer {}", token));
            }
            if let Some(request_id) = request_id {
                request = request.header("x-request-id", request_id);
            }
            let response = request.body(serde_json::to_string(req)?)?.send().await?;
            if !response.ok() {
                anyhow::bail!("API error: {}", response.status());
            }
            return Ok(response.json().await?);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let response = self.build_chat_request(req, request_id).send().await?;
            if !response.status().is_success() {
                anyhow::bail!("API error: {}", response.status());
            }
            let body = response.bytes().await?;
            Ok(serde_json::from_slice(&body)?)
        }
    }

    /// GET /api/v1/chat/sessions
    pub async fn list_chat_sessions(
        &self,
        notebook_id: Option<&str>,
    ) -> anyhow::Result<ChatSessionListResponse> {
        let path = notebook_id
            .map(|id| format!("/api/v1/chat/sessions?notebook_id={id}"))
            .unwrap_or_else(|| "/api/v1/chat/sessions".to_string());
        self.get(&path).await
    }

    /// POST /api/v1/chat/sessions
    pub async fn create_chat_session(
        &self,
        req: &CreateChatSessionRequest,
    ) -> anyhow::Result<ChatSession> {
        self.post("/api/v1/chat/sessions", req).await
    }

    /// GET /api/v1/chat/sessions/{session_id}
    pub async fn get_chat_session(&self, session_id: &str) -> anyhow::Result<ChatSession> {
        self.get(&format!("/api/v1/chat/sessions/{}", session_id))
            .await
    }

    /// PUT /api/v1/chat/sessions/{session_id}
    pub async fn update_chat_session(
        &self,
        session_id: &str,
        req: &UpdateChatSessionRequest,
    ) -> anyhow::Result<ChatSession> {
        self.put(&format!("/api/v1/chat/sessions/{}", session_id), req)
            .await
    }

    /// DELETE /api/v1/chat/sessions/{session_id}
    pub async fn delete_chat_session(&self, session_id: &str) -> anyhow::Result<EmptyResponse> {
        self.delete(&format!("/api/v1/chat/sessions/{}", session_id))
            .await
    }

    /// GET /api/v1/chat/sessions/{session_id}/messages
    pub async fn get_chat_messages(
        &self,
        session_id: &str,
    ) -> anyhow::Result<ChatMessageListResponse> {
        self.get(&format!("/api/v1/chat/sessions/{}/messages", session_id))
            .await
    }

    /// POST /api/v1/chat/citations/lookup
    pub async fn citation_lookup(
        &self,
        req: &CitationLookupRequest,
    ) -> anyhow::Result<CitationLookupResponse> {
        self.post("/api/v1/chat/citations/lookup", req).await
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use reqwest::header::{AUTHORIZATION, HeaderName};

    fn sample_request() -> ChatRequest {
        ChatRequest {
            query: "hello".to_string(),
            notebook_id: Some("nb-1".to_string()),
            session_id: None,
            agent_type: "search".to_string(),
            source_type: None,
            source_token: None,
            doc_scope: Vec::new(),
            messages: Vec::new(),
            stream: false,
        }
    }

    #[test]
    fn native_chat_request_builder_sets_transport_request_id_header() {
        let client = ApiClient::new("http://example.test").with_auth("token-123".to_string());
        let request = client
            .build_chat_request(&sample_request(), Some("req-123"))
            .build()
            .expect("request should build");

        assert_eq!(request.method(), reqwest::Method::POST);
        assert_eq!(request.url().as_str(), "http://example.test/api/v1/chat");
        assert_eq!(
            request
                .headers()
                .get(HeaderName::from_static("x-request-id")),
            Some(&"req-123".parse().unwrap())
        );
        assert_eq!(
            request.headers().get(AUTHORIZATION),
            Some(&"Bearer token-123".parse().unwrap())
        );
    }

    #[test]
    fn native_chat_request_builder_omits_request_id_when_not_provided() {
        let client = ApiClient::new("http://example.test");
        let request = client
            .build_chat_request(&sample_request(), None)
            .build()
            .expect("request should build");

        assert!(
            !request
                .headers()
                .contains_key(HeaderName::from_static("x-request-id")),
            "transport-only request id header should stay optional"
        );
    }
}
