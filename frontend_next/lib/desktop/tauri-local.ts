/**
 * 桌面本机运行时 IPC（数据栈 / 产品进程 / 本机会话）。
 *
 * 与 `tauri-license` 分离：这里只装本机运行时探针与生命周期命令，
 * LLM 配置面已退役（BYOK 只走 `/settings?tab=providers`）。
 */

export type LocalStackServiceStatus = {
  id: string;
  label: string;
  endpoint: string;
  ok: boolean;
  detail: string;
};

export type DockerStatus = {
  cli_ok: boolean;
  daemon_ok: boolean;
  compose_ok: boolean;
  overall_ok: boolean;
  detail: string;
  install_url: string;
  install_hint: string;
  platform: string;
};

export type LocalStackStatus = {
  overall_ok: boolean;
  services: LocalStackServiceStatus[];
  compose_hint: string;
  script_path?: string | null;
  env_file_path?: string | null;
  env_file_exists?: boolean;
  docker?: DockerStatus | null;
};

export type ClientRuntimeConfig = {
  database_url: string;
  redis_url: string;
  /** Desktop default: `pgvector`. Cloud SaaS uses milvus. */
  retrieval_backend?: string;
  /** Legacy; unused on slim desktop stack. */
  milvus_url?: string;
  pg_host: string;
  pg_port: number;
  redis_host: string;
  redis_port: number;
  milvus_host?: string;
  milvus_port?: number;
  migrations_dir?: string | null;
  env_file_path?: string | null;
  env_file_exists: boolean;
  monorepo_root?: string | null;
  note: string;
};

export type EnsureLocalStackResult = {
  ok: boolean;
  message: string;
  stdout: string;
  stderr: string;
  status: LocalStackStatus;
  config: ClientRuntimeConfig;
};

function mapIpcError(error: unknown): Error {
  if (error instanceof Error) {
    return error;
  }
  if (typeof error === "string") {
    try {
      const parsed: unknown = JSON.parse(error);
      if (
        parsed &&
        typeof parsed === "object" &&
        "message" in parsed &&
        typeof (parsed as { message: unknown }).message === "string"
      ) {
        const rec = parsed as { status?: number; code?: string; message: string };
        return new Error(rec.message);
      }
    } catch {
      return new Error(error);
    }
    return new Error(error);
  }
  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message: unknown }).message;
    if (typeof message === "string" && message.trim()) {
      return new Error(message);
    }
  }
  return new Error(String(error));
}

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke: tauriInvoke } = await import("@tauri-apps/api/core");
  try {
    return await tauriInvoke<T>(command, args);
  } catch (error) {
    throw mapIpcError(error);
  }
}

export async function getLocalStackStatus(): Promise<LocalStackStatus> {
  return invoke<LocalStackStatus>("get_local_stack_status");
}

export async function getDockerStatus(): Promise<DockerStatus> {
  return invoke<DockerStatus>("get_docker_status");
}

export async function getClientRuntimeConfig(): Promise<ClientRuntimeConfig> {
  return invoke<ClientRuntimeConfig>("get_client_runtime_config");
}

export async function ensureLocalStack(): Promise<EnsureLocalStackResult> {
  return invoke<EnsureLocalStackResult>("ensure_local_stack");
}

export async function stopLocalStack(): Promise<EnsureLocalStackResult> {
  return invoke<EnsureLocalStackResult>("stop_local_stack");
}

export type LocalProductStatus = {
  overall_ok: boolean;
  api_ok: boolean;
  worker_ok: boolean;
  api_base_url: string;
  api_endpoint: string;
  health_detail: string;
  worker_detail: string;
  compose_hint: string;
  script_path?: string | null;
  log_dir?: string | null;
  api_bin_path?: string | null;
  worker_bin_path?: string | null;
};

export type EnsureLocalProductResult = {
  ok: boolean;
  message: string;
  stdout: string;
  stderr: string;
  status: LocalProductStatus;
};

export async function getLocalProductStatus(): Promise<LocalProductStatus> {
  return invoke<LocalProductStatus>("get_local_product_status");
}

export async function ensureLocalProduct(): Promise<EnsureLocalProductResult> {
  return invoke<EnsureLocalProductResult>("ensure_local_product");
}

export async function stopLocalProduct(): Promise<EnsureLocalProductResult> {
  return invoke<EnsureLocalProductResult>("stop_local_product");
}

/** Force-restart the local product so newly upserted provider secrets resolve at boot. */
export async function restartLocalProduct(): Promise<EnsureLocalProductResult> {
  return invoke<EnsureLocalProductResult>("restart_local_product");
}

export type LocalAuthUser = {
  id: string;
  email: string;
  full_name: string;
};

export type LocalSessionStatus = {
  ready: boolean;
  email: string;
  token?: string | null;
  user?: LocalAuthUser | null;
  message: string;
  api_base_url: string;
};

export async function getLocalSession(): Promise<LocalSessionStatus> {
  return invoke<LocalSessionStatus>("get_local_session");
}

export async function ensureLocalSession(): Promise<LocalSessionStatus> {
  return invoke<LocalSessionStatus>("ensure_local_session");
}
