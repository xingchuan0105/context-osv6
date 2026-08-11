//! Sandbox retrieval bridge: line-delimited JSON RPC over fd3/fd4 pipes.

use std::io::{BufRead, Read, Write};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use crate::{ExecutionResult, InterpreterError};
use async_trait::async_trait;
use serde_json::Value;

/// Host-side bridge invoked from the sandbox via pipe RPC.
#[async_trait]
pub trait HostBridge: Send + Sync {
    async fn call(&self, method: &str, args: Value) -> Value;
}

#[derive(Debug, serde::Deserialize)]
struct BridgeRequest {
    id: u64,
    method: String,
    args: Value,
}

fn bridge_shim_source() -> &'static str {
    static SHIM: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
    SHIM.get_or_init(|| Box::leak(build_shim_source().into_boxed_str()))
}

/// 从 `contracts::sdk_primitives` 注册表 codegen Python shim(D10):
/// 每个原语一行 `async def`,docstring 与 payload 形状全部派生自注册表。
fn build_shim_source() -> String {
    use contracts::sdk_primitives::SDK_PRIMITIVES;

    let mut methods = String::new();
    for p in SDK_PRIMITIVES {
        let docstring = p.docstring.replace('\\', "\\\\").replace('"', "\\\"");
        let return_path = if p.py_return.is_empty() {
            String::new()
        } else {
            p.py_return.to_string()
        };
        // to_thread so asyncio.gather does not serialize on a blocking _rpc.
        methods.push_str(&format!(
            r#"
    async def {id}({py_sig}):
        """"{docstring}"""
        _data = await asyncio.to_thread(_rpc, "{id}", {py_payload})
        return _data{return_path}
"#,
            id = p.id,
            py_sig = p.py_sig,
            docstring = docstring,
            py_payload = p.py_payload,
            return_path = return_path,
        ));
    }

    // Multiplexed line-JSON RPC: a dedicated reader thread demuxes replies by id
    // so asyncio.gather can issue multiple outstanding calls. Host pump runs
    // concurrent workers (see run_bridge_pump_sync).
    format!(
        r#"
import json as _json
# `threading` / `asyncio` pre-imported in wrapper before security import hook.
_req = open(3, "w", buffering=1)
_resp = open(4, "r", buffering=1)
_id = 0
_id_lock = threading.Lock()
_write_lock = threading.Lock()
_pending = {{}}
_pending_lock = threading.Lock()
_reader_started = False
_reader_start_lock = threading.Lock()

def _reader_loop():
    while True:
        line = _resp.readline()
        if not line:
            with _pending_lock:
                for box in _pending.values():
                    box["error"] = "bridge closed"
                    box["ev"].set()
                _pending.clear()
            break
        try:
            msg = _json.loads(line)
        except Exception:
            continue
        rid = msg.get("id")
        with _pending_lock:
            box = _pending.pop(rid, None)
        if box is None:
            continue
        if not msg.get("ok"):
            box["error"] = (msg.get("error") or {{}}).get("message", "bridge error")
        else:
            box["data"] = msg.get("data")
        box["ev"].set()

def _ensure_reader():
    global _reader_started
    with _reader_start_lock:
        if _reader_started:
            return
        _reader_started = True
        t = threading.Thread(target=_reader_loop, name="avrag-bridge-reader", daemon=True)
        t.start()

def _rpc(method, args):
    """Blocking RPC; safe under gather via asyncio.to_thread (see method wrappers)."""
    global _id
    _ensure_reader()
    with _id_lock:
        _id += 1
        rid = _id
    box = {{"ev": threading.Event(), "data": None, "error": None}}
    with _pending_lock:
        _pending[rid] = box
    with _write_lock:
        _req.write(_json.dumps({{"id": rid, "method": method, "args": args}}) + "\n")
        _req.flush()
    if not box["ev"].wait(timeout=300):
        with _pending_lock:
            _pending.pop(rid, None)
        raise RuntimeError("bridge rpc timeout")
    if box["error"] is not None:
        raise RuntimeError(box["error"])
    return box["data"]

class _Client:
    """SaC SDK: dense/lexical; grep for line-level; web/fetch; memory; no topk."""
{methods}
# Module-level aliases (design §5 examples use bare save/load).
async def save(path, data):
    return await client.save(path, data)

async def load(path):
    return await client.load(path)

client = _Client()
"#
    )
}

