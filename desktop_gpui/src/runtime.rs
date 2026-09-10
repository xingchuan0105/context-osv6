use crate::services::{Phase, Services, Snapshot};
use contracts::{
    chat::{ChatEvent, ChatMessage},
    workspaces::ChatSession,
};
use futures::{
    StreamExt,
    channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded},
};
use std::{path::PathBuf, sync::Arc};
use tokio_util::sync::CancellationToken;

use crate::session_titles::{has_title, title_from_messages};

#[cfg(test)]
mod acceptance;

pub enum Update {
    ServicePhase(Phase),
    ServiceSnapshot(Result<Snapshot, String>),
    ServicesStopped(Result<(), String>),
    Login(Result<desktop_core::LocalSessionStatus, String>),
    Sessions(Option<String>, Result<Vec<ChatSession>, String>),
    Workspace(crate::workspace::Reply),
    SessionTitle(Result<ChatSession, String>),
    History(u64, Result<Vec<ChatMessage>, String>),
    Event(u64, ChatEvent),
    End(u64, Result<(), String>),
}

pub struct Host {
    runtime: tokio::runtime::Runtime,
    sender: UnboundedSender<Update>,
    services: Arc<tokio::sync::Mutex<Services>>,
}

impl Host {
    pub fn new() -> Result<(Self, UnboundedReceiver<Update>), std::io::Error> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()?;
        let (sender, receiver) = unbounded();
        Ok((
            Self {
                runtime,
                sender,
                services: Arc::new(tokio::sync::Mutex::new(Services::new())),
            },
            receiver,
        ))
    }

    pub fn login(&self, data_dir: PathBuf) {
        let sender = self.sender.clone();
        let services = self.services.clone();
        self.runtime.spawn(async move {
            let mut services = services.lock().await;
            let result = services
                .connect(data_dir, |phase| {
                    let _ = sender.unbounded_send(Update::ServicePhase(phase));
                })
                .await;
            let _ = sender.unbounded_send(Update::ServiceSnapshot(services.snapshot().await));
            let _ = sender.unbounded_send(Update::Login(result));
        });
    }

    pub fn refresh_services(&self) {
        let services = self.services.clone();
        let sender = self.sender.clone();
        self.runtime.spawn(async move {
            let services = services.lock().await;
            let _ = sender.unbounded_send(Update::ServiceSnapshot(services.snapshot().await));
        });
    }

    pub fn stop_services(&self) {
        let services = self.services.clone();
        let sender = self.sender.clone();
        self.runtime.spawn(async move {
            let mut services = services.lock().await;
            let result = services.shutdown().await;
            let _ = sender.unbounded_send(Update::ServiceSnapshot(services.snapshot().await));
            let _ = sender.unbounded_send(Update::ServicesStopped(result));
        });
    }

    pub fn workspace(&self, token: String, workspace: Option<String>, request: u64, action: crate::workspace::Action) {
        let sender = self.sender.clone();
        self.runtime.spawn(async move {
            let result = crate::workspace::execute(&token, workspace.as_deref(), &action).await;
            let _ = sender.unbounded_send(Update::Workspace(crate::workspace::Reply { request, workspace, action, result }));
        });
    }

    pub fn sessions(&self, token: String, workspace: Option<String>) {
        let sender = self.sender.clone();
        self.runtime.spawn(async move {
            let result = async {
                let value = desktop_core::api_call(
                    "GET".into(),
                    crate::workspace::session_list_path(workspace.as_deref()),
                    None,
                    Some(token.clone()),
                )
                .await
                .map_err(|e| e.to_string())?;
                let body = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
                let list = web_sdk::conversation_api::parse_session_list(&body)
                    .map_err(|e| e.to_string())?;
                Ok::<Vec<ChatSession>, String>(
                    list.sessions
                        .into_iter()
                        .filter(|s| s.workspace_id == workspace)
                        .collect(),
                )
            }
            .await;
            let sessions = match result {
                Ok(sessions) => sessions,
                Err(error) => {
                    let _ = sender.unbounded_send(Update::Sessions(workspace, Err(error)));
                    return;
                }
            };
            let _ = sender.unbounded_send(Update::Sessions(workspace, Ok(sessions.clone())));
            // Show the list immediately; resolve only missing titles, at most four at once.
            let mut titles = futures::stream::iter(sessions.into_iter().filter(|s| !has_title(s)))
                .map(|session| name_session(token.clone(), session))
                .buffer_unordered(4);
            while let Some(result) = titles.next().await {
                match result {
                    Ok(Some(session)) => {
                        let _ = sender.unbounded_send(Update::SessionTitle(Ok(session)));
                    }
                    Ok(None) => {}
                    Err(error) => {
                        let _ = sender.unbounded_send(Update::SessionTitle(Err(error)));
                    }
                }
            }
        });
    }

    pub fn history(&self, token: String, session_id: String, generation: u64) {
        let sender = self.sender.clone();
        self.runtime.spawn(async move {
            let result = async {
                let value = desktop_core::api_call(
                    "GET".into(),
                    web_sdk::conversation_api::session_messages_url("", &session_id),
                    None,
                    Some(token),
                )
                .await
                .map_err(|e| e.to_string())?;
                let body = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
                web_sdk::conversation_api::parse_message_list(&body)
                    .map(|r| r.messages)
                    .map_err(|e| e.to_string())
            }
            .await;
            let _ = sender.unbounded_send(Update::History(generation, result));
        });
    }

    pub fn chat(
        &self,
        token: String,
        query: String,
        session_id: Option<String>,
        workspace_id: Option<String>,
        doc_scope: Vec<String>,
        generation: u64,
    ) -> CancellationToken {
        self.chat_at(
            desktop_core::product_api_base_url(),
            token,
            query,
            session_id,
            workspace_id,
            doc_scope,
            generation,
        )
    }

    fn chat_at(
        &self,
        base: String,
        token: String,
        query: String,
        session_id: Option<String>,
        workspace_id: Option<String>,
        doc_scope: Vec<String>,
        generation: u64,
    ) -> CancellationToken {
        let cancel = CancellationToken::new();
        let cancellation = cancel.clone();
        let sender = self.sender.clone();
        self.runtime.spawn(async move {
            let request = chat_request(query, session_id, workspace_id, doc_scope);
            let events = sender.clone();
            // Dropping the future cancels even before headers or while upstream is silent.
            let result = tokio::select! {
                _ = cancellation.cancelled() => Ok(()),
                r = desktop_core::stream_chat_sse(&base, &request, Some(&token), || false, move |event| {
                    Ok(events.unbounded_send(Update::Event(generation, event.clone())).is_ok())
                }) => r.map_err(|e| e.to_string()),
            };
            let _ = sender.unbounded_send(Update::End(generation, result));
        });
        cancel
    }
}

