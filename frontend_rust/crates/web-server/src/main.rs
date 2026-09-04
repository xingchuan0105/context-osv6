use axum::{Router, routing::get};
use leptos_axum::{LeptosRoutes, file_and_error_handler, generate_route_list};
use leptos_config::get_configuration;
use web_ui::{App, shell};

async fn healthz() -> &'static str {
    "ok"
}

#[tokio::main]
async fn main() {
    // 配置唯一源是 workspace Cargo.toml 的 [[workspace.metadata.leptos]]；
    // cargo-leptos 运行/测试时由 LEPTOS_* 环境变量覆盖（site_addr / site_root 等）。
    let conf = get_configuration(Some("Cargo.toml"))
        .or_else(|_| get_configuration(None))
        .expect("failed to load leptos configuration");
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(App);

    let app = Router::new()
        .route("/healthz", get(healthz))
        .leptos_routes(&leptos_options, routes, {
            let options = leptos_options.clone();
            move || shell(options.clone())
        })
        .fallback(file_and_error_handler(shell))
        .with_state(leptos_options);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {}: {}", addr, e));

    println!("Web server listening on http://{}", addr);
    axum::serve(listener, app.into_make_service())
        .await
        .expect("Server error");
}