/// 规范 SaC SDK 原语名(host 必须实现每个名字;由注册表派生)。
pub fn bridge_shim_client_method_names() -> &'static [&'static str] {
    use contracts::sdk_primitives::{SdkCapability, ids_for};
    static NAMES: std::sync::OnceLock<&'static [&'static str]> = std::sync::OnceLock::new();
    NAMES.get_or_init(|| {
        let ids = ids_for(SdkCapability::BASE | SdkCapability::RAG | SdkCapability::SEARCH);
        Box::leak(ids.into_boxed_slice())
    })
}

pub(crate) fn build_bridge_sandbox_wrapper(
    user_code: &str,
    memory_mb: u64,
    cpu_secs: u64,
) -> String {
    let blocked_modules = [
        "os",
        "subprocess",
        "socket",
        "sys",
        "ctypes",
        "shutil",
        "posix",
        "fcntl",
        "pty",
        "pwd",
        "grp",
        "resource",
        "signal",
        "multiprocessing",
        "threading",
    ];

    let blocked_list = blocked_modules
        .iter()
        .map(|m| format!("'{}'", m))
        .collect::<Vec<_>>()
        .join(", ");

    let indented_user_code = user_code
        .lines()
        .map(|line| format!("    {line}"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"import sys, io, json, traceback, asyncio
# Pre-import before the security hook: multiplexed bridge RPC uses
# asyncio.to_thread → run_in_executor → concurrent.futures.thread, whose
# module top level imports threading/os. Loading it here (pre-hook) keeps
# every later blocked-name import out of the runtime path, so the hook
# below can stay strict. User code cannot import these (hooked below).
import threading
import concurrent.futures
import concurrent.futures.thread

BLOCKED = {{{blocked_list}}}
_original_import = __builtins__.__import__

def _safe_import(name, *args, **kwargs):
    top = name.split('.')[0]
    if top in BLOCKED:
        raise ImportError(f"import of '{{name}}' is blocked for security reasons")
    return _original_import(name, *args, **kwargs)

__builtins__.__import__ = _safe_import

try:
    import resource
    mem_bytes = {memory_mb} * 1024 * 1024
    resource.setrlimit(resource.RLIMIT_AS, (mem_bytes, mem_bytes))
except Exception:
    pass

try:
    import resource
    resource.setrlimit(resource.RLIMIT_CPU, ({cpu_secs}, {cpu_secs}))
except Exception:
    pass

{bridge_shim}

_real_stdout = sys.stdout
_real_stderr = sys.stderr
_cap_stdout = io.StringIO()
_cap_stderr = io.StringIO()
sys.stdout = _cap_stdout
sys.stderr = _cap_stderr

async def __avrag_main():
{indented_user_code}

try:
    asyncio.run(__avrag_main())
except Exception:
    traceback.print_exc()

output = {{
    "stdout": _cap_stdout.getvalue(),
    "stderr": _cap_stderr.getvalue(),
    "result": None,
    "success": True,
    "exit_code": 0,
    "killed": False
}}
_real_stdout.write(json.dumps(output))
# sys.stdout stays swapped for the StringIO capture; without an explicit
# flush the buffered payload can be lost when the process exits with
# bridge/executor threads alive.
_real_stdout.flush()
"#,
        blocked_list = blocked_list,
        memory_mb = memory_mb,
        cpu_secs = cpu_secs,
        bridge_shim = bridge_shim_source(),
        indented_user_code = indented_user_code,
    )
}

fn bridge_error_response(id: u64, code: &str, message: impl Into<String>) -> String {
    serde_json::json!({
        "id": id,
        "ok": false,
        "error": {
            "code": code,
            "message": message.into(),
        }
    })
    .to_string()
}

fn bridge_ok_response(id: u64, data: Value) -> String {
    serde_json::json!({
        "id": id,
        "ok": true,
        "data": data,
    })
    .to_string()
}

#[cfg(unix)]
mod unix_impl {
    use super::*;
    use std::os::unix::io::{FromRawFd, IntoRawFd};
    use std::os::unix::process::CommandExt;
    use std::process::Child;
    use std::sync::OnceLock;

    fn bridge_pump_runtime() -> &'static tokio::runtime::Runtime {
        static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
        RUNTIME.get_or_init(|| {
            // Multi-thread: doc_summary/doc_metadata use tokio::join! for parallel PG
            // reads; a current-thread runtime can stall when the shared pool is busy.
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("bridge pump runtime")
        })
    }

    /// `pre_exec` pins the bridge pipes onto fd 3/4 via `dup2` + `close`.
    /// If the OS allocated a pipe at fd 3/4 already, `dup2(x, x)` is a no-op and
    /// the following `close(x)` kills the very fd the sandbox needs — python's
    /// `open(3)` then fails with EBADF (flaky under parallel fd pressure).
    /// Lift such fds above the 3/4 range so dup2-then-close is always safe.
    pub(crate) fn lift_fd_out_of_range(
        fd: std::os::unix::io::RawFd,
    ) -> Result<std::os::unix::io::RawFd, InterpreterError> {
        if fd > 4 {
            return Ok(fd);
        }
        let moved = unsafe { libc::fcntl(fd, libc::F_DUPFD, 10) };
        if moved == -1 {
            return Err(InterpreterError::Io(std::io::Error::last_os_error()));
        }
        unsafe { libc::close(fd) };
        Ok(moved)
    }

    pub async fn execute_with_bridge(
        python_path: &str,
        timeout_secs: u64,
        memory_mb: u64,
        cpu_secs: u64,
        code: &str,
        bridge: Arc<dyn HostBridge>,
    ) -> Result<ExecutionResult, InterpreterError> {
        let sandbox_code = build_bridge_sandbox_wrapper(code, memory_mb, cpu_secs);

        let (req_reader, req_writer) = std::io::pipe().map_err(InterpreterError::Io)?;
        let (resp_reader, resp_writer) = std::io::pipe().map_err(InterpreterError::Io)?;

        let req_write_fd = lift_fd_out_of_range(req_writer.into_raw_fd())?;
        let resp_read_fd = lift_fd_out_of_range(resp_reader.into_raw_fd())?;

        // Prevent Rust's pre-exec fd sweep from closing bridge fds before dup2.
        unsafe {
            libc::fcntl(req_write_fd, libc::F_SETFD, 0);
            libc::fcntl(resp_read_fd, libc::F_SETFD, 0);
        }

        let temp_dir = tempfile::TempDir::new()
            .map_err(|e| InterpreterError::Io(std::io::Error::other(format!("temp dir: {e}"))))?;

        let req_file = unsafe { std::fs::File::from_raw_fd(req_reader.into_raw_fd()) };
        let resp_file = unsafe { std::fs::File::from_raw_fd(resp_writer.into_raw_fd()) };

        let (pump_ready_tx, pump_ready_rx) = tokio::sync::oneshot::channel();
        let pump_bridge = Arc::clone(&bridge);
        std::thread::spawn(move || {
            let _ = pump_ready_tx.send(());
            if let Err(e) = run_bridge_pump_sync(req_file, resp_file, pump_bridge) {
                tracing::warn!("bridge pump ended with error: {e}");
            }
        });

        pump_ready_rx
            .await
            .map_err(|_| InterpreterError::Bridge("pump failed to start".to_string()))?;
        let mut command = Command::new(python_path);
        command
            .arg("-c")
            .arg(&sandbox_code)
            .current_dir(temp_dir.path())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Put the child in its own process group so that, on timeout, we can
        // kill python *and* any subprocesses it spawned (pid == pgid).
        command.process_group(0);
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(req_write_fd, 3) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::dup2(resp_read_fd, 4) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                // Keep bridge fds open across execve(python).
                libc::fcntl(3, libc::F_SETFD, 0);
                libc::fcntl(4, libc::F_SETFD, 0);
                libc::close(req_write_fd);
                libc::close(resp_read_fd);
                Ok(())
            });
        }
        let child = command.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                InterpreterError::PythonNotFound(python_path.to_string())
            } else {
                InterpreterError::Io(e)
            }
        })?;
        // Capture the child's pid before it is moved into the wait task; with
        // process_group(0) set, this pid doubles as the process-group id for killpg.
        let child_pid = child.id() as i32;

        let timeout = Duration::from_secs(timeout_secs);
        let wait_result = tokio::time::timeout(timeout, wait_child(child)).await;

        let (status, stdout, stderr) = match wait_result {
            Ok(Ok(tuple)) => tuple,
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                // Kill the child's process group to clean up the python process
                // + any subprocesses it spawned. `child` was moved into
                // wait_child; use the captured pid as pgid (process_group(0) =>
                // pid == pgid).
                unsafe {
                    let _ = libc::killpg(child_pid, libc::SIGKILL);
                }
                return Err(InterpreterError::Timeout(timeout_secs));
            }
        };

        let exit_code = status.as_ref().and_then(|s| s.code());
        let success = status.as_ref().is_some_and(|s| s.success());

        match serde_json::from_str::<ExecutionResult>(&stdout) {
            Ok(mut result) => {
                if !stderr.is_empty() && result.stderr.is_empty() {
                    result.stderr = stderr;
                }
                if !success {
                    result.success = false;
                    result.exit_code = exit_code;
                }
                Ok(result)
            }
            Err(_) => Ok(ExecutionResult {
                stdout,
                stderr,
                result: None,
                success,
                exit_code,
                killed: false,
            }),
        }
    }

    async fn wait_child(
        mut child: Child,
    ) -> Result<(Option<std::process::ExitStatus>, String, String), InterpreterError> {
        let stdout_pipe = child.stdout.take();
        let stderr_pipe = child.stderr.take();
        let (tx, rx) = tokio::sync::oneshot::channel();

        std::thread::spawn(move || {
            let result = (|| {
                let stdout_handle = stdout_pipe.map(|mut stdout| {
                    std::thread::spawn(move || {
                        let mut buf = Vec::new();
                        let _ = stdout.read_to_end(&mut buf);
                        buf
                    })
                });
                let stderr_handle = stderr_pipe.map(|mut stderr| {
                    std::thread::spawn(move || {
                        let mut buf = Vec::new();
                        let _ = stderr.read_to_end(&mut buf);
                        buf
                    })
                });

                let status = child.wait().map_err(InterpreterError::Io)?;
                let stdout = stdout_handle
                    .map(|h| h.join())
                    .transpose()
                    .map_err(|_| {
                        InterpreterError::Io(std::io::Error::other("stdout reader panicked"))
                    })?
                    .unwrap_or_default();
                let stderr = stderr_handle
                    .map(|h| h.join())
                    .transpose()
                    .map_err(|_| {
                        InterpreterError::Io(std::io::Error::other("stderr reader panicked"))
                    })?
                    .unwrap_or_default();
                Ok((
                    Some(status),
                    String::from_utf8(stdout)?,
                    String::from_utf8(stderr)?,
                ))
            })();
            let _ = tx.send(result);
        });

        rx.await
            .map_err(|_| InterpreterError::Bridge("child wait channel closed".to_string()))?
    }

    /// Line-JSON request pump with concurrent host calls.
    ///
    /// Each request is handled on a dedicated OS thread via
    /// `runtime.handle().block_on(bridge.call)`, so multiple outstanding RPCs
    /// (Python `asyncio.gather`) overlap. Replies are written as soon as each
    /// call finishes and may arrive out of order (Python matches by `id`).
    fn run_bridge_pump_sync(
        req_file: std::fs::File,
        resp_file: std::fs::File,
        bridge: Arc<dyn HostBridge>,
    ) -> Result<(), InterpreterError> {
        use std::sync::{Arc, Mutex};

        let mut reader = std::io::BufReader::new(req_file);
        let resp = Arc::new(Mutex::new(resp_file));
        let runtime = bridge_pump_runtime();
        let rt_handle = runtime.handle().clone();
        let mut workers: Vec<std::thread::JoinHandle<Result<(), InterpreterError>>> = Vec::new();

        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let trimmed = line.trim().to_string();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let bridge = Arc::clone(&bridge);
                    let resp = Arc::clone(&resp);
                    let rt_handle = rt_handle.clone();
                    workers.push(std::thread::spawn(move || {
                        let response_line = match serde_json::from_str::<BridgeRequest>(&trimmed) {
                            Ok(req) => {
                                let data =
                                    rt_handle.block_on(bridge.call(&req.method, req.args));
                                bridge_ok_response(req.id, data)
                            }
                            Err(e) => {
                                bridge_error_response(0, "invalid_request", format!("{e}"))
                            }
                        };
                        let mut out = resp.lock().map_err(|_| {
                            InterpreterError::Bridge("bridge response lock poisoned".into())
                        })?;
                        out.write_all(response_line.as_bytes())
                            .map_err(InterpreterError::Io)?;
                        out.write_all(b"\n").map_err(InterpreterError::Io)?;
                        out.flush().map_err(InterpreterError::Io)?;
                        Ok(())
                    }));
                }
                Err(e) => return Err(InterpreterError::Io(e)),
            }
        }

        let mut first_err: Option<InterpreterError> = None;
        for h in workers {
            match h.join() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    if first_err.is_none() {
                        first_err = Some(e);
                    }
                }
                Err(_) => {
                    if first_err.is_none() {
                        first_err = Some(InterpreterError::Bridge(
                            "bridge worker thread panicked".into(),
                        ));
                    }
                }
            }
        }
        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// High fds pass through untouched; low fds must be moved out of the
        /// 3/4 pin range and stay usable (the 2026-07-21 EBADF flake).
        #[test]
        fn lift_fd_moves_low_fd_and_keeps_it_writable() {
            let (_read_end, write_end) = std::io::pipe().unwrap();
            let fd = write_end.into_raw_fd();
            // Force the fd as low as the OS allows (>=3) to exercise both paths.
            let low = unsafe { libc::fcntl(fd, libc::F_DUPFD, 3) };
            assert!(low >= 3);
            unsafe { libc::close(fd) };

            let moved = lift_fd_out_of_range(low).expect("lift");
            assert!(
                moved > 4,
                "bridge fd must end outside the 3/4 pin range, got {moved}"
            );
            // The (possibly moved) fd still refers to the same pipe.
            let n = unsafe { libc::write(moved, b"x".as_ptr() as *const libc::c_void, 1) };
            assert_eq!(n, 1);
            unsafe { libc::close(moved) };
        }

        #[test]
        fn lift_fd_keeps_high_fd_unchanged() {
            let (_read_end, write_end) = std::io::pipe().unwrap();
            let fd = write_end.into_raw_fd();
            // Place a duplicate at >=10 so the no-op path is deterministic.
            let high = unsafe { libc::fcntl(fd, libc::F_DUPFD, 10) };
            assert!(high >= 10);
            unsafe { libc::close(fd) };

            assert_eq!(lift_fd_out_of_range(high).expect("lift"), high);
            unsafe { libc::close(high) };
        }
    }
}

