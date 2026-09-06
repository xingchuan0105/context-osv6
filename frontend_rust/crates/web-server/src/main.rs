mod static_files;

use axum::http::{HeaderValue, header};
use axum::{Router, routing::get};
use leptos_axum::{LeptosRoutes, file_and_error_handler, generate_route_list};
use leptos_config::get_configuration;
use std::path::Path;
use tower::ServiceBuilder;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use web_ui::{App, shell};

async fn healthz() -> &'static str {
    "ok"
}

/// E4.4 抓取协议静态文本路由（public/ 在部署产物中不落盘，走 app 层）。
async fn llms_txt() -> impl axum::response::IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        include_str!("../../../assets/site/llms.txt"),
    )
}

/// Baidu 站长文件验证。
async fn baidu_verify() -> impl axum::response::IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "c3054d0735912577ce4407383c2a7965",
    )
}

/// 第三方声明 Markdown 下载（licenses 页「下载 .md」入口）。
async fn third_party_notices_md() -> impl axum::response::IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
        include_str!("../../../assets/legal/third-party-notices.md"),
    )
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
    let site_root = Path::new(leptos_options.site_root.as_ref()).to_path_buf();
    static_files::precompress_site(&site_root);
    let routes = generate_route_list(App);

    let pkg = ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        ))
        .service(
            ServeDir::new(site_root.join("pkg"))
                .precompressed_gzip()
                .precompressed_br(),
        );

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/llms.txt", get(llms_txt))
        .route("/baidu_verify_codeva-THd6TRYMwv.html", get(baidu_verify))
        .route(
            "/legal/third-party-notices.md",
            get(third_party_notices_md),
        )
        .nest_service("/pkg", pkg)
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
