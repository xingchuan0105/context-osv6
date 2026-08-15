//! Sandbox retrieval bridge: line-delimited JSON RPC over fd3/fd4 pipes.

use std::io::{BufRead, Read};
use std::sync::Arc;

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

fn bridge_shim_source(transport_setup: &str) -> String {
    // Built per execution: the windows TCP transport embeds a per-run
    // port/token, so no caching (unix fd setup is constant, but the build is
    // a cheap format! over ~20 registry entries).
    build_shim_source(transport_setup)
}

fn build_shim_source(transport_setup: &str) -> String {
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
    // concurrent workers (see run_bridge_pump_sync).
    format!(
        r#"
import json as _json
# `threading` / `asyncio` pre-imported in wrapper before security import hook.
{transport_setup}
_req = _bridge_transport["req"]
_resp = _bridge_transport["resp"]
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
"#,
        transport_setup = transport_setup,
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

pub(crate) struct SandboxOpts<'a> {
    pub(crate) user_code: &'a str,
    pub(crate) memory_mb: u64,
    pub(crate) cpu_secs: u64,
    /// Extra pre-hook imports (platform transport needs, e.g. socket on
    /// Windows). Injected before the security import hook so the hook stays
    /// strict for user code.
    pub(crate) prelude_imports: &'a str,
    /// Python snippet assigning `_bridge_transport = {{"req":…, "resp":…}}`
    /// (line-buffered text streams) for the shim RPC.
    pub(crate) transport_setup: &'a str,
}

pub(crate) fn build_bridge_sandbox_wrapper(opts: &SandboxOpts<'_>) -> String {
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
        .map(|m| format!("'{m}'"))
        .collect::<Vec<_>>()
        .join(", ");

    let indented_user_code = opts
        .user_code
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
{prelude_imports}

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
        memory_mb = opts.memory_mb,
        cpu_secs = opts.cpu_secs,
        prelude_imports = opts.prelude_imports,
        bridge_shim = bridge_shim_source(opts.transport_setup),
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

/// Platform-neutral bridge plumbing: line-JSON pump, child wait, and output
/// parsing shared by the fd (unix) and TCP (windows) transports.
#[cfg(any(unix, windows))]
mod shared {
    use super::*;
    use std::process::Child;
    use std::sync::{Arc, Mutex, OnceLock};

    pub(super) fn bridge_pump_runtime() -> &'static tokio::runtime::Runtime {
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

    pub(super) async fn wait_child(
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
    pub(super) fn run_bridge_pump_sync<R, W>(
        req_reader: R,
        resp_writer: W,
        bridge: Arc<dyn HostBridge>,
    ) -> Result<(), InterpreterError>
    where
        R: std::io::Read,
        W: std::io::Write + Send + 'static,
    {
        let mut reader = std::io::BufReader::new(req_reader);
        let resp = Arc::new(Mutex::new(resp_writer));
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

    pub(super) fn parse_child_output(
        status: Option<&std::process::ExitStatus>,
        stdout: String,
        stderr: String,
    ) -> Result<ExecutionResult, InterpreterError> {
        let exit_code = status.and_then(|s| s.code());
        let success = status.is_some_and(|s| s.success());

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
}

#[cfg(unix)]
mod unix_impl {
    use super::*;
    use super::shared::{parse_child_output, run_bridge_pump_sync, wait_child};
    use std::os::unix::io::{FromRawFd, IntoRawFd};
    use std::process::{Command, Stdio};
    use std::time::Duration;
    use std::os::unix::process::CommandExt;

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
        let sandbox_code = build_bridge_sandbox_wrapper(&SandboxOpts {
            user_code: code,
            memory_mb,
            cpu_secs,
            prelude_imports: "",
            // fd3/fd4 pinned by pre_exec below; line-buffered text mode.
            transport_setup: r#"
_bridge_transport = {"req": open(3, "w", buffering=1), "resp": open(4, "r", buffering=1)}
"#,
        });

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

        parse_child_output(status.as_ref(), stdout, stderr)
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

/// Windows transport: the shim talks line-JSON RPC over a loopback TCP socket
/// instead of fd3/4 (no dup2/inheritable-fd equivalent). The child authenticates
/// with a per-execution random token so unrelated local processes cannot ride
/// the listener; the listener binds 127.0.0.1:0 and accepts exactly one
/// connection before the host proceeds. Python is discovered via
/// `AVRAG_SANDBOX_PYTHON`, the bundle next to the exe, or PATH (probed —
/// WindowsApps ships a zero-byte store stub that silently exits 49).
#[cfg(windows)]
mod windows_impl {
    use super::shared::{parse_child_output, run_bridge_pump_sync, wait_child};
    use super::{
        build_bridge_sandbox_wrapper, ExecutionResult, HostBridge, InterpreterError, SandboxOpts,
    };
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::process::{Command, Stdio};
    use std::sync::Arc;
    use std::time::Duration;

    const BRIDGE_TOKEN_ENV: &str = "AVRAG_SANDBOX_BRIDGE_TOKEN";

    /// Resolve the Python interpreter for this host process.
    ///
    /// Order: `AVRAG_SANDBOX_PYTHON` (explicit override wins), bundle shipped
    /// next to the current exe (`python\python.exe`, the desktop install tree),
    /// then PATH candidates. Every candidate is probed with `--version` so the
    /// WindowsApps store stub (a 2-byte reparse exe that exits silently) is
    /// rejected instead of producing empty sandbox output.
    pub(super) fn resolve_python_path(configured: &str) -> Result<String, InterpreterError> {
        if let Ok(explicit) = std::env::var("AVRAG_SANDBOX_PYTHON") {
            let explicit = explicit.trim().to_string();
            if !explicit.is_empty() {
                return Ok(explicit);
            }
        }
        if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
        {
            let bundled = dir.join("python").join("python.exe");
            if bundled.is_file() {
                return Ok(bundled.to_string_lossy().into_owned());
            }
        }
        // `configured` is "python" on Windows (CodeInterpreter default).
        let mut candidates: Vec<String> = vec![configured.to_string(), "python".into()];
        candidates.retain(|c| !c.trim().is_empty());
        candidates.dedup();
        for cand in candidates {
            if probe_python(&cand) {
                return Ok(cand);
            }
        }
        Err(InterpreterError::PythonNotFound(
            "no usable python on Windows (set AVRAG_SANDBOX_PYTHON or bundle python/python.exe next to avrag-api.exe)"
                .to_string(),
        ))
    }

    fn probe_python(candidate: &str) -> bool {
        std::process::Command::new(candidate)
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .is_ok_and(|out| out.status.success())
    }

    fn transport_setup_py(port: u16, token: &str) -> String {
        format!(
            r#"_sock = socket.create_connection(("127.0.0.1", {port}), timeout=300)
_sock.sendall(("{token}\n").encode())
_req = _sock.makefile("w", buffering=1, encoding="utf-8")
_resp = _sock.makefile("r", buffering=1, encoding="utf-8")
_bridge_transport = {{"req": _req, "resp": _resp}}"#
        )
    }

    /// Accept exactly one connection, verify the token line, then hand the
    /// socket halves to the shared line-JSON pump.
    fn pump_accept(
        listener: TcpListener,
        expected_token: &str,
        bridge: Arc<dyn HostBridge>,
    ) -> Result<(), InterpreterError> {
        listener
            .set_nonblocking(false)
            .map_err(InterpreterError::Io)?;
        let (stream, _) = listener.accept().map_err(InterpreterError::Io)?;
        stream
            .set_read_timeout(Some(Duration::from_secs(3600)))
            .map_err(InterpreterError::Io)?;
        let mut reader = BufReader::new(&stream);
        let mut token_line = String::new();
        reader.read_line(&mut token_line).map_err(InterpreterError::Io)?;
        if token_line.trim() != expected_token {
            return Err(InterpreterError::Bridge(
                "sandbox bridge token mismatch".to_string(),
            ));
        }
        // Duplicate the stream so req/resp halves own independent handles.
        run_bridge_pump_sync_req_resp(stream, bridge)
    }

    /// Same line-JSON contract as the unix fd pump, over socket halves.
    /// The shared pump is generic over Read/Write; cloned TcpStreams feed it.
    fn run_bridge_pump_sync_req_resp(
        stream: std::net::TcpStream,
        bridge: Arc<dyn HostBridge>,
    ) -> Result<(), InterpreterError> {
        let req_stream = stream.try_clone().map_err(InterpreterError::Io)?;
        let resp_stream = stream;
        run_bridge_pump_sync(req_stream, resp_stream, bridge)
    }

    pub async fn execute_with_bridge(
        python_path: &str,
        timeout_secs: u64,
        memory_mb: u64,
        cpu_secs: u64,
        code: &str,
        bridge: Arc<dyn HostBridge>,
    ) -> Result<ExecutionResult, InterpreterError> {
        let python = resolve_python_path(python_path)?;
        let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(InterpreterError::Io)?;
        let port = listener.local_addr().map_err(InterpreterError::Io)?.port();
        let token = format!("{:032x}", rand_u64() | 1);
        let sandbox_code = build_bridge_sandbox_wrapper(&SandboxOpts {
            user_code: code,
            memory_mb,
            cpu_secs,
            // socket is BLOCKED post-hook; the TCP transport needs it first.
            prelude_imports: "import socket",
            transport_setup: &transport_setup_py(port, &token),
        });

        let (pump_ready_tx, pump_ready_rx) = tokio::sync::oneshot::channel();
        let pump_bridge = Arc::clone(&bridge);
        let pump_token = token.clone();
        std::thread::spawn(move || {
            let _ = pump_ready_tx.send(());
            if let Err(e) = pump_accept(listener, &pump_token, pump_bridge) {
                tracing::warn!("bridge pump ended with error: {e}");
            }
        });
        pump_ready_rx
            .await
            .map_err(|_| InterpreterError::Bridge("pump failed to start".to_string()))?;

        let temp_dir = tempfile::TempDir::new()
            .map_err(|e| InterpreterError::Io(std::io::Error::other(format!("temp dir: {e}"))))?;

        let mut command = Command::new(&python);
        command
            .arg("-c")
            .arg(&sandbox_code)
            .env(BRIDGE_TOKEN_ENV, &token)
            .current_dir(temp_dir.path())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = command.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                InterpreterError::PythonNotFound(python.clone())
            } else {
                InterpreterError::Io(e)
            }
        })?;

        // Job Object containment is best-effort: endpoint security (observed:
        // 360 on the acceptance machine) denies AssignProcessToJobObject with
        // ACCESS_DENIED even for freshly spawned children. Degrade to a
        // per-pid TerminateProcess on timeout; the wall timeout is enforced
        // either way.
        let job = JobObject::create(memory_mb, cpu_secs)
            .and_then(|job| job.assign(child.id()).map(|_| job))
            .ok();
        if job.is_none() {
            tracing::warn!(
                pid = child.id(),
                "job-object containment unavailable (endpoint security); per-pid kill on timeout"
            );
        }

        let timeout = Duration::from_secs(timeout_secs);
        let child_pid = child.id();
        let wait_result = tokio::time::timeout(timeout, wait_child(child)).await;
        let (status, stdout, stderr) = match wait_result {
            Ok(Ok(tuple)) => tuple,
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                match &job {
                    Some(j) => j.terminate(),
                    None => terminate_process_by_pid(child_pid),
                }
                return Err(InterpreterError::Timeout(timeout_secs));
            }
        };
        parse_child_output(status.as_ref(), stdout, stderr)
    }

    /// Fallback kill when job containment is unavailable: open the child pid
    /// with PROCESS_TERMINATE and terminate it. Best-effort (child may have
    /// exited between timeout and kill); python's own subprocesses are not
    /// covered.
    fn terminate_process_by_pid(pid: u32) {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            OpenProcess, TerminateProcess, PROCESS_TERMINATE,
        };
        unsafe {
            let proc = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if proc.is_null() {
                tracing::warn!(pid, "fallback kill could not open child process");
                return;
            }
            TerminateProcess(proc, 1);
            CloseHandle(proc);
        }
    }

    /// Best-effort per-run random u64 (no rand dep): xorshift over address
    /// entropy + nanos. Token strength is defense-in-depth on top of the
    /// 127.0.0.1-only listener + single-accept lifecycle.
    fn rand_u64() -> u64 {
        use std::time::Instant;
        let mut x = std::process::id() as u64 ^ Instant::now().elapsed().as_nanos() as u64;
        x ^= &x as *const u64 as u64;
        let mut s = x | 1;
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    }

    /// Job Object wrapper: process-tree containment + memory/CPU limits +
    /// `TerminateJobObject` on timeout (the killpg analogue).
    pub(crate) struct JobObject(WindowsHandle);

    pub(crate) struct WindowsHandle(WindowsSysHandle);
    struct WindowsSysHandle(*mut core::ffi::c_void);

    // A Win32 HANDLE is a process-wide kernel object identifier with no thread
    // affinity: TerminateJobObject/AssignProcessToJobObject/CloseHandle are all
    // callable from any thread in the owning process (same as std's own
    // Send/Sync impls for RawHandle wrappers like std::fs::File). Required
    // because execute_with_bridge holds the JobObject across .await points.
    unsafe impl Send for WindowsSysHandle {}

    impl JobObject {
        pub(crate) fn create(memory_mb: u64, cpu_secs: u64) -> Result<Self, InterpreterError> {
            use windows_sys::Win32::System::JobObjects::{
                CreateJobObjectW, JobObjectExtendedLimitInformation,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOB_OBJECT_LIMIT_PROCESS_MEMORY, JOB_OBJECT_LIMIT_PROCESS_TIME,
                SetInformationJobObject,
            };
            unsafe {
                let job = CreateJobObjectW(core::ptr::null(), core::ptr::null());
                if job.is_null() {
                    return Err(InterpreterError::Io(std::io::Error::last_os_error()));
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = core::mem::zeroed();
                info.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                    | JOB_OBJECT_LIMIT_PROCESS_TIME
                    | JOB_OBJECT_LIMIT_PROCESS_MEMORY;
                info.BasicLimitInformation.PerProcessUserTimeLimit =
                    (cpu_secs as i64) * 10_000_000; // 100ns units
                info.ProcessMemoryLimit = (memory_mb * 1024 * 1024) as usize;
                let ok = SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const core::ffi::c_void,
                    core::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );
                if ok == 0 {
                    return Err(InterpreterError::Io(std::io::Error::last_os_error()));
                }
                Ok(Self(WindowsHandle(WindowsSysHandle(job))))
            }
        }

        pub(crate) fn assign(&self, pid: u32) -> Result<(), InterpreterError> {
            use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
            use windows_sys::Win32::Foundation::CloseHandle;
            unsafe {
                let proc = open_process_for_job(pid)?;
                let ok = AssignProcessToJobObject((self.0).0 .0, proc);
                CloseHandle(proc);
                if ok == 0 {
                    return Err(InterpreterError::Io(std::io::Error::last_os_error()));
                }
                Ok(())
            }
        }

        pub(crate) fn terminate(&self) {
            use windows_sys::Win32::System::JobObjects::TerminateJobObject;
            unsafe {
                TerminateJobObject((self.0).0 .0, 1);
            }
        }
    }

    impl Drop for JobObject {
        fn drop(&mut self) {
            use windows_sys::Win32::Foundation::CloseHandle;
            unsafe { CloseHandle((self.0).0 .0) };
        }
    }

    fn open_process_for_job(pid: u32) -> Result<*mut core::ffi::c_void, InterpreterError> {
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA};
        unsafe {
            let proc = OpenProcess(PROCESS_SET_QUOTA, 0, pid);
            if proc.is_null() {
                return Err(InterpreterError::Io(std::io::Error::last_os_error()));
            }
            Ok(proc)
        }
    }
}

