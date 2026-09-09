//! Adapter-level counterparts to the desktop suite; no WebView or real provider.
use super::*;
use crate::session::Conversation;
use futures::StreamExt;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    time::Duration,
};
use web_sdk::TurnStatus;

fn request(stream: &mut TcpStream) -> serde_json::Value {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut byte = [0];
    while !bytes.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
        assert!(bytes.len() < 16_384);
    }
    let headers = String::from_utf8(bytes).unwrap();
    assert!(headers.starts_with("POST /api/v1/chat "));
    assert!(
        headers
            .to_lowercase()
            .contains("authorization: bearer synthetic")
    );
    let size = headers
        .lines()
        .find_map(|l| {
            l.to_lowercase()
                .strip_prefix("content-length:")
                .map(|s| s.trim().parse::<usize>().unwrap())
        })
        .unwrap();
    let mut body = vec![0; size];
    stream.read_exact(&mut body).unwrap();
    serde_json::from_slice(&body).unwrap()
}

fn next(host: &Host, updates: &mut UnboundedReceiver<Update>) -> Update {
    host.runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(5), updates.next())
            .await
            .unwrap()
            .unwrap()
    })
}

#[test]
fn shared_fixture_streams_before_completion_and_preserves_personal_scope() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (release, resume) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let body = request(&mut socket);
        assert_eq!(body["capabilities"], serde_json::json!([]));
        assert_eq!(body["agent_type"], "chat");
        assert_eq!(body["session_id"], "sess-100");
        let fixture = include_str!("../../../frontend_rust/tests/fixtures/stream-normal-long.json");
        let frames: Vec<String> = fixture
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let value: serde_json::Value = serde_json::from_str(l).unwrap();
                format!("event: {}\ndata: {l}\n\n", value["event"].as_str().unwrap())
            })
            .collect();
        let size: usize = frames.iter().map(String::len).sum();
        write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {size}\r\nConnection: close\r\n\r\n").unwrap();
        for frame in &frames[..3] {
            socket.write_all(frame.as_bytes()).unwrap();
        }
        resume.recv_timeout(Duration::from_secs(5)).unwrap();
        for frame in &frames[3..] {
            socket.write_all(frame.as_bytes()).unwrap();
        }
    });
    let (host, mut updates) = Host::new().unwrap();
    let mut conversation = Conversation::default();
    let generation = conversation.begin("中文问题".into()).unwrap();
    let _cancel = host.chat_at(
        base,
        "synthetic".into(),
        "中文问题".into(),
        Some("sess-100".into()),
        generation,
    );
    while conversation.turn.answer_text.is_empty() {
        match next(&host, &mut updates) {
            Update::Event(g, e) => conversation.event(g, e),
            _ => panic!("stream ended before the first text"),
        }
    }
    assert_eq!(conversation.turn.status, TurnStatus::Streaming);
    release.send(()).unwrap();
    loop {
        match next(&host, &mut updates) {
            Update::Event(g, e) => conversation.event(g, e),
            Update::End(g, result) => {
                assert_eq!(g, generation);
                result.unwrap();
                break;
            }
            _ => panic!("unexpected update"),
        }
    }
    let expected = crate::reduce_fixture_json_lines(include_str!(
        "../../../frontend_rust/tests/fixtures/stream-normal-long.json"
    ))
    .unwrap();
    assert_eq!(conversation.turn.answer_text, expected.answer_text);
    assert_eq!(conversation.turn.status, TurnStatus::Done);
    server.join().unwrap();
}

#[test]
fn upstream_configuration_error_reaches_host_instead_of_hanging() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        request(&mut socket);
        let body = r#"{"message":"LLM client is not configured"}"#;
        write!(socket, "HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    });
    let (host, mut updates) = Host::new().unwrap();
    let _cancel = host.chat_at(base, "synthetic".into(), "ping".into(), None, 9);
    match next(&host, &mut updates) {
        Update::End(9, Err(error)) => assert!(error.contains("LLM client is not configured")),
        _ => panic!("configuration error was lost"),
    }
    server.join().unwrap();
}

#[test]
fn unavailable_local_api_reaches_host_as_error() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let (host, mut updates) = Host::new().unwrap();
    let _cancel = host.chat_at(base, "synthetic".into(), "ping".into(), None, 3);
    assert!(matches!(next(&host, &mut updates), Update::End(3, Err(_))));
}

