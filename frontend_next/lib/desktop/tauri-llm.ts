import { openInBrowser } from "@/lib/desktop/tauri-license";

export type LocalEmbeddingConfig = {
  provider: string;
  base_url: string;
  api_key: string;
  model: string;
};

export type LocalLlmConfig = {
  provider: string;
  base_url: string;
  api_key: string;
  model: string;
  timeout_ms?: number;
  embedding?: LocalEmbeddingConfig | null;
};

export type TestResult = {
  ok: boolean;
  latency_ms?: number;
  message: string;
};

export type DiagnosticStatus = "ok" | "warning" | "error";

export type DiagnosticCheck = {
  name: string;
  status: DiagnosticStatus;
  latency_ms?: number;
  message: string;
};

export type RepairAction =
  | { type: "OpenUrl"; url: string }
  | { type: "UpdateConfig"; patch: Record<string, unknown> }
  | { type: "RunCommand"; command: string }
  | { type: "ShowGuide"; guide_id: string };

export type RepairSuggestion = {
  code: string;
  message: string;
  action?: RepairAction | null;
};

export type DiagnosticReport = {
  overall: DiagnosticStatus;
  checks: DiagnosticCheck[];
  suggestions: RepairSuggestion[];
};

export type RepairActionResult = {
  applied: boolean;
  message: string;
};

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke: tauriInvoke } = await import("@tauri-apps/api/core");
  return tauriInvoke<T>(command, args);
}

export async function getLlmConfig(): Promise<LocalLlmConfig | null> {
  return invoke<LocalLlmConfig | null>("get_llm_config");
}

export async function setLlmConfig(config: LocalLlmConfig): Promise<void> {
  await invoke("set_llm_config", { config });
}

export async function testLlmConnection(config: LocalLlmConfig): Promise<TestResult> {
  return invoke<TestResult>("test_llm_connection", { config });
}

export async function diagnoseLlm(config: LocalLlmConfig): Promise<DiagnosticReport> {
  return invoke<DiagnosticReport>("diagnose_llm", { config });
}

export async function listAvailableModels(config: LocalLlmConfig): Promise<string[]> {
  return invoke<string[]>("list_available_models", { config });
}

export function mergeLlmConfigPatch(
  current: LocalLlmConfig | null,
  patch: Record<string, unknown>,
): LocalLlmConfig {
  const base: LocalLlmConfig = current ?? {
    provider: "custom",
    base_url: "",
    api_key: "",
    model: "",
    timeout_ms: 30_000,
  };

  return {
    ...base,
    ...patch,
    provider: typeof patch.provider === "string" ? patch.provider : base.provider,
    base_url: typeof patch.base_url === "string" ? patch.base_url : base.base_url,
    api_key: typeof patch.api_key === "string" ? patch.api_key : base.api_key,
    model: typeof patch.model === "string" ? patch.model : base.model,
    timeout_ms:
      typeof patch.timeout_ms === "number" ? patch.timeout_ms : (base.timeout_ms ?? 30_000),
  };
}

export async function executeRepairAction(
  action: RepairAction,
  options?: {
    currentConfig?: LocalLlmConfig | null;
    onConfigUpdated?: (config: LocalLlmConfig) => void;
  },
): Promise<RepairActionResult> {
  switch (action.type) {
    case "OpenUrl": {
      await openInBrowser(action.url);
      return { applied: true, message: "已在浏览器中打开链接" };
    }
    case "UpdateConfig": {
      const merged = mergeLlmConfigPatch(options?.currentConfig ?? null, action.patch);
      await setLlmConfig(merged);
      options?.onConfigUpdated?.(merged);
      return { applied: true, message: "配置已更新" };
    }
    case "RunCommand": {
      return {
        applied: false,
        message: `请手动执行命令：${action.command}`,
      };
    }
    case "ShowGuide": {
      return {
        applied: false,
        message: `请查看指南：${action.guide_id}`,
      };
    }
    default: {
      return { applied: false, message: "不支持的修复动作" };
    }
  }
}

export function repairActionLabel(action: RepairAction): string {
  switch (action.type) {
    case "OpenUrl":
      return "打开链接";
    case "UpdateConfig":
      if (typeof action.patch.model === "string") {
        return `使用 ${action.patch.model}`;
      }
      return "应用修复";
    case "RunCommand":
      return "查看启动说明";
    case "ShowGuide":
      return "查看教程";
    default:
      return "修复";
  }
}
