//! Loopback HTTP fixture for the real GPUI Host; never starts product services.
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub struct Fixture {
    pub healthy: Arc<AtomicBool>,
    pub hold_auth: Arc<AtomicBool>,
    pub finish_stream: Arc<AtomicBool>,
    pub requests: Arc<Mutex<Vec<(String, Value)>>>,
    stop: Arc<AtomicBool>,
    server: Option<JoinHandle<()>>,
}

impl Fixture {
    pub fn start() -> Self {
        assert_eq!(
            std::env::var("GPUI_ACCEPTANCE_UI").as_deref(),
            Ok("1"),
            "run scripts/accept-headless.ps1 to isolate API and user data"
        );
        let base = desktop_core::product_api_base_url();
        let address = base
            .strip_prefix("http://127.0.0.1:")
            .expect("loopback fixture only");
        let port: u16 = address.parse().unwrap();
        assert!(port > 1024 && ![18080, 18081, 18082].contains(&port));
        let listener = TcpListener::bind(("127.0.0.1", port)).expect("fixture port occupied");
        listener.set_nonblocking(true).unwrap();
        let root =
            std::path::PathBuf::from(std::env::var_os("CONTEXT_OS_DESKTOP_DATA_DIR").unwrap());
        assert!(
            root.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("gpui-ui-")
        );
        // Only synthetic credentials from the preceding serial test in this run.
        for name in ["local_user.json", "local_session.json"] {
            let path = root.join(name);
            if path.is_file() {
                std::fs::remove_file(path).unwrap();
            }
        }
        let healthy = Arc::new(AtomicBool::new(true));
        let hold_auth = Arc::new(AtomicBool::new(false));
        let finish_stream = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let flags = (
            healthy.clone(),
            hold_auth.clone(),
            finish_stream.clone(),
            stop.clone(),
        );
        let recorded = requests.clone();
        let server = thread::spawn(move || {
            let mut workers = Vec::new();
            while !flags.3.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((socket, _)) => {
                        let flags = flags.clone();
                        let recorded = recorded.clone();
                        workers.push(thread::spawn(move || serve(socket, flags, recorded)));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(e) => panic!("fixture accept: {e}"),
                }
            }
            for worker in workers {
                worker.join().unwrap();
            }
        });
        Self {
            healthy,
            hold_auth,
            finish_stream,
            requests,
            stop,
            server: Some(server),
        }
    }

    pub fn count(&self, prefix: &str) -> usize {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|(route, _)| route.starts_with(prefix))
            .count()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.hold_auth.store(false, Ordering::SeqCst);
        self.finish_stream.store(true, Ordering::SeqCst);
        if let Some(server) = self.server.take() {
            let result = server.join();
            if !thread::panicking() {
                result.unwrap();
            }
        }
    }
}

type Flags = (
    Arc<AtomicBool>,
    Arc<AtomicBool>,
    Arc<AtomicBool>,
    Arc<AtomicBool>,
);
fn serve(mut socket: TcpStream, flags: Flags, requests: Arc<Mutex<Vec<(String, Value)>>>) {
    // Winsock may inherit the listener's nonblocking mode on accepted sockets.
    socket.set_nonblocking(false).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut headers = Vec::new();
    let mut byte = [0];
    loop {
        match socket.read(&mut byte) {
            Ok(0) => return, // Health TCP probe or cancelled connection.
            Ok(_) => headers.push(byte[0]),
            Err(_) if flags.3.load(Ordering::SeqCst) => return,
            Err(e) => panic!("fixture request: {e}"),
        }
        if headers.ends_with(b"\r\n\r\n") {
            break;
        }
        assert!(headers.len() < 16384);
    }
    let headers = String::from_utf8(headers).unwrap();
    let size = headers
        .lines()
        .find_map(|line| {
            line.to_lowercase()
                .strip_prefix("content-length:")
                .map(|s| s.trim().parse::<usize>().unwrap())
        })
        .unwrap_or(0);
    let mut bytes = vec![0; size];
    socket.read_exact(&mut bytes).unwrap();
    let payload = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    let parts: Vec<_> = headers.lines().next().unwrap().split_whitespace().collect();
    let route = format!("{} {}", parts[0], parts[1]);
    requests
        .lock()
        .unwrap()
        .push((route.clone(), payload.clone()));
    if parts[1].starts_with("/api/v1/") {
        assert!(
            headers
                .to_lowercase()
                .contains("authorization: bearer synthetic-ui")
        );
    }
    if parts[1].starts_with("/api/auth/") {
        while flags.1.load(Ordering::SeqCst) && !flags.3.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(2));
        }
    }
    if route == "POST /api/v1/chat" {
        assert_eq!(payload["capabilities"], json!([]));
        assert_eq!(payload["agent_type"], "chat");
        let start = json!({"event":"start","request_id":"ui-r","session_id":"ui-history"});
        let first =
            json!({"event":"token","request_id":"ui-r","message_id":1,"content":"第一段中文"});
        let last =
            json!({"event":"token","request_id":"ui-r","message_id":1,"content":"，完成回答。"});
        let done = json!({"event":"done","request_id":"ui-r","session_id":"ui-history","message_id":1,"payload":{"answer":"第一段中文，完成回答。","answer_blocks":[],"session_id":"ui-history","agent_type":"chat","sources":[],"citations":[],"trace":{"mode":"chat"},"degrade_trace":[]}});
        let frame = |v: &Value| format!("event: {}\ndata: {v}\n\n", v["event"].as_str().unwrap());
        let first_frames = format!("{}{}", frame(&start), frame(&first));
        let last_frames = format!("{}{}", frame(&last), frame(&done));
        if write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{first_frames}", first_frames.len() + last_frames.len()).is_err() { return; }
        while !flags.2.load(Ordering::SeqCst) && !flags.3.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(2));
        }
        let _ = socket.write_all(last_frames.as_bytes()); // Cancellation may have closed transport.
        return;
    }
    let value = match route.as_str() {
        "GET /health" => json!({"success":flags.0.load(Ordering::SeqCst)}),
        "POST /api/auth/login" => json!({"success":true,"data":{"token":"synthetic-ui","user":{"id":"ui-user","email":"local@context-os.client","full_name":"UI Fixture"}}}),
        "GET /api/auth/me" => json!({"success":true}),
        "GET /api/v1/chat/sessions" => json!({"sessions":[
            {"id":"ui-history","title":"自动验收历史","owner_user_id":"ui-user","scope_kind":"personal","model_role":"quick_chat","agent_type":"chat","created_at":"2026-09-10","updated_at":"2026-09-10"},
            {"id":"workspace-history","title":"工作区历史不应出现","workspace_id":"workspace-ui","owner_user_id":"ui-user","scope_kind":"workspace","model_role":"agent","agent_type":"rag","created_at":"2026-09-10","updated_at":"2026-09-10"}
        ]}),
        "GET /api/v1/chat/sessions/ui-history/messages" => json!({"messages":[
            {"id":1,"session_id":"ui-history","role":"user","content":"历史问题","created_at":"2026-09-10"},
            {"id":2,"session_id":"ui-history","role":"assistant","content":"历史正文保留。","created_at":"2026-09-10"}
        ]}),
        _ => panic!("unexpected route {route}"),
    }.to_string();
    let _ = write!(
        socket,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{value}",
        value.len()
    );
}
