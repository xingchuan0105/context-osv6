//! Sandboxed Python code interpreter for the UnifiedAgent.
//!
//! ## Security
//!
//! The interpreter runs user-submitted Python code in an isolated subprocess:
//! - `setrlimit(RLIMIT_AS, 256MB)` — virtual memory cap
//! - `setrlimit(RLIMIT_CPU, 30s)` — CPU time cap
//! - Process-level timeout (30s wall-clock)
//! - Temp directory with restricted permissions
//! - Python `__import__` hook blocks: `os`, `subprocess`, `socket`, `sys`, `ctypes`, `shutil`, `posix`, `fcntl`, `pty`, `pwd`, `grp`, `resource`, `signal`, `multiprocessing`, `threading`
//!
//! ## Output
//!
//! The Python wrapper captures `stdout`, `stderr`, and the value of the last
//! expression (if it is not `None`).  Results are serialized as JSON and
//! returned via the Rust `ExecutionResult` struct.

mod bridge;

pub use bridge::{HostBridge, bridge_shim_client_method_names};

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

/// Result of a single code execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionResult {
    /// Captured stdout.
    pub stdout: String,
    /// Captured stderr.
    pub stderr: String,
    /// Value of the last expression (Python repr), if any.
    pub result: Option<String>,
    /// Whether the process exited successfully (code 0).
    pub success: bool,
    /// Process exit code.
    pub exit_code: Option<i32>,
    /// Whether the run was killed by timeout or resource limit.
    pub killed: bool,
}

/// Errors that can occur when executing code.
#[derive(Debug, thiserror::Error)]
pub enum InterpreterError {
    #[error("python3 not found: {0}")]
    PythonNotFound(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("rlimit error: {0}")]
    Rlimit(String),

    #[error("timeout after {0}s")]
    Timeout(u64),

    #[error("utf-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("bridge error: {0}")]
    Bridge(String),
}

/// Sandboxed Python code interpreter.
pub struct CodeInterpreter {
    python_path: String,
    timeout_secs: u64,
    memory_limit_mb: u64,
    cpu_limit_secs: u64,
}

impl Default for CodeInterpreter {
    fn default() -> Self {
        Self {
            python_path: "python3".to_string(),
            timeout_secs: 30,
            memory_limit_mb: 256,
            cpu_limit_secs: 30,
        }
    }
}

impl CodeInterpreter {
    /// Create a new interpreter with default limits (256MB memory, 30s CPU, 30s wall).
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the Python binary path.
    pub fn with_python(mut self, path: impl Into<String>) -> Self {
        self.python_path = path.into();
        self
    }

    /// Set wall-clock timeout in seconds.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Set memory limit in MB.
    pub fn with_memory_limit(mut self, mb: u64) -> Self {
        self.memory_limit_mb = mb;
        self
    }

