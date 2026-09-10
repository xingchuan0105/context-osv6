//! Startup commands finish (or are killed and reaped) before their caller returns.
use std::{
    io::{self, Read},
    process::{Child, Command, ExitStatus, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

pub(crate) struct Deadline(Instant);

impl Deadline {
    pub(crate) fn after(duration: Duration) -> Self {
        Self(Instant::now() + duration)
    }

    pub(crate) fn check(&self) -> Result<(), String> {
        if Instant::now() < self.0 {
            Ok(())
        } else {
            Err("native startup timed out".into())
        }
    }

    pub(crate) fn status(&self, command: &mut Command) -> Result<ExitStatus, String> {
        self.check()?;
        crate::win_cmd::hide_console(command);
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        self.wait(&mut child)
    }

    pub(crate) fn output(&self, command: &mut Command) -> Result<Output, String> {
        self.check()?;
        crate::win_cmd::hide_console(command);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        // Drain both pipes while waiting: initdb output must not fill a pipe and stall.
        let stdout = drain(child.stdout.take().expect("stdout pipe"));
        let stderr = drain(child.stderr.take().expect("stderr pipe"));
        let status = self.wait(&mut child);
        let stdout = stdout
            .join()
            .map_err(|_| "stdout reader panicked")?
            .map_err(|e| e.to_string())?;
        let stderr = stderr
            .join()
            .map_err(|_| "stderr reader panicked")?
            .map_err(|e| e.to_string())?;
        Ok(Output {
            status: status?,
            stdout,
            stderr,
        })
    }

    pub(crate) fn wait(&self, child: &mut Child) -> Result<ExitStatus, String> {
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) if self.check().is_ok() => thread::sleep(Duration::from_millis(25)),
                result => {
                    let reason = result
                        .err()
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "native startup timed out".into());
                    terminate_and_wait(child)?;
                    return Err(reason);
                }
            }
        }
    }
}

fn drain(mut stream: impl Read + Send + 'static) -> thread::JoinHandle<io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes)?;
        Ok(bytes)
    })
}

pub(crate) fn terminate_and_wait(child: &mut Child) -> Result<(), String> {
    #[cfg(windows)]
    crate::win_cmd::kill_pid_tree(child.id());
    if child.try_wait().map_err(|e| e.to_string())?.is_none() {
        child
            .kill()
            .map_err(|e| format!("cannot stop startup child: {e}"))?;
    }
    child.wait().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_fixture() {
        let Ok(mode) = std::env::var("GPUI_DEADLINE_CHILD") else {
            return;
        };
        if mode == "pipes" {
            for _ in 0..1024 {
                println!("{}", "x".repeat(256));
                eprintln!("{}", "y".repeat(256));
            }
        } else {
            let marker = std::env::var("GPUI_DEADLINE_MARKER").unwrap();
            std::fs::write(&marker, std::process::id().to_string()).unwrap();
            thread::sleep(Duration::from_secs(30));
            std::fs::write(format!("{marker}.late"), "untracked work").unwrap();
        }
    }

    fn fixture(mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "process_deadline::tests::child_fixture",
                "--nocapture",
            ])
            .env("GPUI_DEADLINE_CHILD", mode);
        command
    }

    #[test]
    fn timeout_reaps_child_before_returning() {
        let marker = std::env::temp_dir().join(format!("gpui-deadline-{}", uuid::Uuid::new_v4()));
        let error = Deadline::after(Duration::from_secs(2))
            .output(fixture("sleep").env("GPUI_DEADLINE_MARKER", &marker))
            .unwrap_err();
        assert!(error.contains("timed out"), "{error}");
        let pid = std::fs::read_to_string(&marker).unwrap().parse().unwrap();
        assert!(crate::win_cmd::process_executable(pid).is_none());
        assert!(!marker.with_extension("late").exists());
        std::fs::remove_file(marker).unwrap();
    }

    #[test]
    fn captures_both_full_pipes_without_deadlock() {
        let output = Deadline::after(Duration::from_secs(10))
            .output(&mut fixture("pipes"))
            .unwrap();
        assert!(output.status.success());
        assert!(output.stdout.len() > 256_000);
        assert!(output.stderr.len() > 256_000);
    }

    #[test]
    fn expired_deadline_never_spawns() {
        let mut command = Command::new("must-not-be-spawned");
        let error = Deadline::after(Duration::ZERO)
            .status(&mut command)
            .unwrap_err();
        assert!(error.contains("timed out"));
    }
}