/// Windows non-bridge `execute()` helper: create a Job Object with the same
/// kill-on-close/limits policy and assign the spawned child, so the plain
/// sandbox path also gets process-tree containment + timeout kill.
#[cfg(windows)]
pub(crate) fn job_object_for_child(
    pid: u32,
    memory_mb: u64,
    cpu_secs: u64,
) -> Option<windows_impl::JobObject> {
    windows_impl::JobObject::create(memory_mb, cpu_secs)
        .and_then(|job| job.assign(pid).map(|_| job))
        .ok()
}

/// Windows non-bridge `execute()` helper: resolve a usable Python interpreter
/// (env override → bundle next to the exe → probed PATH candidates) so the
/// plain sandbox path cannot pick up the zero-byte WindowsApps store stub.
#[cfg(windows)]
pub(crate) fn resolve_python_path(configured: &str) -> Result<String, InterpreterError> {
    windows_impl::resolve_python_path(configured)
}


#[cfg(windows)]
pub use windows_impl::execute_with_bridge;
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
    use std::process::Command;
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


/// Platform-neutral bridge interop tests (unix fd transport + windows TCP
/// transport both run these). Python resolution goes through the same
/// production discovery as the runtime path.
#[cfg(test)]
mod bridge_interop_tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct EchoBridge {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl HostBridge for EchoBridge {
        async fn call(&self, method: &str, args: Value) -> Value {
            self.calls.fetch_add(1, Ordering::SeqCst);
            json!({ "method": method, "args": args })
        }
    }

