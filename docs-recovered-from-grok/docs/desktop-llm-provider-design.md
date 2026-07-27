# 桌面端 LLM Provider 兼容性与诊断设计

> 日期：2026-07-08　状态：Proposed
> 关联 ADR：`docs/adr/0005-llm-provider-protocol-architecture.md`

---

## 1. 目标

桌面端用户自带 LLM API key（BYOK），需支持尽可能多的 LLM provider，并提供连接诊断能力。设计要求：

1. **广泛兼容**：覆盖主流 provider（OpenAI / Anthropic / DeepSeek / 智谱 / Gemini / Ollama 等 16+ 家）
2. **Coding Plan 接入**：智谱 GLM Coding Plan 等订阅制 provider 一键配置
3. **连接诊断**：给本地 agent 一个能诊断连接问题的接口和修复机制
4. **Provider 预设**：降低配置门槛，选 provider 即预填 base_url / model / 申请链接

底层 LLM 调用由 ADR 0004 的四轴架构提供（`avrag-llm` 重构后支持 3 种原生协议 + 13 种 OpenAI 兼容 profile）。本文聚焦桌面端的**配置层**和**诊断层**。

---

## 2. LLM 配置模型

### 2.1 本地配置结构

持久化到 `app_data_dir/llm-config.json`：

```rust
// desktop/src-tauri/src/commands/llm_config.rs

#[derive(Serialize, Deserialize, Clone)]
pub struct LocalLlmConfig {
    pub provider: String,              // "zhipu" | "openai" | "anthropic" | "deepseek" | ... | "custom"
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_ms: u64,
    pub enable_thinking: Option<bool>,
    pub enable_cache: Option<bool>,

    // Embedding（可选，独立配置；不配则桌面端用本地 BM25 检索回退）
    pub embedding: Option<LocalEmbeddingConfig>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LocalEmbeddingConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub dimensions: Option<usize>,
}
```

### 2.2 与 avrag-llm 的桥接

`LocalLlmConfig` 转为 `avrag_llm::ModelProviderConfig`（旧 API 兼容）或直接构造 `avrag_llm::providers::Provider`（新 API）：

```rust
impl LocalLlmConfig {
    /// 构造 avrag-llm Provider（四轴架构）
    pub fn to_provider(&self) -> avrag_llm::providers::Provider {
        match self.provider.as_str() {
            "anthropic" => avrag_llm::providers::anthropic::configure(
                self.api_key.clone(),
                Some(self.base_url.clone()),
            ),
            "google" | "gemini" => avrag_llm::providers::google::configure(
                self.api_key.clone(),
                Some(self.base_url.clone()),
            ),
            // OpenAI 兼容（含 deepseek/zhipu/siliconflow/groq/...）
            profile_id => {
                if let Some(profile) = avrag_llm::providers::openai_compatible::find_profile(profile_id) {
                    avrag_llm::providers::openai_compatible::configure(
                        profile, self.api_key.clone(), Some(self.base_url.clone()),
                    )
                } else {
                    // 自定义
                    avrag_llm::providers::openai_compatible::configure_generic(
                        self.api_key.clone(), self.base_url.clone(),
                    )
                }
            }
        }
    }
}
```

---

## 3. Provider 预设

前端 TS 定义，供配置引导页渲染选择列表：