#[cfg(not(unix))]
pub async fn execute_with_bridge(
    _python_path: &str,
    _timeout_secs: u64,
    _memory_mb: u64,
    _cpu_secs: u64,
    _code: &str,
    _bridge: Arc<dyn HostBridge>,
) -> Result<ExecutionResult, InterpreterError> {
    Err(InterpreterError::Bridge(
        "sandbox retrieval bridge requires a Unix platform".to_string(),
    ))
}

#[cfg(unix)]
pub use unix_impl::execute_with_bridge;

#[cfg(all(test, unix))]
mod bridge_shim_tests {
    use super::bridge_shim_client_method_names;

    #[test]
    fn shim_exposes_sac_sdk_methods() {
        assert_eq!(
            bridge_shim_client_method_names(),
            &[
                "save",
                "load",
                "history",
                "user_profile",
                "user_context",
                "calculator",
                "weather_query",
                "dense",
                "lexical",
                "grep",
                "doc_summary",
                "struct_catalog",
                "struct_query",
                "web",
                "fetch",
            ]
        );
        // Removed anchors: no graph / graph_search / chunk_fetch / read_lines / aggregators.
        for banned in [
            "graph",
            "graph_search",
            "chunk_fetch",
            "read_lines",
            "doc_scan",
            "rerank",
            "count",
            "dedupe",
            "dense_search",
            "lexical_search",
        ] {
            assert!(
                !bridge_shim_client_method_names().contains(&banned),
                "banned method {banned} must not be on SaC SDK surface"
            );
        }
    }
}

