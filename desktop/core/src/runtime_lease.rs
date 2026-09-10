//! Ownership of processes started during one host lifetime. No install-tree sweeps.
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Component {
    Api,
    Worker,
    Postgres,
    Redis,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProcessRecord {
    component: Component,
    file: PathBuf,
    pid: u32,
    executable: PathBuf,
}

fn read_record(component: Component, file: PathBuf) -> Option<ProcessRecord> {
    let pid = fs::read_to_string(&file)
        .ok()?
        .lines()
        .next()?
        .trim()
        .parse::<u32>()
        .ok()?;
    if pid <= 1 {
        return None;
    }
    let executable = crate::win_cmd::process_executable(pid)?;
    Some(ProcessRecord {
        component,
        file,
        pid,
        executable,
    })
}

#[derive(Default)]
pub struct ProcessSnapshot(Vec<ProcessRecord>);

impl ProcessSnapshot {
    pub fn capture() -> Self {
        let Some(root) = crate::native_stack::runtime_home() else {
            return Self::default();
        };
        Self::at(&root)
    }

    fn at(root: &Path) -> Self {
        Self(
            [
                (Component::Api, "run/api.pid"),
                (Component::Worker, "run/worker.pid"),
                (Component::Postgres, "run/postgres-native.pid"),
                (Component::Redis, "run/redis-native.pid"),
            ]
            .into_iter()
            .filter_map(|(component, file)| read_record(component, root.join(file)))
            .filter(|_record| {
                // Separate Windows clients can share a runtime directory. Only the
                // process that actually spawned a sidecar may adopt its PID record.
                #[cfg(windows)]
                { crate::win_cmd::process_parent(_record.pid) == Some(std::process::id()) }
                #[cfg(not(windows))]
                { true }
            })
            .collect(),
        )
    }
}

#[derive(Default)]
pub struct RuntimeLease(Vec<ProcessRecord>);

impl RuntimeLease {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn record_started_since(&mut self, before: &ProcessSnapshot) {
        self.record_new(before, ProcessSnapshot::capture());
    }

    fn record_new(&mut self, before: &ProcessSnapshot, after: ProcessSnapshot) {
        for process in after.0 {
            if !before.0.contains(&process) {
                self.0.retain(|old| old.file != process.file);
                self.0.push(process);
            }
        }
    }

    /// Product processes precede data services. Changed PID/executable records are left alone.
    pub fn shutdown(&mut self) -> Result<(), String> {
        self.shutdown_with(stop_record)
    }

    fn shutdown_with(
        &mut self,
        mut stop: impl FnMut(&ProcessRecord) -> Result<(), String>,
    ) -> Result<(), String> {
        self.0.sort_by_key(|p| match p.component {
            Component::Api => 0,
            Component::Worker => 1,
            Component::Postgres => 2,
            Component::Redis => 3,
        });
        let mut processes = std::mem::take(&mut self.0).into_iter();
        while let Some(process) = processes.next() {
            if let Err(error) = stop(&process) {
                self.0.push(process);
                self.0.extend(processes);
                return Err(error);
            }
        }
        Ok(())
    }
}

fn stop_record(process: &ProcessRecord) -> Result<(), String> {
    if read_record(process.component, process.file.clone()).as_ref() != Some(process) {
        return Ok(());
    }
    let result = if process.component == Component::Postgres {
        crate::native_stack::stop_owned_postgres(
            &process.file.parent().and_then(Path::parent).expect("runtime directory")
                .join("data/pg-native"),
        )
    } else {
        stop_process(process.pid);
        if crate::win_cmd::process_executable(process.pid).as_ref() == Some(&process.executable) {
            Err(format!(
                "{:?} pid {} did not stop",
                process.component, process.pid
            ))
        } else {
            Ok(())
        }
    };
    result?;
    // Never remove a replacement process record. PG manages postmaster.pid separately.
    if fs::read_to_string(&process.file)
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            == Some(process.pid)
    {
        let _ = fs::remove_file(&process.file);
    }
    Ok(())
}

fn stop_process(pid: u32) {
    #[cfg(windows)]
    {
        crate::win_cmd::kill_pid_tree(pid);
    }
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status();
    }
    // Let an already signalled process exit before reporting the result.
    for _ in 0..20 {
        if crate::win_cmd::process_executable(pid).is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn another_parent_process_is_never_adopted() {
        let root = std::env::temp_dir().join(format!("cos-parent-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("run")).unwrap();
        let file = root.join("run/api.pid");
        fs::write(&file, std::process::id().to_string()).unwrap();
        assert_ne!(crate::win_cmd::process_parent(std::process::id()), Some(std::process::id()));
        assert!(ProcessSnapshot::at(&root).0.is_empty());
        fs::remove_file(file).unwrap();
        fs::remove_dir(root.join("run")).unwrap();
        fs::remove_dir(root).unwrap();
    }
    fn process(component: Component, pid: u32) -> ProcessRecord {
        ProcessRecord {
            component,
            file: PathBuf::from(format!("{component:?}.pid")),
            pid,
            executable: PathBuf::from("synthetic"),
        }
    }

    #[test]
    fn inherited_services_are_not_owned_and_new_components_are_retained() {
        let postgres = process(Component::Postgres, 100);
        let api = process(Component::Api, 200);
        let before = ProcessSnapshot(vec![postgres.clone()]);
        let mut lease = RuntimeLease::default();
        lease.record_new(&before, ProcessSnapshot(vec![postgres, api.clone()]));
        assert_eq!(lease.0, vec![api.clone()]);
        lease.record_new(
            &ProcessSnapshot(vec![api.clone()]),
            ProcessSnapshot(vec![api.clone()]),
        );
        assert_eq!(lease.0, vec![api]);
    }

    #[test]
    fn failed_product_shutdown_leaves_data_services_for_retry() {
        let mut lease = RuntimeLease(vec![
            process(Component::Redis, 400),
            process(Component::Postgres, 300),
            process(Component::Worker, 200),
            process(Component::Api, 100),
        ]);
        let mut stopped = Vec::new();
        assert!(lease
            .shutdown_with(|p| {
                stopped.push(p.component);
                if p.component == Component::Worker {
                    Err("still busy".into())
                } else {
                    Ok(())
                }
            })
            .is_err());
        assert_eq!(stopped, vec![Component::Api, Component::Worker]);
        stopped.clear();
        lease
            .shutdown_with(|p| {
                stopped.push(p.component);
                Ok(())
            })
            .unwrap();
        assert_eq!(
            stopped,
            vec![Component::Worker, Component::Postgres, Component::Redis]
        );
        assert!(lease.is_empty());
    }

    #[test]
    fn replaced_or_missing_records_are_never_terminated() {
        let dir = std::env::temp_dir().join(format!("gpui-lease-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("api.pid");
        fs::write(&file, std::process::id().to_string()).unwrap();
        let mut record = read_record(Component::Api, file.clone()).unwrap();
        record.executable = PathBuf::from("different-executable");
        let mut lease = RuntimeLease(vec![record]);
        lease.shutdown().unwrap();
        assert!(lease.is_empty());
        assert!(file.exists());
        fs::remove_file(file).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