```typescript
// frontend_next/lib/desktop/llm-presets.ts

export type LlmPreset = {
  id: string;
  label: string;
  description: string;
  base_url: string;
  model: string;
  api_key_url: string;       // 申请 key 的链接
  docs_url: string;
  pricing_note: string;
  auth_style: 'bearer' | 'x-api-key' | 'x-goog-api-key';
  protocol: 'openai-chat' | 'anthropic-messages' | 'gemini';
};

export const LLM_PRESETS: LlmPreset[] = [
  {
    id: 'zhipu',
    label: '智谱 GLM（含 Coding Plan）',
    description: '按月订阅 Coding Plan 无 token 计费，或按 token 付费',
    base_url: 'https://open.bigmodel.cn/api/paas/v4',
    model: 'glm-4.6',
    api_key_url: 'https://open.bigmodel.cn/console/apikey',
    docs_url: 'https://open.bigmodel.cn/dev/api',
    pricing_note: 'Coding Plan ¥20/月 · 或按 token',
    auth_style: 'bearer',
    protocol: 'openai-chat',
  },
  {
    id: 'anthropic',
    label: 'Anthropic Claude',
    description: 'Claude Sonnet/Opus，原生协议支持 prompt caching',
    base_url: 'https://api.anthropic.com/v1',
    model: 'claude-sonnet-4-20250514',
    api_key_url: 'https://console.anthropic.com/settings/keys',
    docs_url: 'https://docs.anthropic.com',
    pricing_note: '$3-15/百万 token',
    auth_style: 'x-api-key',
    protocol: 'anthropic-messages',
  },
  {
    id: 'deepseek',
    label: 'DeepSeek',
    description: '高性价比，支持 thinking 模式',
    base_url: 'https://api.deepseek.com',
    model: 'deepseek-chat',
    api_key_url: 'https://platform.deepseek.com/api_keys',
    docs_url: 'https://api-docs.deepseek.com',
    pricing_note: '¥1-8/百万 token',
    auth_style: 'bearer',
    protocol: 'openai-chat',
  },
  {
    id: 'openai',
    label: 'OpenAI',
    description: 'GPT-4o / o1 / o3 系列',
    base_url: 'https://api.openai.com/v1',
    model: 'gpt-4o',
    api_key_url: 'https://platform.openai.com/api-keys',
    docs_url: 'https://platform.openai.com/docs',
    pricing_note: '$2.50-15/百万 token',
    auth_style: 'bearer',
    protocol: 'openai-chat',
  },
  {
    id: 'google',
    label: 'Google Gemini',
    description: 'Gemini 2.0 Flash / Pro，原生协议',
    base_url: 'https://generativelanguage.googleapis.com/v1beta',
    model: 'gemini-2.0-flash',
    api_key_url: 'https://aistudio.google.com/apikey',
    docs_url: 'https://ai.google.dev/gemini-api/docs',
    pricing_note: '免费额度 · 或 $1.25-5/百万 token',
    auth_style: 'x-goog-api-key',
    protocol: 'gemini',
  },
  {
    id: 'siliconflow',
    label: 'SiliconFlow',
    description: '多模型聚合，含 Qwen / DeepSeek 等',
    base_url: 'https://api.siliconflow.cn/v1',
    model: 'Qwen/Qwen2.5-72B-Instruct',
    api_key_url: 'https://cloud.siliconflow.cn/account/ak',
    docs_url: 'https://docs.siliconflow.cn',
    pricing_note: '¥1-4/百万 token',
    auth_style: 'bearer',
    protocol: 'openai-chat',
  },
  {
    id: 'dashscope',
    label: '通义千问（DashScope）',
    description: '阿里云通义千问系列',
    base_url: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
    model: 'qwen-plus',
    api_key_url: 'https://dashscope.console.aliyun.com/apiKey',
    docs_url: 'https://help.aliyun.com/zh/dashscope',
    pricing_note: '¥0.8-4/百万 token',
    auth_style: 'bearer',
    protocol: 'openai-chat',
  },
  {
    id: 'groq',
    label: 'Groq',
    description: '超低延迟推理，Llama 系列',
    base_url: 'https://api.groq.com/openai/v1',
    model: 'llama-3.3-70b-versatile',
    api_key_url: 'https://console.groq.com/keys',
    docs_url: 'https://console.groq.com/docs',
    pricing_note: '免费额度 · 或 $0.59-0.79/百万 token',
    auth_style: 'bearer',
    protocol: 'openai-chat',
  },
  {
    id: 'ollama',
    label: '本地 Ollama',
    description: '完全离线，无需 API key',
    base_url: 'http://localhost:11434/v1',
    model: 'llama3.2',
    api_key_url: '',
    docs_url: 'https://ollama.com',
    pricing_note: '免费（本地运行）',
    auth_style: 'bearer',
    protocol: 'openai-chat',
  },
  {
    id: 'openrouter',
    label: 'OpenRouter',
    description: '聚合 100+ 模型，统一计费',
    base_url: 'https://openrouter.ai/api/v1',
    model: 'anthropic/claude-3.5-sonnet',
    api_key_url: 'https://openrouter.ai/keys',
    docs_url: 'https://openrouter.ai/docs',
    pricing_note: '按模型不同',
    auth_style: 'bearer',
    protocol: 'openai-chat',
  },
  {
    id: 'togetherai',
    label: 'Together AI',
    description: '开源模型托管',
    base_url: 'https://api.together.xyz/v1',
    model: 'meta-llama/Llama-3-70b-chat-hf',
    api_key_url: 'https://api.together.ai/settings/api-keys',
    docs_url: 'https://docs.together.ai',
    pricing_note: '$0.20-5/百万 token',
    auth_style: 'bearer',
    protocol: 'openai-chat',
  },
  {
    id: 'cerebras',
    label: 'Cerebras',
    description: '超快推理速度',
    base_url: 'https://api.cerebras.ai/v1',
    model: 'llama3.1-8b',
    api_key_url: 'https://cloud.cerebras.ai',
    docs_url: 'https://inference-docs.cerebras.ai',
    pricing_note: '$0.10-1/百万 token',
    auth_style: 'bearer',
    protocol: 'openai-chat',
  },
  {
    id: 'fireworks',
    label: 'Fireworks AI',
    description: '开源模型高速推理',
    base_url: 'https://api.fireworks.ai/inference/v1',
    model: 'accounts/fireworks/models/llama-v3p1-70b-instruct',
    api_key_url: 'https://fireworks.ai/account/api-keys',
    docs_url: 'https://docs.fireworks.ai',
    pricing_note: '$0.20-3/百万 token',
    auth_style: 'bearer',
    protocol: 'openai-chat',
  },
  {
    id: 'xai',
    label: 'xAI Grok',
    description: 'Grok 系列模型',
    base_url: 'https://api.x.ai/v1',
    model: 'grok-2',
    api_key_url: 'https://console.x.ai',
    docs_url: 'https://docs.x.ai',
    pricing_note: '$2-10/百万 token',
    auth_style: 'bearer',
    protocol: 'openai-chat',
  },
  {
    id: 'deepinfra',
    label: 'DeepInfra',
    description: '开源模型推理',
    base_url: 'https://api.deepinfra.com/v1/openai',
    model: 'meta-llama/Meta-Llama-3.1-70B-Instruct',
    api_key_url: 'https://deepinfra.com/dash/api_keys',
    docs_url: 'https://deepinfra.com/docs',
    pricing_note: '$0.20-3/百万 token',
    auth_style: 'bearer',
    protocol: 'openai-chat',
  },
  {
    id: 'custom',
    label: '自定义 API',
    description: '任何 OpenAI / Anthropic 兼容端点',
    base_url: '',
    model: '',
    api_key_url: '',
    docs_url: '',
    pricing_note: '',
    auth_style: 'bearer',
    protocol: 'openai-chat',
  },
];
```

