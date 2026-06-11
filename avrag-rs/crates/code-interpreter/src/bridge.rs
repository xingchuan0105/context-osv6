//! Sandbox retrieval bridge: line-delimited JSON RPC over fd3/fd4 pipes.

use std::io::{BufRead, Read, Write};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use crate::{ExecutionResult, InterpreterError};

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
    r#"
import json as _json
_req = open(3, "w", buffering=1)
_resp = open(4, "r", buffering=1)
_id = 0

def _rpc(method, args):
    global _id
    _id += 1
    _req.write(_json.dumps({"id": _id, "method": method, "args": args}) + "\n")
    _req.flush()
    msg = _json.loads(_resp.readline())
    if not msg.get("ok"):
        raise RuntimeError(msg.get("error", {}).get("message", "bridge error"))
    return msg["data"]

class _Client:
    async def dense_search(self, query, top_k=10, method="auto"):
        return _rpc("dense_search", {"query": query, "top_k": top_k, "method": method})["chunks"]
    async def lexical_search(self, query, top_k=10):
        return _rpc("lexical_search", {"query": query, "top_k": top_k})["chunks"]
    async def graph_search(self, query, depth=2):
        return _rpc("graph_search", {"query": query, "depth": depth})["chunks"]
    async def chunk_fetch(self, chunk_id):
        return _rpc("chunk_fetch", {"chunk_id": chunk_id})["chunks"]
    async def doc_summary(self, doc_ids, level="doc"):
        return _rpc("doc_summary", {"doc_ids": doc_ids, "level": level})["chunks"]
    async def doc_profile(self, doc_ids, fields=None):
        payload = {"doc_ids": doc_ids}
        if fields:
            payload["fields"] = fields
        return _rpc("doc_profile", payload)["chunks"]

client = _Client()
"#
}

/// RPC method names exposed on the injected Python `client` (must match host bridge).
pub fn bridge_shim_client_method_names() -> &'static [&'static str] {
    &[
        "dense_search",
        "lexical_search",
        "graph_search",
        "chunk_fetch",
        "doc_summary",
        "doc_profile",
    ]
}

pub(crate) fn build_bridge_sandbox_wrapper(user_code: &str, memory_mb: u64) -> String {
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
"#,
        blocked_list = blocked_list,
        memory_mb = memory_mb,
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
            // Multi-thread: doc_profile/doc_metadata use tokio::join! for parallel PG
            // reads; a current-thread runtime can stall when the shared pool is busy.
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("bridge pump runtime")
        })
    }

    pub async fn execute_with_bridge(
        python_path: &str,
        timeout_secs: u64,
        memory_mb: u64,
        code: &str,
        bridge: Arc<dyn HostBridge>,
    ) -> Result<ExecutionResult, InterpreterError> {
        let sandbox_code = build_bridge_sandbox_wrapper(code, memory_mb);

        let (req_reader, req_writer) = std::io::pipe().map_err(InterpreterError::Io)?;
        let (resp_reader, resp_writer) = std::io::pipe().map_err(InterpreterError::Io)?;

        let req_write_fd = req_writer.into_raw_fd();
        let resp_read_fd = resp_reader.into_raw_fd();

        // Prevent Rust's pre-exec fd sweep from closing bridge fds before dup2.
        unsafe {
            libc::fcntl(req_write_fd, libc::F_SETFD, 0);
            libc::fcntl(resp_read_fd, libc::F_SETFD, 0);
        }

        let temp_dir = tempfile::TempDir::new().map_err(|e| {
            InterpreterError::Io(std::io::Error::other(format!("temp dir: {e}")))
        })?;

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

        let timeout = Duration::from_secs(timeout_secs);
        let wait_result = tokio::time::timeout(timeout, wait_child(child)).await;

        let (status, stdout, stderr) = match wait_result {
            Ok(Ok(tuple)) => tuple,
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(InterpreterError::Timeout(timeout_secs)),
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
                Ok((Some(status), String::from_utf8(stdout)?, String::from_utf8(stderr)?))
            })();
            let _ = tx.send(result);
        });

        rx.await
            .map_err(|_| InterpreterError::Bridge("child wait channel closed".to_string()))?
    }

    fn run_bridge_pump_sync(
        req_file: std::fs::File,
        mut resp_file: std::fs::File,
        bridge: Arc<dyn HostBridge>,
    ) -> Result<(), InterpreterError> {
        let mut reader = std::io::BufReader::new(req_file);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let response_line = match serde_json::from_str::<BridgeRequest>(line.trim()) {
                        Ok(req) => {
                            let data =
                                bridge_pump_runtime().block_on(bridge.call(&req.method, req.args));
                            bridge_ok_response(req.id, data)
                        }
                        Err(e) => bridge_error_response(0, "invalid_request", format!("{e}")),
                    };
                    resp_file
                        .write_all(response_line.as_bytes())
                        .map_err(InterpreterError::Io)?;
                    resp_file.write_all(b"\n").map_err(InterpreterError::Io)?;
                    resp_file.flush().map_err(InterpreterError::Io)?;
                }
                Err(e) => return Err(InterpreterError::Io(e)),
            }
        }
        Ok(())
    }
}

#[cfg(not(unix))]
pub async fn execute_with_bridge(
    _python_path: &str,
    _timeout_secs: u64,
    _memory_mb: u64,
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
    fn shim_exposes_only_host_supported_methods() {
        assert_eq!(
            bridge_shim_client_method_names(),
            &[
                "dense_search",
                "lexical_search",
                "graph_search",
                "chunk_fetch",
                "doc_summary",
                "doc_profile",
            ]
        );
        assert!(!bridge_shim_client_method_names().contains(&"rerank"));
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
        let write_fd = write_end.into_raw_fd();
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
        assert!(output.status.success(), "stderr={}", String::from_utf8_lossy(&output.stderr));

        let mut buf = [0u8; 16];
        let n = read_end.read(&mut buf).expect("read pipe");
        assert_eq!(&buf[..n], b"hello");
    }
}

pub(crate) async fn execute_with_bridge_arc<B: HostBridge + Send + Sync + 'static>(
    python_path: &str,
    timeout_secs: u64,
    memory_mb: u64,
    code: &str,
    bridge: Arc<B>,
) -> Result<ExecutionResult, InterpreterError> {
    let bridge: Arc<dyn HostBridge> = bridge;
    execute_with_bridge(python_path, timeout_secs, memory_mb, code, bridge).await
}