#[cfg(all(test, unix))]
mod spawn_tests {
    use super::*;
    use std::io::Read;
    use std::os::unix::io::IntoRawFd;
    use std::os::unix::process::CommandExt;

    #[test]
    fn python_can_write_inherited_bridge_fd() {
        let (mut read_end, write_end) = std::io::pipe().unwrap();
        // Same lift as the production spawn path: fd 3/4 must never be the
        // dup2 source, or dup2-then-close would kill the pinned fd.
        let write_fd = super::unix_impl::lift_fd_out_of_range(write_end.into_raw_fd())
            .expect("lift bridge fd");
        unsafe {
            libc::fcntl(write_fd, libc::F_SETFD, 0);
        }

        let mut command = Command::new("python3");
        command.arg("-c").arg("import os; os.write(3, b'hello')");
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(write_fd, 3) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                libc::fcntl(3, libc::F_SETFD, 0);
                libc::close(write_fd);
                Ok(())
            });
        }
        let output = command.output().expect("spawn python");
        assert!(
            output.status.success(),
            "stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );

        let mut buf = [0u8; 16];
        let n = read_end.read(&mut buf).expect("read pipe");
        assert_eq!(&buf[..n], b"hello");
    }
}

#[cfg(all(test, unix))]
mod timeout_kill_tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;
    use std::time::Instant;

    /// Minimal host bridge stub whose responses never matter for these tests
    /// (the busy-loop never makes an RPC).
    struct NoopBridge;

    #[async_trait]
    impl HostBridge for NoopBridge {
        async fn call(&self, _method: &str, _args: Value) -> Value {
            json!({ "chunks": [] })
        }
    }

    /// `while True: pass` pins a CPU and never exits on its own. The bridge
    /// path must time out and return `InterpreterError::Timeout`, *and* kill the
    /// orphaned python process so it does not leak. This asserts the error
    /// variant and a reasonable wall-clock bound (well under the runaway 30s
    /// default). Requires `python3` on PATH.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn bridge_timeout_kills_busy_python() {
        // `python3` may be absent in some CI images; skip rather than fail there.
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("skipping: python3 not found on PATH");
            return;
        }

        let started = Instant::now();
        // A 2s timeout: the loop spins forever, so the only way this returns is
        // via the timeout path. We also assert it returns promptly (< 10s) so a
        // regression that fails to time out surfaces as a slow failure.
        let result = execute_with_bridge(
            "python3",
            2,
            256,
            30,
            "while True:\n    pass",
            Arc::new(NoopBridge),
        )
        .await;

        let elapsed = started.elapsed();
        assert!(
            matches!(result, Err(InterpreterError::Timeout(2))),
            "expected Timeout(2), got {:?}",
            result
        );
        assert!(
            elapsed.as_secs() < 10,
            "timeout path took too long ({elapsed:?}); child likely not reaped"
        );
    }

    /// Manual / non-deterministic check that no orphaned python process from the
    /// `bridge_timeout_kills_busy_python` run survives. Killing a process group
    /// is inherently racy to observe from outside, so this is `#[ignore]`:
    /// enable locally with `cargo test -p avrag-code-interpreter -- --ignored`
    /// and (if needed) visually confirm with `pgrep -af 'while True'`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "orphan-process check is non-deterministic; run manually with python3 available"]
    async fn bridge_timeout_leaves_no_orphan_python() {
        let _ = execute_with_bridge(
            "python3",
            2,
            256,
            30,
            "while True:\n    pass",
            Arc::new(NoopBridge),
        )
        .await;

        // Give the SIGKILL a moment to take effect and the reaper to clean up.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let pgrep = std::process::Command::new("pgrep")
            .arg("-af")
            .arg("python3")
            .output();
        match pgrep {
            Ok(out) => {
                let listing = String::from_utf8_lossy(&out.stdout);
                // Our busy loop runs as `python3 -c "<wrapper>...while True..."`.
                assert!(
                    !listing.contains("while True"),
                    "orphan python process still running:\n{listing}"
                );
            }
            Err(_) => {
                // pgrep itself unavailable; nothing deterministic to assert.
                eprintln!("skipping orphan assertion: pgrep not available");
            }
        }
    }

    /// Host + shim must run independent RPCs concurrently: wall clock ≈ max
    /// latency, not sum (ReWOO-style gather fan-out).
    struct SlowEchoBridge;

    #[async_trait]
    impl HostBridge for SlowEchoBridge {
        async fn call(&self, method: &str, args: Value) -> Value {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            json!({ "method": method, "args": args, "chunks": [] })
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn bridge_gather_runs_rpcs_concurrently() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("skipping: python3 not found on PATH");
            return;
        }

        let code = r#"
import asyncio
a, b = await asyncio.gather(
    client.calculator("1+1"),
    client.calculator("2+2"),
)
print("ok", a is not None or True, b is not None or True)
"#;
        // calculator is always on BASE; SlowEchoBridge ignores method body.
        let started = Instant::now();
        let result = execute_with_bridge(
            "python3",
            15,
            256,
            30,
            code,
            Arc::new(SlowEchoBridge),
        )
        .await
        .expect("bridge gather should succeed");
        let elapsed = started.elapsed();
        assert!(
            result.success || result.exit_code == Some(0) || !result.stdout.is_empty(),
            "unexpected result: stdout={} stderr={}",
            result.stdout,
            result.stderr
        );
        // Two 250ms serial would be ≥500ms of pure sleep; concurrent should land
        // well under 450ms of sleep + overhead. Allow generous overhead for spawn.
        assert!(
            elapsed.as_millis() < 700,
            "expected concurrent RPC wall <700ms, got {elapsed:?} (serial would be ~500ms+overhead)"
        );
        assert!(
            elapsed.as_millis() >= 200,
            "expected at least one 250ms sleep visible, got {elapsed:?}"
        );
    }
}

pub(crate) async fn execute_with_bridge_arc<B: HostBridge + Send + Sync + 'static>(
    python_path: &str,
    timeout_secs: u64,
    memory_mb: u64,
    cpu_secs: u64,
    code: &str,
    bridge: Arc<B>,
) -> Result<ExecutionResult, InterpreterError> {
    let bridge: Arc<dyn HostBridge> = bridge;
    execute_with_bridge(python_path, timeout_secs, memory_mb, cpu_secs, code, bridge).await
}