### 3.1 Coding Plan 接入说明

智谱 GLM Coding Plan 是按月订阅（非按 token），对桌面端用户非常友好。接入方式和普通 OpenAI 兼容 API 完全一致：

```
provider: zhipu
base_url: https://open.bigmodel.cn/api/paas/v4
api_key: <用户在智谱官网获取的 key>
model: glm-4.6
```

智谱 API 兼容 OpenAI 格式（`/chat/completions`），`avrag-llm` 的 OpenAI Chat 协议直接覆盖。预设模板一键填充配置，用户只需粘贴 API key。

---

## 4. 连接诊断接口

### 4.1 设计目标

给本地 agent 一个结构化的诊断工具，能：
- 逐层排查连接问题（DNS → TCP → TLS → 鉴权 → 模型可用 → Embedding）
- 返回可执行的修复建议（打开申请页 / 自动修正配置 / 启动本地服务）
- 前端一键执行修复

### 4.2 Tauri Command

```rust
// desktop/src-tauri/src/commands/llm_config.rs

#[derive(Serialize)]
pub struct DiagnosticReport {
    pub overall: DiagnosticStatus,       // ok | warning | error
    pub checks: Vec<DiagnosticCheck>,
    pub suggestions: Vec<RepairSuggestion>,
}

#[derive(Serialize)]
pub struct DiagnosticCheck {
    pub name: String,                    // "dns" | "tcp_connect" | "tls" | "auth" | "model_available" | "embedding"
    pub status: DiagnosticStatus,        // ok | warning | error
    pub latency_ms: Option<u64>,
    pub message: String,
}

#[derive(Serialize)]
pub struct RepairSuggestion {
    pub code: String,                    // "renew_api_key" | "switch_model" | "start_ollama" | ...
    pub message: String,
    pub action: Option<RepairAction>,    // 可执行的修复动作
}

#[derive(Serialize)]
#[serde(tag = "type")]
pub enum RepairAction {
    OpenUrl { url: String },                      // 打开 API key 申请页
    UpdateConfig { patch: serde_json::Value },     // 自动修正配置（如切换 model）
    RunCommand { command: String },                // 如启动 ollama
    ShowGuide { guide_id: String },                // 显示图文教程
}

/// 诊断 LLM 连接（6 步检查，返回结构化报告 + 修复建议）
#[tauri::command]
async fn diagnose_llm(config: LocalLlmConfig) -> Result<DiagnosticReport, String>
```