    fn platform_python() -> Option<String> {
        #[cfg(unix)]
        {
            if std::process::Command::new("python3").arg("--version").output().is_ok() {
                return Some("python3".to_string());
            }
            None
        }
        #[cfg(windows)]
        {
            super::windows_impl::resolve_python_path("python").ok()
        }
    }

    /// Round trip: sandbox code calls `client.save` (module-level alias) and
    /// prints the response; the host bridge must observe the RPC and the
    /// captured stdout must carry the echoed payload.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn bridge_rpc_roundtrip_across_transport() {
        let Some(python) = platform_python() else {
            eprintln!("skipping: no usable python on this host");
            return;
        };
        let bridge = Arc::new(EchoBridge {
            calls: AtomicUsize::new(0),
        });
        let result = execute_with_bridge(
            &python,
            60,
            256,
            60,
            "data = await client.save('k', {'v': 42})\nprint('got', data['method'], data['args']['data']['v'])",
            bridge.clone(),
        )
        .await
        .expect("bridge execution");
        assert!(
            result.success || result.exit_code == Some(0),
            "sandbox failed: {} {}",
            result.stdout,
            result.stderr
        );
        assert_eq!(bridge.calls.load(Ordering::SeqCst), 1, "host must see 1 RPC");
        assert!(
            result.stdout.contains("got save 42"),
            "stdout must carry echoed RPC payload, got: {}",
            result.stdout
        );
    }

    /// A blocked import from user code must stay blocked on both transports
    /// (security hook parity), surfacing as a traceback in stderr.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn bridge_blocks_os_import_on_all_transports() {
        let Some(python) = platform_python() else {
            eprintln!("skipping: no usable python on this host");
            return;
        };
        let bridge = Arc::new(EchoBridge {
            calls: AtomicUsize::new(0),
        });
        let result = execute_with_bridge(
            &python,
            60,
            256,
            60,
            "import os\nprint('should not reach')",
            bridge,
        )
        .await
        .expect("bridge execution");
        assert!(
            result.stderr.contains("blocked for security reasons"),
            "expected import block traceback, got stderr: {}",
            result.stderr
        );
        assert!(
            !result.stdout.contains("should not reach"),
            "user code after blocked import must not run"
        );
    }
}
