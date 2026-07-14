//! Local data-plane health probes for desktop (Postgres / Redis / Milvus).

use serde::Serialize;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct LocalStackServiceStatus {
    pub id: String,
    pub label: String,
    pub endpoint: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalStackStatus {
    pub overall_ok: bool,
    pub services: Vec<LocalStackServiceStatus>,
    pub compose_hint: String,
}

fn probe_tcp(host: &str, port: u16) -> (bool, String) {
    let endpoint = format!("{host}:{port}");
    let addrs = match endpoint.to_socket_addrs() {
        Ok(a) => a.collect::<Vec<SocketAddr>>(),
        Err(e) => return (false, format!("resolve failed: {e}")),
    };
    if addrs.is_empty() {
        return (false, "no addresses".into());
    }
    match TcpStream::connect_timeout(&addrs[0], Duration::from_millis(400)) {
        Ok(_) => (true, "port open".into()),
        Err(e) => (false, e.to_string()),
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

#[tauri::command]
pub fn get_local_stack_status() -> LocalStackStatus {
    // Non-default ports avoid clashing with system PG/Redis on the same machine.
    let pg_host = env_or("CLIENT_PG_HOST", "127.0.0.1");
    let pg_port: u16 = env_or("CLIENT_PG_PORT", "5433").parse().unwrap_or(5433);
    let redis_host = env_or("CLIENT_REDIS_HOST", "127.0.0.1");
    let redis_port: u16 = env_or("CLIENT_REDIS_PORT", "6380").parse().unwrap_or(6380);
    let milvus_host = env_or("CLIENT_MILVUS_HOST", "127.0.0.1");
    let milvus_port: u16 = env_or("CLIENT_MILVUS_PORT", "19530").parse().unwrap_or(19530);

    let mut services = Vec::new();

    let (pg_ok, pg_detail) = probe_tcp(&pg_host, pg_port);
    services.push(LocalStackServiceStatus {
        id: "postgres".into(),
        label: "PostgreSQL".into(),
        endpoint: format!("{pg_host}:{pg_port}"),
        ok: pg_ok,
        detail: pg_detail,
    });

    let (redis_ok, redis_detail) = probe_tcp(&redis_host, redis_port);
    services.push(LocalStackServiceStatus {
        id: "redis".into(),
        label: "Redis".into(),
        endpoint: format!("{redis_host}:{redis_port}"),
        ok: redis_ok,
        detail: redis_detail,
    });

    let (milvus_ok, milvus_detail) = probe_tcp(&milvus_host, milvus_port);
    services.push(LocalStackServiceStatus {
        id: "milvus".into(),
        label: "Milvus".into(),
        endpoint: format!("{milvus_host}:{milvus_port}"),
        ok: milvus_ok,
        detail: milvus_detail,
    });

    let overall_ok = services.iter().all(|s| s.ok);
    LocalStackStatus {
        overall_ok,
        services,
        compose_hint: "bash scripts/desktop-local-stack.sh up".into(),
    }
}
