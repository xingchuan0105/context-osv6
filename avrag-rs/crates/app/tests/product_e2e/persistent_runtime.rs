//! Process-lifetime tokio runtime for mock HTTP servers.
//!
//! `#[tokio::test]` shuts down per-test runtimes; mocks spawned there die with the
//! test. Shared [`crate::fixtures::RagSharedFixture`] reuses one AppState, so mock
//! endpoints must outlive individual tests.

use std::sync::OnceLock;

static PERSISTENT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

pub(crate) fn persistent() -> &'static tokio::runtime::Runtime {
    PERSISTENT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("persistent e2e runtime")
    })
}

pub(crate) fn spawn_persistent<F>(future: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    persistent().spawn(future);
}

/// Bind a listener on the process-lifetime runtime (safe to reuse across `#[tokio::test]` cases).
pub(crate) async fn bind_persistent_listener() -> (tokio::net::TcpListener, String) {
    let (tx, rx) = tokio::sync::oneshot::channel();
    persistent().spawn(async move {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind persistent api listener");
        let base_url = format!(
            "http://{}",
            listener.local_addr().expect("persistent api local_addr")
        );
        let _ = tx.send((listener, base_url));
    });
    rx.await.expect("persistent api bind")
}