### 4.3 诊断流程（6 步）

```
┌──────────────────────────────────────────────────────────────┐
│ Step 1: DNS 解析                                              │
│   解析 base_url 的 host → IP 列表                             │
│   失败 → 建议检查 VPN / DNS                                   │
├──────────────────────────────────────────────────────────────┤
│ Step 2: TCP 连接（端口可达性）                                  │
│   tokio::net::TcpStream::connect(host, port)，5s 超时         │
│   失败 → localhost 建议启动 ollama；远程建议检查网络/代理        │
├──────────────────────────────────────────────────────────────┤
│ Step 3: 鉴权（API key 有效性）                                  │
│   发一个最小 chat completion 测试请求（max_tokens=1）           │
│   401 → API key 无效 → 建议打开申请页                          │
│   403 → 无权限 → 建议检查 plan/model                           │
├──────────────────────────────────────────────────────────────┤
│ Step 4: 模型可用性                                             │
│   测试请求返回的 model 字段是否匹配                             │
│   404 / model_not_found → 建议切换 model                       │
│   调 GET /models 列出可用模型                                  │
├──────────────────────────────────────────────────────────────┤
│ Step 5: Embedding 连通性（如果配了 embedding）                  │
│   测试 embed 一个 "test" 文本                                  │
│   失败 → 建议用本地 BM25 回退                                  │
├──────────────────────────────────────────────────────────────┤
│ Step 6: Coding Plan 配额（智谱专用，可选）                      │
│   智谱无公开配额 API → 显示订阅说明                             │
└──────────────────────────────────────────────────────────────┘
```

### 4.4 诊断实现要点