#[test]
#[ignore = "requires isolated process environment; run scripts/accept-tauri-shared.ps1 -WithHttpFixture"]
fn local_session_and_history_over_http() {
    assert_eq!(std::env::var("GPUI_ACCEPTANCE_HTTP").as_deref(), Ok("1"));
    assert_eq!(
        desktop_core::product_api_base_url(),
        "http://127.0.0.1:18180"
    );
    let listener =
        TcpListener::bind("127.0.0.1:18180").expect("isolated fixture port must be free");
    let server = std::thread::spawn(move || {
        for _ in 0..5 {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut headers = Vec::new();
            let mut byte = [0];
            while !headers.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).unwrap();
                headers.push(byte[0]);
                assert!(headers.len() < 16384);
            }
            let headers = String::from_utf8(headers).unwrap();
            let path = headers.split_whitespace().nth(1).unwrap();
            let size = headers
                .lines()
                .find_map(|l| {
                    l.to_lowercase()
                        .strip_prefix("content-length:")
                        .map(|s| s.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            socket.read_exact(&mut vec![0; size]).unwrap();
            let body = match path {
                "/health" | "/api/auth/me" => serde_json::json!({"success":true}),
                "/api/auth/login" => serde_json::json!({"success":true,"data":{"token":"synthetic-local-session","user":{"id":"fixture-user","email":"local@context-os.client","full_name":"Fixture"}}}),
                "/api/v1/chat/sessions" => serde_json::json!({"sessions":[
                    {"id":"sess-900","owner_user_id":"fixture-user","scope_kind":"personal","model_role":"quick_chat","agent_type":"chat","created_at":"2026-09-09","updated_at":"2026-09-09"},
                    {"id":"sess-workspace","owner_user_id":"fixture-user","workspace_id":"workspace-fixture","scope_kind":"workspace","model_role":"agent","agent_type":"rag","created_at":"2026-09-09","updated_at":"2026-09-09"}
                ]}),
                "/api/v1/chat/sessions/sess-history/messages" => serde_json::json!({"messages":[
                    {"id":1,"session_id":"sess-history","role":"user","content":"历史问题","created_at":"2026-09-09"},
                    {"id":2,"session_id":"sess-history","role":"assistant","content":"这是历史助手的完整回答内容。","created_at":"2026-09-09"}
                ]}),
                _ => panic!("unexpected fixture path: {path}"),
            }.to_string();
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
    });
    let dir = std::env::temp_dir().join(format!("gpui-acceptance-{}", uuid::Uuid::new_v4()));
    let (host, mut updates) = Host::new().unwrap();
    host.login(dir.clone());
    let session = match next(&host, &mut updates) {
        Update::Login(Ok(session)) => session,
        _ => panic!("local session failed"),
    };
    assert!(session.ready);
    let token = session.token.unwrap();
    assert!(dir.join("local_session.json").is_file());
    host.sessions(token.clone());
    match next(&host, &mut updates) {
        Update::Sessions(Ok(sessions)) => {
            assert_eq!(
                sessions.len(),
                1,
                "workspace sessions leaked into personal chat"
            );
            assert_eq!(sessions[0].id, "sess-900");
        }
        _ => panic!("session listing failed"),
    }
    host.history(token, "sess-history".into(), 42);
    match next(&host, &mut updates) {
        Update::History(42, Ok(messages)) => {
            assert_eq!(messages.len(), 2);
            assert_eq!(messages[0].role, "user");
            assert_eq!(messages[1].role, "assistant");
            assert_eq!(messages[1].content, "这是历史助手的完整回答内容。");
        }
        _ => panic!("history loading failed"),
    }
    host.login(dir.clone());
    assert!(matches!(next(&host, &mut updates), Update::Login(Ok(s)) if s.ready));
    server.join().unwrap();
    drop(host);
    // Only the two files created by this test; no recursive profile deletion.
    std::fs::remove_file(dir.join("local_user.json")).unwrap();
    std::fs::remove_file(dir.join("local_session.json")).unwrap();
    std::fs::remove_dir(dir).unwrap();
}
