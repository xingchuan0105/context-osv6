//! GPUI orchestration over the shared Tauri core; presentation data has no credentials.
use desktop_core::{
    LocalProductStatus, LocalStackStatus,
    runtime_lease::{ProcessSnapshot, RuntimeLease},
};
use std::path::PathBuf;

// A concurrent Host must take its ownership snapshot after the preceding startup.
static MANAGED_START: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    Checking,
    StartingStack,
    StartingProduct,
    Connecting,
    Ready,
    Stopping,
    Stopped,
    Failed,
}
impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "尚未连接",
            Self::Checking => "正在检查本机服务…",
            Self::StartingStack => "正在准备数据库与缓存…",
            Self::StartingProduct => "正在迁移并启动服务…",
            Self::Connecting => "正在恢复本机会话…",
            Self::Ready => "本机服务已连接",
            Self::Stopping => "正在停止本次启动的服务…",
            Self::Stopped => "已断开本机会话",
            Self::Failed => "本机服务需要处理",
        }
    }
    pub fn busy(self) -> bool {
        matches!(
            self,
            Self::Checking
                | Self::StartingStack
                | Self::StartingProduct
                | Self::Connecting
                | Self::Stopping
        )
    }
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub product: LocalProductStatus,
    pub stack: Option<LocalStackStatus>,
    pub tools_available: bool,
    pub env_exists: bool,
    pub logs: PathBuf,
    pub attached: bool,
    pub owned: bool,
}

pub struct Services {
    attached: bool,
    lease: RuntimeLease,
}
impl Services {
    pub fn new() -> Self {
        let attached = ["CLIENT_API_BASE_URL", "AVRAG_PUBLIC_BASE_URL"]
            .iter()
            .any(|key| std::env::var(key).is_ok_and(|v| !v.trim().is_empty()));
        Self {
            attached,
            lease: RuntimeLease::default(),
        }
    }
    pub async fn snapshot(&self) -> Result<Snapshot, String> {
        let attached = self.attached;
        let owned = !self.lease.is_empty();
        tokio::task::spawn_blocking(move || Snapshot {
            product: desktop_core::local_product::get_local_product_status(),
            logs: if attached {
                desktop_core::native_stack::runtime_home()
                    .unwrap_or_else(desktop_core::local_product::log_dir_path)
            } else {
                desktop_core::local_product::log_dir_path()
            },
            attached,
            owned,
            stack: (!attached).then(desktop_core::local_stack::get_local_stack_status),
            tools_available: !attached && desktop_core::native_tools_available(),
            env_exists: desktop_core::native_stack::runtime_home()
                .is_some_and(|p| p.join("client.env").is_file()),
        })
        .await
        .map_err(|e| e.to_string())
    }
    pub async fn connect(
        &mut self,
        data_dir: PathBuf,
        mut progress: impl FnMut(Phase),
    ) -> Result<desktop_core::LocalSessionStatus, String> {
        progress(Phase::Checking);
        let _startup = if self.attached { None } else { Some(MANAGED_START.lock().await) };
        let snapshot = self.snapshot().await?;
        if self.attached {
            if !snapshot.product.api_ok {
                return Err("指定的本机服务尚未就绪，请确认服务已启动后重试连接。".into());
            }
        } else {
            let before = ProcessSnapshot::capture();
            let result = self.start_managed(&snapshot, &mut progress).await;
            // Retain partial starts after migration errors so they can also be cleaned up.
            self.lease.record_started_since(&before);
            result?;
        }
        progress(Phase::Connecting);
        desktop_core::local_session::connect_local_session(&data_dir)
            .await
            .map_err(|e| e.to_string())
    }
    async fn start_managed(
        &self,
        snapshot: &Snapshot,
        progress: &mut impl FnMut(Phase),
    ) -> Result<(), String> {
        if snapshot.product.overall_ok {
            return Ok(());
        }
        if !snapshot.product.api_ok && snapshot.product.api_port_open {
            return Err(
                "已有服务占用 API 地址，但健康检查未通过。请查看该服务日志后重试；未重启现有进程。"
                    .into(),
            );
        }
        if !snapshot.stack.as_ref().is_some_and(|s| s.overall_ok) || !snapshot.env_exists {
            progress(Phase::StartingStack);
            let stack = desktop_core::ensure_local_stack(None, None)
                .await
                .map_err(|e| e.to_string())?;
            if !stack.ok {
                return Err(stack.message);
            }
        }
        progress(Phase::StartingProduct);
        let product = desktop_core::ensure_local_product()
            .await
            .map_err(|e| e.to_string())?;
        if product.ok {
            Ok(())
        } else {
            Err(product.message)
        }
    }
    pub async fn shutdown(&mut self) -> Result<(), String> {
        let mut lease = std::mem::take(&mut self.lease);
        let (returned, result) = tokio::task::spawn_blocking(move || {
            let result = lease.shutdown();
            (lease, result)
        })
        .await
        .map_err(|e| e.to_string())?;
        self.lease = returned;
        result
    }
}

#[derive(Default)]
pub struct ServiceViewState {
    pub snapshot: Option<Snapshot>,
    pub error: Option<String>,
    pub phase: Phase,
}
impl ServiceViewState {
    pub fn busy(&self) -> bool {
        self.phase.busy()
    }
}