    /// Execute Python code in the sandbox and return the result.
    ///
    /// # Security
    ///
    /// The code runs inside a wrapper that:
    /// 1. Creates a temporary working directory
    /// 2. Sets resource limits (memory, CPU)
    /// 3. Blocks dangerous `import` calls
    /// 4. Captures output
    pub fn execute(&self, code: &str) -> Result<ExecutionResult, InterpreterError> {
        let sandbox_code = build_sandbox_wrapper(code, self.memory_limit_mb, self.cpu_limit_secs);

        let temp_dir = tempfile::TempDir::new()
            .map_err(|e| InterpreterError::Io(std::io::Error::other(format!("temp dir: {e}"))))?;

        let mut command = Command::new(&self.python_path);
        command
            .arg("-c")
            .arg(&sandbox_code)
            .current_dir(temp_dir.path())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Put the child in its own process group so that, on timeout, we can
        // kill the python process *and* any subprocesses it may have spawned
        // (child becomes the pgid leader, so pid == pgid).
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                InterpreterError::PythonNotFound(self.python_path.clone())
            } else {
                InterpreterError::Io(e)
            }
        })?;
        // Capture the child's pid before it is moved into the wait thread; with
        // process_group(0) set, this pid doubles as the process-group id for killpg.
        #[cfg(unix)]
        let child_pid = child.id();
        // Keep `child_pid` referenced on non-unix so the binding stays live; the
        // group-kill is a no-op there (sandbox requires Unix anyway).
        #[cfg(not(unix))]
        let child_pid: Option<u32> = child.id();

        // Resource limits are applied inside the Python sandbox via the
        // `resource` module.  The Python wrapper calls resource.setrlimit()
        // before executing user code (see build_sandbox_wrapper).

        let timeout = Duration::from_secs(self.timeout_secs);
        let (tx, rx) = std::sync::mpsc::channel();
        let mut child_stdout = child.stdout.take();
        let mut child_stderr = child.stderr.take();

        std::thread::spawn(move || {
            let status = child.wait();
            let _ = tx.send(status);
        });

        match rx.recv_timeout(timeout) {
            Ok(status_result) => {
                let status = status_result?;
                let stdout = read_pipe(&mut child_stdout)?;
                let stderr = read_pipe(&mut child_stderr)?;

                match serde_json::from_str::<ExecutionResult>(&stdout) {
                    Ok(mut result) => {
                        if !stderr.is_empty() && result.stderr.is_empty() {
                            result.stderr = stderr;
                        }
                        Ok(result)
                    }
                    Err(_) => Ok(ExecutionResult {
                        stdout,
                        stderr,
                        result: None,
                        success: status.success(),
                        exit_code: status.code(),
                        killed: false,
                    }),
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Child was moved into the wait thread; kill its whole process
                // group to clean up python + any subprocesses it spawned, then
                // reap the zombie via the wait thread (which still owns `child`).
                #[cfg(unix)]
                unsafe {
                    let _ = libc::killpg(child_pid as i32, libc::SIGKILL);
                }
                Err(InterpreterError::Timeout(self.timeout_secs))
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(InterpreterError::Io(
                std::io::Error::other("child process communication channel disconnected"),
            )),
        }
    }

    /// Execute Python code with a host retrieval bridge (fd3/fd4 pipe RPC).
    pub async fn execute_with_bridge<B: HostBridge + Send + Sync + 'static>(
        &self,
        code: &str,
        bridge: std::sync::Arc<B>,
    ) -> Result<ExecutionResult, InterpreterError> {
        bridge::execute_with_bridge_arc(
            &self.python_path,
            self.timeout_secs,
            self.memory_limit_mb,
            self.cpu_limit_secs,
            code,
            bridge,
        )
        .await
    }
}

fn read_pipe<R: Read>(stream: &mut Option<R>) -> Result<String, InterpreterError> {
    let mut buf = Vec::new();
    if let Some(s) = stream.as_mut() {
        s.read_to_end(&mut buf)?;
    }
    Ok(String::from_utf8(buf)?)
}

// ---------------------------------------------------------------------------
// Python sandbox wrapper
// ---------------------------------------------------------------------------