```rust
async fn diagnose_llm(config: LocalLlmConfig) -> Result<DiagnosticReport, String> {
    let mut checks = Vec::new();
    let mut suggestions = Vec::new();

    let url = url::Url::parse(&config.base_url).map_err(|e| format!("Base URL 无效: {e}"))?;
    let host = url.host_str().ok_or("Base URL 缺少 host")?;
    let port = url.port_or_known_default().unwrap_or(443);

    // Step 1: DNS
    match tokio::net::lookup_host((host, port)).await {
        Ok(_) => checks.push(DiagnosticCheck::ok("dns", "DNS 解析成功")),
        Err(e) => {
            checks.push(DiagnosticCheck::error("dns", format!("DNS 解析失败: {e}")));
            suggestions.push(RepairSuggestion::new("check_dns",
                "请检查网络连接或 DNS 配置。如使用 VPN，请确保 DNS 正常解析。"));
        }
    }

    // Step 2: TCP
    match tokio::time::timeout(
        Duration::from_secs(5),
        tokio::net::TcpStream::connect((host, port)),
    ).await {
        Ok(Ok(_)) => checks.push(DiagnosticCheck::ok("tcp_connect", "TCP 连接成功")),
        Ok(Err(e)) => {
            checks.push(DiagnosticCheck::error("tcp_connect", format!("无法连接: {e}")));
            if host == "localhost" || host == "127.0.0.1" {
                suggestions.push(RepairSuggestion::new("start_ollama",
                    "本地服务不可达。请确认 Ollama 已启动。")
                    .with_action(RepairAction::RunCommand("ollama serve".into())));
            } else {
                suggestions.push(RepairSuggestion::new("check_network",
                    "无法连接服务器。请检查网络或代理设置。"));
            }
        }
        Err(_) => {
            checks.push(DiagnosticCheck::error("tcp_connect", "连接超时（5s）"));
            suggestions.push(RepairSuggestion::new("check_network",
                "连接超时。请检查网络或代理设置。"));
        }
    }

    // Step 3+4: 鉴权 + 模型可用性（发一个最小测试请求）
    let provider = config.to_provider();
    let test_request = LlmRequest {
        model: ModelRef { id: config.model.clone(), provider: config.provider.clone() },
        system: vec![],
        messages: vec![Message::user("ping")],
        tools: vec![],
        generation: GenerationOptions { max_tokens: Some(1), ..Default::default() },
        ..Default::default()
    };

    match provider.route.generate(test_request).await {
        Ok(resp) => {
            checks.push(DiagnosticCheck::ok("auth", "API key 有效"));
            checks.push(DiagnosticCheck::ok("model_available",
                format!("模型 {} 可用", resp.model())));
        }
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("401") {
                checks.push(DiagnosticCheck::error("auth", "API key 无效或已过期"));
                suggestions.push(RepairSuggestion::new("renew_api_key",
                    "API key 无效，请重新获取。")
                    .with_action(RepairAction::OpenUrl {
                        url: api_key_url(&config.provider).to_string(),
                    }));
            } else if err_str.contains("403") {
                checks.push(DiagnosticCheck::error("auth", "无权限访问此模型"));
                suggestions.push(RepairSuggestion::new("check_plan",
                    "当前账号无权使用此模型，请检查订阅计划或更换模型。"));
            } else if err_str.contains("404") || err_str.contains("model_not_found") {
                checks.push(DiagnosticCheck::error("model_available",
                    format!("模型 \"{}\" 不存在", config.model)));
                // 尝试列出可用模型
                if let Ok(models) = list_models(&config).await {
                    suggestions.push(RepairSuggestion::new("switch_model",
                        format!("可用模型: {}", models.join(", ")))
                        .with_action(RepairAction::UpdateConfig(
                            serde_json::json!({ "model": models.first() }))));
                }
            } else if err_str.contains("timeout") {
                checks.push(DiagnosticCheck::warning("timeout", "请求超时"));
                suggestions.push(RepairSuggestion::new("increase_timeout",
                    "请求超时，可尝试增加超时时间。")
                    .with_action(RepairAction::UpdateConfig(
                        serde_json::json!({ "timeout_ms": 60000 }))));
            } else {
                checks.push(DiagnosticCheck::error("request",
                    format!("请求失败: {err_str}")));
            }
        }
    }

    // Step 5: Embedding
    if let Some(emb) = &config.embedding {
        match test_embedding(emb).await {
            Ok(_) => checks.push(DiagnosticCheck::ok("embedding", "Embedding 连接正常")),
            Err(e) => {
                checks.push(DiagnosticCheck::warning("embedding",
                    format!("Embedding 异常: {e}")));
                suggestions.push(RepairSuggestion::new("use_local_bm25",
                    "Embedding 不可用，将使用本地 BM25 检索作为回退。"));
            }
        }
    }

    let overall = if checks.iter().any(|c| c.status == "error") { "error" }
                  else if checks.iter().any(|c| c.status == "warning") { "warning" }
                  else { "ok" };

    Ok(DiagnosticReport { overall, checks, suggestions })
}

fn api_key_url(provider: &str) -> &str {
    match provider {
        "zhipu" => "https://open.bigmodel.cn/console/apikey",
        "anthropic" => "https://console.anthropic.com/settings/keys",
        "openai" => "https://platform.openai.com/api-keys",
        "deepseek" => "https://platform.deepseek.com/api_keys",
        "google" | "gemini" => "https://aistudio.google.com/apikey",
        "siliconflow" => "https://cloud.siliconflow.cn/account/ak",
        "dashscope" => "https://dashscope.console.aliyun.com/apiKey",
        "groq" => "https://console.groq.com/keys",
        "openrouter" => "https://openrouter.ai/keys",
        _ => "",
    }
}
```