fn chat_request(query: String, session_id: Option<String>, workspace_id: Option<String>, doc_scope: Vec<String>) -> serde_json::Value {
    let mut request = serde_json::json!({"query":query,"session_id":session_id,"capabilities":[],"agent_type":"chat","stream":true,"request_id":uuid::Uuid::new_v4().to_string()});
    if let Some(workspace) = workspace_id {
        request["workspace_id"] = workspace.into();
        request["capabilities"] = serde_json::json!(["rag"]);
        request["agent_type"] = "rag".into();
        request["doc_scope"] = doc_scope.into();
    }
    request
}

impl Drop for Host {
    fn drop(&mut self) {
        // A final guard for application quit paths that bypass the window-close callback.
        // The same mutex waits for a pending start before releasing its process records.
        self.runtime.block_on(async {
            if let Err(error) = self.services.lock().await.shutdown().await {
                eprintln!("GPUI service shutdown: {error}");
            }
        });
    }
}

async fn name_session(token: String, session: ChatSession) -> Result<Option<ChatSession>, String> {
    let value = desktop_core::api_call(
        "GET".into(),
        web_sdk::conversation_api::session_messages_url("", &session.id),
        None,
        Some(token.clone()),
    )
    .await
    .map_err(|e| e.to_string())?;
    let body = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    let messages =
        web_sdk::conversation_api::parse_message_list(&body).map_err(|e| e.to_string())?;
    let Some(title) = title_from_messages(&messages.messages) else {
        return Ok(None);
    };
    let body = web_sdk::workspace_api::update_session_json(Some(&title), None)
        .map_err(|e| e.to_string())?;
    let value = desktop_core::api_call(
        "PATCH".into(),
        web_sdk::conversation_api::session_url("", &session.id),
        Some(serde_json::from_slice(&body).map_err(|e| e.to_string())?),
        Some(token),
    )
    .await
    .map_err(|e| e.to_string())?;
    let body = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    web_sdk::conversation_api::parse_session(&body)
        .map(Some)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Read, net::TcpListener, sync::mpsc, time::Duration};

    #[test]
    fn personal_requests_cannot_reuse_workspace_retrieval_scope() {
        let personal = chat_request("question".into(), None, None, vec!["stale-document".into()]);
        assert_eq!(personal["capabilities"], serde_json::json!([]));
        assert_eq!(personal["agent_type"], "chat");
        assert!(personal.get("workspace_id").is_none());
        assert!(personal.get("doc_scope").is_none());
        let workspace = chat_request("question".into(), Some("session-a".into()), Some("workspace-a".into()), vec!["document-a".into()]);
        assert_eq!(workspace["workspace_id"], "workspace-a");
        assert_eq!(workspace["doc_scope"], serde_json::json!(["document-a"]));
        assert_eq!(workspace["capabilities"], serde_json::json!(["rag"]));
    }

    #[test]
    fn cancel_closes_connection_while_waiting_for_headers() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (ready_tx, ready_rx) = mpsc::channel();
        let (closed_tx, closed_rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut buf = [0; 4096];
            let n = stream.read(&mut buf).unwrap();
            assert!(n > 0);
            ready_tx.send(()).unwrap();
            loop {
                match stream.read(&mut buf) {
                    Ok(0) => break,
                    Ok(_) => continue,
                    Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => break,
                    Err(e) => panic!("connection was not cancelled: {e}"),
                }
            }
            closed_tx.send(()).unwrap();
        });
        let (host, _updates) = Host::new().unwrap();
        let cancel = host.chat_at(base, "synthetic".into(), "question".into(), None, None, vec![], 1);
        ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        cancel.cancel();
        closed_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        server.join().unwrap();
    }
}
