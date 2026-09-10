//! Real portable PG/Redis + product sidecars. Opt-in, isolated by accept-managed.ps1.
use desktop_gpui::{
    runtime::{Host, Update},
    services::Phase,
};
use futures::{StreamExt, channel::mpsc::UnboundedReceiver};
use std::{
    fs,
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn next(events: &mut UnboundedReceiver<Update>) -> Update {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            tokio::time::timeout(Duration::from_secs(210), events.next())
                .await
                .unwrap()
                .unwrap()
        })
}

fn login(
    events: &mut UnboundedReceiver<Update>,
) -> Result<desktop_core::LocalSessionStatus, String> {
    loop {
        if let Update::Login(result) = next(events) {
            return result;
        }
    }
}

fn port_open(port: u16) -> bool {
    TcpStream::connect_timeout(
        &SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(200),
    )
    .is_ok()
}

fn ports() -> [u16; 3] {
    ["CLIENT_API_PORT", "CLIENT_PG_PORT", "CLIENT_REDIS_PORT"]
        .map(|k| std::env::var(k).unwrap().parse().unwrap())
}

fn assert_stopped(root: &Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while ports().into_iter().any(port_open) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        ports().into_iter().all(|p| !port_open(p)),
        "isolated service port remains open"
    );
    for file in [
        "api.pid",
        "worker.pid",
        "postgres-native.pid",
        "redis-native.pid",
    ] {
        let path = root.join("run").join(file);
        assert!(!path.exists(), "process record remains: {}", path.display());
    }
}

fn stop(host: &Host, events: &mut UnboundedReceiver<Update>, root: &Path) {
    host.stop_services();
    loop {
        if let Update::ServicesStopped(result) = next(events) {
            result.unwrap();
            break;
        }
    }
    assert_stopped(root);
}

fn record(root: &Path, steps: &mut Vec<&str>, step: &'static str) {
    steps.push(step);
    fs::write(
        root.join("steps.json"),
        serde_json::to_vec_pretty(&steps).unwrap(),
    )
    .unwrap();
    println!("PASS {step}");
}

#[test]
#[ignore = "requires fresh isolated data plane; run accept-managed.ps1 -DataPlaneOnly"]
fn native_data_plane_cold_start_and_shutdown() {
    use desktop_core::runtime_lease::{ProcessSnapshot, RuntimeLease};
    struct Cleanup(RuntimeLease);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            if let Err(error) = self.0.shutdown() {
                eprintln!("isolated data-plane cleanup: {error}");
            }
        }
    }
    assert_eq!(std::env::var("GPUI_ACCEPTANCE_MANAGED").as_deref(), Ok("1"));
    let root = PathBuf::from(std::env::var_os("CONTEXT_OS_CLIENT_HOME").unwrap());
    assert!(root.join("acceptance.marker").is_file());
    assert!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("gpui-managed-")
    );
    assert!(!root.join("data").exists());
    assert!(ports().into_iter().all(|p| !port_open(p)));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut steps = Vec::new();
    let mut cleanup = Cleanup(RuntimeLease::default());
    let mut ensure = || {
        let before = ProcessSnapshot::capture();
        let result = runtime.block_on(desktop_core::ensure_local_stack(None, None));
        cleanup.0.record_started_since(&before);
        let report = result.unwrap();
        assert!(report.ok, "{}\n{}", report.message, report.stdout);
    };
    ensure();
    let pg_port = ports()[1];
    let env = fs::read_to_string(root.join("client.env")).unwrap();
    assert!(env.contains(&format!("CLIENT_PG_PORT={pg_port}")));
    assert!(env.contains(&format!("CLIENT_REDIS_PORT={}", ports()[2])));
    assert!(env.contains(&format!("AVRAG_API_ADDR=127.0.0.1:{}", ports()[0])));
    assert_eq!(
        desktop_core::product_api_base_url(),
        format!("http://127.0.0.1:{}", ports()[0])
    );
    let config = desktop_core::get_client_runtime_config();
    assert_eq!(
        config.env_file_path.as_deref(),
        root.join("client.env").to_str()
    );
    assert!(config.env_file_exists);
    assert_eq!(
        config.migrations_dir.as_deref(),
        root.join("migrations").to_str()
    );
    assert!(config.database_url.contains("avrag_runtime"));
    assert_eq!(
        desktop_core::local_stack::get_local_stack_status().env_file_path,
        config.env_file_path
    );
    record(&root, &mut steps, "native_cold_init_and_port_configuration");
    let sql = |statement: &str| {
        let mut command = std::process::Command::new(
            PathBuf::from(std::env::var_os("PG_BIN_DIR").unwrap()).join("psql.exe"),
        );
        command.args([
            "-h",
            "127.0.0.1",
            "-p",
            &pg_port.to_string(),
            "-U",
            "avrag",
            "-d",
            "avrag_client",
            "-v",
            "ON_ERROR_STOP=1",
            "-tAc",
            statement,
        ]);
        desktop_core::win_cmd::hide_console(&mut command);
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    };
    assert_eq!(
        sql("SELECT extname FROM pg_extension WHERE extname='vector'"),
        "vector"
    );
    assert_eq!(
        sql("SELECT rolsuper OR rolbypassrls FROM pg_roles WHERE rolname='avrag_runtime'"),
        "f"
    );
    sql(
        "CREATE TABLE gpui_acceptance_probe (value integer); INSERT INTO gpui_acceptance_probe VALUES (37)",
    );
    record(&root, &mut steps, "real_pgvector_roles_and_database_write");
    cleanup.0.shutdown().unwrap();
    assert_stopped(&root);
    record(&root, &mut steps, "data_services_exit_and_pid_cleanup");
    let before = ProcessSnapshot::capture();
    let result = runtime.block_on(desktop_core::ensure_local_stack(None, None));
    cleanup.0.record_started_since(&before);
    assert!(result.unwrap().ok);
    assert_eq!(sql("SELECT value FROM gpui_acceptance_probe"), "37");
    cleanup.0.shutdown().unwrap();
    assert_stopped(&root);
    record(&root, &mut steps, "restart_preserves_real_database_data");
}