### 4.5 Tauri Commands 汇总

```rust
#[tauri::command]
async fn get_llm_config(app: AppHandle) -> Result<Option<LocalLlmConfig>, String>

#[tauri::command]
async fn set_llm_config(app: AppHandle, config: LocalLlmConfig) -> Result<(), String>

#[tauri::command]
async fn test_llm_connection(config: LocalLlmConfig) -> Result<TestResult, String>
/// 简单连通性测试（诊断的轻量版，只做 Step 3+4）

#[tauri::command]
async fn diagnose_llm(config: LocalLlmConfig) -> Result<DiagnosticReport, String>
/// 完整 6 步诊断

#[tauri::command]
async fn list_available_models(config: LocalLlmConfig) -> Result<Vec<String>, String>
/// 调 GET /models 列出可用模型（OpenAI 兼容端点支持）
```

---

## 5. 诊断 UI 示例

设置页 LLM 配置 tab 的诊断面板（详细 UI 布局见 `docs/desktop-frontend-pages-design.md`）：

```
┌─────────────────────────────────────────────┐
│ ⚙ LLM 配置                          [诊断]  │
├─────────────────────────────────────────────┤
│ Provider: [智谱 GLM ▼]                      │
│ Base URL: https://open.bigmodel.cn/...      │
│ API Key:  ••••••••••••••••          [显示]  │
│ Model:    glm-4.6                            │
├─────────────────────────────────────────────┤
│ 诊断报告:                                    │
│  ✓ DNS 解析          (12ms)                  │
│  ✓ TCP 连接          (45ms)                  │
│  ✓ 鉴权              API key 有效             │
│  ✗ 模型可用          "glm-4.6" 不存在         │
│    ┌──────────────────────────────────────┐ │
│    │ 可用模型: glm-4-plus, glm-4-flash... │ │
│    │ [使用 glm-4-plus]  ← 一键修复         │ │
│    └──────────────────────────────────────┘ │
│  ✓ Embedding        连接正常                 │
└─────────────────────────────────────────────┘
```

修复动作（`RepairAction`）由前端根据 `action.type` 执行：
- `OpenUrl` → 调 `invoke("open_in_browser", { url })`
- `UpdateConfig` → 合并 patch 到配置，调 `set_llm_config`
- `RunCommand` → 提示用户手动执行（安全考虑不自动执行）
- `ShowGuide` → 渲染对应 guide_id 的图文教程

---

## 6. 关联

- `docs/adr/0005-llm-provider-protocol-architecture.md` — 四轴架构（底层 LLM 调用）
- `docs/desktop-frontend-pages-design.md` — 前端诊断 UI 布局
- `docs/desktop-execution-plan.md` — 总执行计划（WP4 覆盖本设计）