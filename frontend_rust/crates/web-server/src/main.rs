use axum::{Router, response::Html, routing::get};
use std::env;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use tower_http::services::ServeDir;

fn resolve_asset_dir(sub: &str) -> PathBuf {
    if let Ok(root) = env::var("ASSET_ROOT") {
        return PathBuf::from(root).join(sub);
    }

    for root in [Path::new("."), Path::new("frontend_rust")] {
        let candidate = root.join(sub);
        if candidate.exists() {
            return candidate;
        }
    }

    PathBuf::from(sub)
}

async fn chat_shell() -> Html<&'static str> {
    Html(
        "<!DOCTYPE html><html><head><title>Context-OS Chat (Phase 0 PoC)</title><link rel=\"stylesheet\" href=\"/style/design-tokens.css\"></head><body><div id=\"app\">Context-OS Chat PoC</div></body></html>",
    )
}

pub fn create_app() -> Router {
    let style_path = resolve_asset_dir("style");
    let assets_path = resolve_asset_dir("assets");

    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        // Phase 0 /chat 最小验证壳
        .route("/chat", get(chat_shell))
        .route("/chat/{session_id}", get(chat_shell))
        // 静态资源分发
        .nest_service("/style", ServeDir::new(style_path))
        .nest_service("/assets", ServeDir::new(assets_path))
}

#[tokio::main]
async fn main() {
    let app = create_app();

    let port = env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3001);

    let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| format!("127.0.0.1:{}", port));
    let addr: SocketAddr = bind_addr
        .parse()
        .unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], port)));

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {}: {}", addr, e));

    println!("Web server listening on {}", addr);
    axum::serve(listener, app).await.expect("Server error");
}