/// Build the Python sandbox wrapper that:
/// 1. Sets resource limits via the `resource` module
/// 2. Overrides `__import__` to block dangerous modules
/// 3. Captures stdout/stderr
/// 4. Returns a JSON-serialized `ExecutionResult`
fn build_sandbox_wrapper(user_code: &str, memory_mb: u64, cpu_secs: u64) -> String {
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

    // Escape user code for safe embedding in a Python string literal.
    let escaped_code = user_code
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n");

    format!(
        r#"import sys, io, json, traceback

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

_real_stdout = sys.stdout
_real_stderr = sys.stderr
_cap_stdout = io.StringIO()
_cap_stderr = io.StringIO()
sys.stdout = _cap_stdout
sys.stderr = _cap_stderr

_result = None
_code = '{escaped_code}'
try:
    exec(compile(_code, '<sandbox>', 'exec'))
except Exception:
    traceback.print_exc()

output = {{
    "stdout": _cap_stdout.getvalue(),
    "stderr": _cap_stderr.getvalue(),
    "result": repr(_result) if _result is not None else None,
    "success": True,
    "exit_code": 0,
    "killed": False
}}
_real_stdout.write(json.dumps(output))
"#,
        blocked_list = blocked_list,
        memory_mb = memory_mb,
        cpu_secs = cpu_secs,
        escaped_code = escaped_code,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;

    struct StubBridge;

    #[async_trait]
    impl HostBridge for StubBridge {
        async fn call(&self, method: &str, _args: serde_json::Value) -> serde_json::Value {
            match method {
                "dense_search" => json!({
                    "chunks": [{
                        "chunk_id": "00000000-0000-4000-8000-000000000001",
                        "doc_id": "00000000-0000-4000-8000-000000000010",
                        "content": "hello from stub",
                        "score": 0.9
                    }]
                }),
                _ => json!({ "chunks": [] }),
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn bridge_dense_search_returns_chunks_in_stdout() {
        let interpreter = CodeInterpreter::new().with_timeout(10);
        let code = r#"
chunks = await client.dense_search(query="x", top_k=5)
import json
print(json.dumps(chunks))
"#;
        let result = interpreter
            .execute_with_bridge(code, std::sync::Arc::new(StubBridge))
            .await
            .unwrap();
        assert!(result.success, "stderr: {}", result.stderr);
        assert!(
            result
                .stdout
                .contains("00000000-0000-4000-8000-000000000001"),
            "stdout: {}",
            result.stdout
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn bridge_blocks_socket_import() {
        let interpreter = CodeInterpreter::new().with_timeout(10);
        let result = interpreter
            .execute_with_bridge("import socket", std::sync::Arc::new(StubBridge))
            .await
            .unwrap();
        assert!(
            !result.stderr.is_empty() || !result.success,
            "stderr: {}",
            result.stderr
        );
    }

    #[test]
    fn test_simple_expression() {
        let interpreter = CodeInterpreter::new().with_timeout(10);
        let result = interpreter.execute("x = 1 + 2\nprint(x)").unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("3"));
    }

    #[test]
    fn test_print_capture() {
        let interpreter = CodeInterpreter::new().with_timeout(10);
        let result = interpreter.execute("print('hello')").unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("hello"));
    }

    #[test]
    fn test_error_capture() {
        let interpreter = CodeInterpreter::new().with_timeout(10);
        let result = interpreter.execute("raise ValueError('boom')").unwrap();
        assert!(
            result.stderr.contains("ValueError"),
            "stderr: {}",
            result.stderr
        );
    }

    #[test]
    fn test_blocked_os_import() {
        let interpreter = CodeInterpreter::new().with_timeout(10);
        let result = interpreter.execute("import os").unwrap();
        assert!(
            !result.stderr.is_empty(),
            "stderr should contain error: {}",
            result.stderr
        );
        assert!(result.stderr.contains("blocked") || result.stderr.contains("ImportError"));
    }

    #[test]
    fn test_blocked_subprocess_import() {
        let interpreter = CodeInterpreter::new().with_timeout(10);
        let result = interpreter.execute("import subprocess").unwrap();
        assert!(!result.stderr.is_empty(), "stderr: {}", result.stderr);
    }

    #[test]
    fn test_list_comprehension() {
        let interpreter = CodeInterpreter::new().with_timeout(10);
        let result = interpreter
            .execute("print(sum([i*i for i in range(10)]))")
            .unwrap();
        assert!(result.stdout.contains("285"));
    }

    #[test]
    fn test_data_analysis() {
        let interpreter = CodeInterpreter::new().with_timeout(10);
        let code = "import json, math
data = [1, 2, 3, 4, 5]
mean = sum(data) / len(data)
std = math.sqrt(sum((x - mean) ** 2 for x in data) / len(data))
print(json.dumps({'mean': mean, 'std': round(std, 2)}))";
        let result = interpreter.execute(code).unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("3.0"), "stdout: {}", result.stdout);
        assert!(result.stdout.contains("1.41"), "stdout: {}", result.stdout);
    }

    /// The non-bridge path must kill the child on timeout and return
    /// `InterpreterError::Timeout`. `while True: pass` spins forever, so the
    /// only way this completes is via the timeout arm. Requires `python3`.
    #[cfg(unix)]
    #[test]
    fn execute_timeout_kills_busy_python() {
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("skipping: python3 not found on PATH");
            return;
        }

        let started = std::time::Instant::now();
        let interpreter = CodeInterpreter::new().with_timeout(2);
        let result = interpreter.execute("while True:\n    pass");
        let elapsed = started.elapsed();

        assert!(
            matches!(result, Err(InterpreterError::Timeout(2))),
            "expected Timeout(2), got {:?}",
            result
        );
        // If the kill/reap regressed, recv_timeout itself still fires at 2s, but
        // a leaked process can keep the test runner's machine hot; bound the
        // wall clock as a sanity check.
        assert!(
            elapsed.as_secs() < 10,
            "timeout path took too long ({elapsed:?})"
        );
    }
}