#[test]
#[ignore = "requires fresh isolated process environment; run accept-managed.ps1"]
fn managed_cold_start_failure_recovery_and_shutdown() {
    assert_eq!(std::env::var("GPUI_ACCEPTANCE_MANAGED").as_deref(), Ok("1"));
    assert!(std::env::var_os("CLIENT_API_BASE_URL").is_none());
    assert!(std::env::var_os("AVRAG_PUBLIC_BASE_URL").is_none());
    let root = PathBuf::from(std::env::var_os("CONTEXT_OS_CLIENT_HOME").unwrap());
    assert!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("gpui-managed-")
    );
    assert!(root.join("acceptance.marker").is_file());
    assert!(
        !root.join("data").exists(),
        "cold start requires a new data directory"
    );
    assert!(
        ports().into_iter().all(|p| !port_open(p)),
        "selected port already occupied"
    );
    let app_data = root.join("app-data");
    let migration = root.join("migrations/999999_gpui_failure.sql");
    let mut steps = Vec::new();

    fs::write(&migration, "SELECT gpui_deliberate_missing_function();\n").unwrap();
    {
        let (host, mut events) = Host::new().unwrap();
        host.login(app_data.clone());
        let error = login(&mut events).unwrap_err();
        assert!(error.contains("schema migration failed"), "{error}");
        assert!(root.join("data/pg-native/PG_VERSION").is_file());
        assert!(port_open(ports()[1]) && port_open(ports()[2]));
        assert!(!port_open(ports()[0]));
        assert!(!root.join("run/worker.pid").exists());
        record(
            &root,
            &mut steps,
            "cold_init_and_migration_failure_blocks_product",
        );
        stop(&host, &mut events, &root);
        record(&root, &mut steps, "partial_start_cleanup");
    }
    fs::remove_file(&migration).unwrap();

    let owner;
    let keys;
    {
        let (host, mut events) = Host::new().unwrap();
        let (observer, mut observed) = Host::new().unwrap();
        host.login(app_data.clone());
        loop {
            if matches!(
                next(&mut events),
                Update::ServicePhase(Phase::StartingStack)
            ) {
                break;
            }
        }
        observer.login(app_data.clone());
        let session = login(&mut events).unwrap();
        assert!(session.ready);
        owner = session.user.unwrap().id;
        assert!(ports().into_iter().all(port_open));
        assert!(root.join("run/worker.pid").is_file());
        let worker_deadline = Instant::now() + Duration::from_secs(15);
        while !fs::read_to_string(root.join("logs/worker.log"))
            .unwrap_or_default()
            .contains("worker heartbeat")
        {
            assert!(
                Instant::now() < worker_deadline,
                "worker did not complete initialization and heartbeat"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        keys =
            ["jwt.secret", "byok.key", "upload.signing"].map(|p| fs::read(root.join(p)).unwrap());
        record(
            &root,
            &mut steps,
            "repaired_migrations_api_worker_and_local_auth",
        );

        // Its request began during startup; the observer must not adopt the first host's lease.
        assert_eq!(login(&mut observed).unwrap().user.unwrap().id, owner);
        drop(observer);
        assert!(ports().into_iter().all(port_open));
        record(
            &root,
            &mut steps,
            "concurrent_observer_exit_preserves_existing_services",
        );
        stop(&host, &mut events, &root);
        record(&root, &mut steps, "owned_product_and_data_services_stop");
    }

    // A real migrator waits in SQL past the startup deadline. Close while it is pending.
    // No mock clocks: Host::drop must wait for cancellation/reaping and then release its lease.
    fs::write(&migration, "SELECT pg_sleep(120);\n").unwrap();
    {
        let (host, mut events) = Host::new().unwrap();
        host.login(app_data.clone());
        loop {
            if matches!(
                next(&mut events),
                Update::ServicePhase(Phase::StartingProduct)
            ) {
                break;
            }
        }
        let started = Instant::now();
        drop(host);
        assert!(
            started.elapsed() >= Duration::from_secs(80),
            "real timeout path was not reached"
        );
        assert!(
            started.elapsed() < Duration::from_secs(115),
            "startup did not finish within its bound"
        );
        let error = login(&mut events).unwrap_err();
        assert!(error.contains("timed out"), "{error}");
        assert_stopped(&root);
        record(
            &root,
            &mut steps,
            "close_during_real_migration_timeout_cleans_all_processes",
        );
    }
    fs::remove_file(&migration).unwrap();
    {
        let (host, mut events) = Host::new().unwrap();
        host.login(app_data);
        let restored = login(&mut events).unwrap();
        assert_eq!(restored.user.unwrap().id, owner);
        assert_eq!(
            ["jwt.secret", "byok.key", "upload.signing"].map(|p| fs::read(root.join(p)).unwrap()),
            keys
        );
        record(
            &root,
            &mut steps,
            "restart_after_timeout_preserves_account_and_keys",
        );
        drop(host);
        assert_stopped(&root);
        record(&root, &mut steps, "host_drop_releases_owned_processes");
    }
}
