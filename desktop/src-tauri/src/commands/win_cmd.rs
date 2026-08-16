//! Windows process helpers: never flash a console (no powershell / taskkill / cmd).
//!
//! Console subsystem tools (`pg_ctl`, `redis-server`, `avrag-api`, `curl`) still
//! need `CREATE_NO_WINDOW`. Killing uses Win32 `TerminateProcess` + Toolhelp.

use std::path::PathBuf;
use std::process::Command;

/// Hide any console window this child would otherwise flash.
pub fn hide_console(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = cmd;
}

/// Long-lived child (redis / api / worker).
///
/// **Do not** combine `CREATE_NO_WINDOW` with `DETACHED_PROCESS`:
/// MSDN: CREATE_NO_WINDOW is **ignored** if used with DETACHED_PROCESS /
/// CREATE_NEW_CONSOLE — Windows then allocates a visible console (the 4 flashes).
pub fn hide_and_detach(cmd: &mut Command) {
    hide_console(cmd);
}

/// Terminate `pid` and its descendants. Returns how many processes were signalled.
pub fn kill_pid_tree(pid: u32) -> u32 {
    #[cfg(windows)]
    {
        return win::kill_tree(pid);
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        0
    }
}

/// Kill named processes whose executable path sits under any of `roots`.
pub fn kill_named_under(names: &[&str], roots: &[PathBuf]) -> Vec<String> {
    #[cfg(windows)]
    {
        return win::kill_named_under(names, roots);
    }
    #[cfg(not(windows))]
    {
        let _ = (names, roots);
        Vec::new()
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::path::Path;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, TerminateProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        PROCESS_TERMINATE,
    };

    struct Snap(HANDLE);
    impl Drop for Snap {
        fn drop(&mut self) {
            if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
                unsafe { CloseHandle(self.0) };
            }
        }
    }

    fn snapshot() -> Option<Snap> {
        let h = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if h.is_null() || h == INVALID_HANDLE_VALUE {
            return None;
        }
        Some(Snap(h))
    }

    fn exe_path(pid: u32) -> Option<PathBuf> {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return None;
            }
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len);
            CloseHandle(h);
            if ok == 0 || len == 0 {
                return None;
            }
            Some(PathBuf::from(OsString::from_wide(&buf[..len as usize])))
        }
    }

    fn terminate(pid: u32) -> bool {
        if pid == 0 {
            return false;
        }
        unsafe {
            let h = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if h.is_null() {
                return false;
            }
            let ok = TerminateProcess(h, 1);
            CloseHandle(h);
            ok != 0
        }
    }

    fn children_of(parent: u32) -> Vec<u32> {
        let Some(snap) = snapshot() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        unsafe {
            let mut pe: PROCESSENTRY32W = std::mem::zeroed();
            pe.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snap.0, &mut pe) == 0 {
                return out;
            }
            loop {
                if pe.th32ParentProcessID == parent && pe.th32ProcessID != parent {
                    out.push(pe.th32ProcessID);
                }
                if Process32NextW(snap.0, &mut pe) == 0 {
                    break;
                }
            }
        }
        out
    }

    pub fn kill_tree(pid: u32) -> u32 {
        let mut n = 0u32;
        for c in children_of(pid) {
            n += kill_tree(c);
        }
        if terminate(pid) {
            n += 1;
        }
        n
    }

    fn under_roots(path: &Path, roots: &[PathBuf]) -> bool {
        let ps = path.to_string_lossy().to_ascii_lowercase();
        roots.iter().any(|r| {
            let rs = r.to_string_lossy().to_ascii_lowercase();
            let rs = rs.trim_end_matches(['\\', '/']);
            !rs.is_empty() && (ps == rs || ps.starts_with(&format!("{rs}\\")) || ps.starts_with(&format!("{rs}/")))
        })
    }

    pub fn kill_named_under(names: &[&str], roots: &[PathBuf]) -> Vec<String> {
        let want: Vec<String> = names.iter().map(|s| s.to_ascii_lowercase()).collect();
        let Some(snap) = snapshot() else {
            return vec!["toolhelp snapshot failed".into()];
        };
        let mut lines = Vec::new();
        unsafe {
            let mut pe: PROCESSENTRY32W = std::mem::zeroed();
            pe.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snap.0, &mut pe) == 0 {
                return vec!["Process32First failed".into()];
            }
            loop {
                let name = {
                    let raw = &pe.szExeFile;
                    let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
                    OsString::from_wide(&raw[..end])
                        .to_string_lossy()
                        .to_ascii_lowercase()
                };
                let stem = name.trim_end_matches(".exe");
                if want.iter().any(|w| w == stem || *w == name) {
                    let pid = pe.th32ProcessID;
                    if let Some(path) = exe_path(pid) {
                        if under_roots(&path, roots) {
                            let n = kill_tree(pid);
                            lines.push(format!(
                                "kill pid={pid} name={stem} n={n} path={}",
                                path.display()
                            ));
                        }
                    }
                }
                if Process32NextW(snap.0, &mut pe) == 0 {
                    break;
                }
            }
        }
        lines
    }
}
