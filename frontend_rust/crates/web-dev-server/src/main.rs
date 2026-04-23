#![recursion_limit = "4096"]

use anyhow::Result;
use axum::{
    Router,
    body::Body,
    extract::{FromRef, OriginalUri, Request, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::any,
};
use futures_util::TryStreamExt;
use leptos::{
    config::{LeptosOptions, get_configuration},
    hydration::{AutoReload, HydrationScripts},
    prelude::*,
};
use leptos_axum::{LeptosRoutes, generate_route_list, site_pkg_dir_service_route_path};
use reqwest::Client;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info;

#[derive(Clone)]
struct DevState {
    api_origin: String,
    client: Client,
    leptos_options: LeptosOptions,
}

impl FromRef<DevState> for LeptosOptions {
    fn from_ref(state: &DevState) -> Self {
        state.leptos_options.clone()
    }
}

#[component]
fn DevShell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="zh-CN">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <link id="leptos" rel="stylesheet" href="/pkg/index.css"/>
                <link rel="stylesheet" href="/pkg/stylance.css"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options=options.clone() />
            </head>
            <body>
                <web_ui::App />
            </body>
        </html>
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = any_spawner::Executor::init_tokio();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "frontend_web_dev_server=info,tower_http=info".into()),
        )
        .init();

    let conf = get_configuration(Some("Cargo.toml"))?;
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let backend = std::env::var("AVRAG_BACKEND_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
        .trim_end_matches('/')
        .to_string();

    let state = DevState {
        api_origin: backend.clone(),
        client: Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()?,
        leptos_options: leptos_options.clone(),
    };
    let routes = generate_route_list(web_ui::App);

    let app = Router::new()
        .route("/api", any(proxy_api))
        .route("/api/{*path}", any(proxy_api))
        .route_service(
            &site_pkg_dir_service_route_path(&leptos_options),
            leptos_axum::site_pkg_dir_service(&leptos_options),
        )
        .leptos_routes(&state, routes, {
            let options = leptos_options.clone();
            move || view! { <DevShell options=options.clone() /> }
        })
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(addr).await?;
    info!(site_addr = %display_addr(addr), api_origin = %backend, "frontend dev server listening");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn proxy_api(
    State(state): State<DevState>,
    OriginalUri(uri): OriginalUri,
    request: Request,
) -> Response {
    let upstream_url = format!("{}{}", state.api_origin, uri);
    let (parts, body) = request.into_parts();

    let body_stream = body
        .into_data_stream()
        .map_err(|error| std::io::Error::other(format!("request body stream error: {error}")));

    let upstream = state
        .client
        .request(parts.method.clone(), upstream_url)
        .body(reqwest::Body::wrap_stream(body_stream));

    let upstream = copy_request_headers(parts.headers, upstream);

    match upstream.send().await {
        Ok(response) => map_upstream_response(response),
        Err(error) => {
            tracing::warn!(error = %error, "api proxy request failed");
            (
                StatusCode::BAD_GATEWAY,
                format!("frontend dev proxy could not reach backend: {error}"),
            )
                .into_response()
        }
    }
}

fn copy_request_headers(
    headers: HeaderMap,
    mut builder: reqwest::RequestBuilder,
) -> reqwest::RequestBuilder {
    for (name, value) in headers.iter() {
        if *name == header::HOST || *name == header::CONTENT_LENGTH {
            continue;
        }
        builder = builder.header(name, value);
    }
    builder
}

fn map_upstream_response(response: reqwest::Response) -> Response {
    let status = response.status();
    let headers = response.headers().clone();
    let stream = response
        .bytes_stream()
        .map_err(|error| std::io::Error::other(format!("response body stream error: {error}")));

    let mut builder = Response::builder().status(status);
    for (name, value) in headers.iter() {
        if *name == header::CONTENT_LENGTH || *name == header::TRANSFER_ENCODING {
            continue;
        }
        builder = builder.header(name, value);
    }

    builder
        .body(Body::from_stream(stream))
        .unwrap_or_else(|error| {
            tracing::warn!(error = %error, "api proxy response build failed");
            (
                StatusCode::BAD_GATEWAY,
                "frontend dev proxy failed to build upstream response",
            )
                .into_response()
        })
}

fn display_addr(addr: SocketAddr) -> String {
    format!("http://{}", addr)
}
