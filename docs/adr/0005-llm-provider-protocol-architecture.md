# ADR 0004: LLM Provider 四轴架构——Protocol / Route / Provider 正交

## Status

Proposed

## Context

### 现有架构

`avrag-llm` crate 当前的 `LlmClient`（`crates/llm/src/client/mod.rs`，435 行）是一个**单层结构**：

- 固定走 OpenAI `/chat/completions` 协议
- 鉴权硬编码为 `Authorization: Bearer {api_key}`（`mod.rs:150`）
- provider 差异通过 `base_url` 字符串匹配处理（`request.rs:46-83` 的 `enable_thinking`、`response_format`、`prompt_cache` 分支）
- 流式解析固定为 SSE（`stream_parser.rs`）

已支持的 provider（全部走 OpenAI 兼容格式）：OpenAI、DeepSeek、SiliconFlow、智谱、通义（DashScope）、Ollama。

### 缺失

1. **Anthropic 原生协议**：当前只能用 Anthropic 的 OpenAI 兼容端点，无法用 `prompt caching` 原生格式、`extended thinking`、`cache_control` 等 Anthropic 特有能力。
2. **Gemini 原生协议**：同理，只能用 OpenAI 兼容端点，丢失 `generateContent` 原生格式。
3. **自定义鉴权**：Anthropic 用 `x-api-key` header，Gemini 用 `x-goog-api-key`，AWS Bedrock 用 SigV4 签名——当前 `Bearer` 硬编码无法适配。
4. **架构耦合**：Protocol（说什么协议）和 Provider（发给谁）耦合在一个 `LlmClient` 里，加新 provider 需要改 `LlmClient` 内部。

### 参考架构

opencode（`sst/opencode`）的 `packages/llm` 采用**四轴正交设计**，支持 20+ provider：

```
四轴：Protocol（协议）→ Endpoint（端点）→ Auth（鉴权）→ Framing（帧解码）

protocols/           providers/
├── openai-chat.ts        506行   ├── openai.ts
├── openai-compatible.ts    24行   ├── anthropic.ts       (x-api-key)
├── anthropic-messages.ts  855行   ├── google.ts          (x-goog-api-key)
├── gemini.ts             512行   ├── amazon-bedrock.ts   (SigV4)
└── bedrock-converse.ts   674行   ├── azure.ts           (apiVersion)
                                   ├── github-copilot.ts  (OAuth)
                                   ├── openrouter.ts
                                   └── openai-compatible-profile.ts (9个预设，一行加新provider)
```

核心设计信念：**API 契约（说什么协议）与请求目标（发给谁）正交。** 一个 `OpenAIChat` 协议被 7 个 provider 复用，每个 provider 只贡献鉴权和端点 URL。

## Decision

**将 opencode 的四轴架构移植到 Rust，重构 `avrag-llm` crate。**

保留旧 `LlmClient` API 作为 thin wrapper，SaaS 侧 ~15 个调用点（`app-chat/`）无需改动。

### 架构映射：opencode TypeScript → Rust

| opencode (TS) | avrag-llm (Rust) | 对应物 |
|---------------|-------------------|--------|
| `Effect<T, E>` | `async fn() -> Result<T, E>` | Rust 异步 |
| `Stream<LLMEvent>` | `impl Stream<Item = Result<LlmEvent, LlmError>>` | futures::Stream |
| `Schema.Codec<Body>` | serde Serialize/Deserialize | serde |
| `Protocol<Body, Frame, Event, State>` | `trait Protocol { type Body; type State; }` | trait + 关联类型 |
| `Route<Body, Prepared>` | `struct Route<P: Protocol>` | 泛型结构体 |
| `route.with(patch)` 不可变 | `route.with(patch) -> Self` (Clone) | builder 模式 |
| `Auth.orElse(that)` | `Auth::or_else(self, fallback) -> Self` | 组合 |

### 目标 Crate 结构

```
avrag-rs/crates/llm/
├── src/
│   ├── lib.rs                          # pub use 重导出
│   ├── schema/                          # 规范类型层（协议无关）
│   │   ├── mod.rs
│   │   ├── messages.rs                  # LlmRequest / Message / ContentPart / MessageRole
│   │   ├── events.rs                    # LlmEvent（16 种事件变体）/ Usage / FinishReason / LlmResponse
│   │   ├── options.rs                   # GenerationOptions / ModelLimits / ToolDefinition / ToolChoice
│   │   └── errors.rs                    # LlmError
│   ├── route/                           # 四轴路由层
│   │   ├── mod.rs
│   │   ├── client.rs                    # Route<P> + stream()/generate() + AnyRoute 类型擦除
│   │   ├── endpoint.rs                  # URL 构造 + merge
│   │   ├── auth.rs                      # Auth enum: None/Bearer/XApiKey/XGoogApiKey/Custom/SigV4
│   │   ├── framing.rs                   # Framing: Sse / (未来 AwsEventStream)
│   │   └── transport.rs                 # reqwest HTTP 执行
│   ├── protocols/                       # 协议实现（API 契约）
│   │   ├── mod.rs                       # Protocol trait 定义
│   │   ├── openai_chat.rs               # OpenAI Chat Completions（从现有 request.rs + stream_parser.rs 迁移）
│   │   ├── anthropic_messages.rs        # Anthropic Messages API（原生，~300 行新增）
│   │   └── gemini.rs                    # Gemini generateContent（原生，~250 行新增）
│   ├── providers/                       # Provider 配置层（部署配置）
│   │   ├── mod.rs                       # Provider struct + configure() 模式
│   │   ├── openai.rs
│   │   ├── anthropic.rs                 # x-api-key 鉴权
│   │   ├── google.rs                    # x-goog-api-key 鉴权
│   │   └── openai_compatible.rs         # 泛型 + profiles 注册表（12 个预设）
│   ├── client/                          # 兼容层（旧 API）
│   │   ├── mod.rs                       # LlmClient → thin wrapper 调新 Route
│   │   ├── types.rs                     # ChatMessage/LlmResponse（schema 类型的 alias）
│   │   ├── request.rs                   # 旧 body 构建（迁移到 protocols/openai_chat.rs 后删除）
│   │   └── stream_parser.rs             # 旧 SSE 解析（迁移到 protocols/openai_chat.rs 后删除）
│   ├── provider_profiles.rs            # Provider 元数据（诊断 UI 用）
│   ├── embedding.rs                     # 现有，保留
│   ├── reranker.rs                      # 现有，保留
│   ├── planner.rs                       # 现有，保留
│   ├── rate_limiter.rs                  # 现有，保留
│   ├── synthesizer.rs                   # 现有，保留
│   ├── summary.rs                       # 现有，保留
│   └── token_counter.rs                 # 现有，保留
└── Cargo.toml                           # 加 async-stream, futures 依赖
```

### 各层核心设计

#### Layer 1: `schema/` — 规范类型（协议无关）

所有协议和 provider 通过这些类型交互。这是整个系统的公共契约。

**`Message` 和 `ContentPart`**：

规范化消息用 `content: Vec<ContentPart>` 而非 `content: String`，因为 Anthropic 的 content blocks 和 Gemini 的 parts 都是多块结构，OpenAI 的纯字符串只是 `Vec<TextPart>` 的退化。

```rust
pub enum ContentPart {
    Text { text: String },
    Media { media_type: String, data: String },
    ToolCall { id: String, name: String, input: serde_json::Value },
    ToolResult { id: String, name: String, result: ToolResultValue },
    Reasoning { text: String },
}
```

**`LlmEvent`**（16 种流式事件变体，对应 opencode）：

```rust
pub enum LlmEvent {
    StepStart { index: usize },
    TextStart { id: String },
    TextDelta { id: String, text: String },
    TextEnd { id: String },
    ReasoningStart { id: String },
    ReasoningDelta { id: String, text: String },
    ReasoningEnd { id: String },
    ToolInputStart { id: String, name: String },
    ToolInputDelta { id: String, name: String, text: String },
    ToolInputEnd { id: String, name: String },
    ToolCall { id: String, name: String, input: serde_json::Value },
    ToolResult { id: String, name: String, result: ToolResultValue },
    ToolError { id: String, name: String, message: String },
    StepFinish { index: usize, reason: FinishReason, usage: Option<Usage> },
    Finish { reason: FinishReason, usage: Option<Usage> },
    ProviderError { message: String, retryable: Option<bool> },
}
```

**`LlmResponse`**：从事件流组装，提供 `.text()` / `.reasoning()` / `.tool_calls()` 便捷方法。

#### Layer 2: `route/` — 四轴路由

**`Auth` 枚举**——可组合的鉴权策略：

```rust
pub enum Auth {
    None,
    Bearer(String),              // Authorization: Bearer {key}（OpenAI/DeepSeek/智谱/SiliconFlow）
    XApiKey(String),             // x-api-key: {key}（Anthropic）
    XGoogApiKey(String),          // x-goog-api-key: {key}（Google Gemini）
    Custom(String, String),       // 自定义 header（Azure、特殊代理）
    SigV4(SigV4Config),           // AWS Bedrock（延后实现）
}
```

**`Endpoint`**——URL 构造：

```rust
pub struct Endpoint {
    pub base_url: Option<String>,
    pub path: String,             // "/chat/completions" or "/messages"
    pub query: Vec<(String, String)>,
}
```

**`Framing`**——流帧解码：

```rust
pub enum Framing {
    Sse,                          // SSE text frames（所有 JSON+SSE provider）
    // AwsEventStream,            // AWS binary event-stream（Bedrock，延后）
}
```

**`Route<P: Protocol>`**——四轴组合：

```rust
pub struct Route<P: Protocol> {
    pub id: String,
    pub provider: String,
    pub protocol: P,
    pub endpoint: Endpoint,
    pub auth: Auth,
    pub framing: Framing,
    pub http_client: reqwest::Client,
}
```

`Route<P>` 提供 `stream()` 和 `generate()` 方法，内部管线为：
1. 协议构造 body（`protocol.build_body(&request)`）
2. 端点渲染 URL（`endpoint.render()`）
3. 鉴权注入 header（`auth.apply(&mut headers)`）
4. HTTP 发送（`http_client.post(...).json(&body).send()`）
5. 帧解码（`framing.frames(response)`）
6. 协议解码 + 状态机（`protocol.decode_frame()` → `protocol.step()`）
7. 产出 `LlmEvent` 流

#### Layer 3: `protocols/` — 协议实现

```rust
pub trait Protocol: Clone + Send + Sync + 'static {
    type Body: Serialize + Send;
    type State: Default + Send;

    fn protocol_id(&self) -> &'static str;
    fn build_body(&self, req: &LlmRequest) -> Result<Self::Body, LlmError>;
    fn initial_state(&self, req: &LlmRequest) -> Self::State;
    fn decode_frame(&self, frame: String) -> Result<serde_json::Value, LlmError>;
    fn step(&self, state: &mut Self::State, event: &serde_json::Value) -> Result<Vec<LlmEvent>, LlmError>;
    fn on_halt(&self, state: &Self::State) -> Vec<LlmEvent> { Vec::new() }
}
```

**三个协议实现**：

| 协议 | 线格式 | Body | Framing | 鉴权 | 工程量 |
|------|--------|------|---------|------|--------|
| OpenAI Chat | JSON → SSE | `/chat/completions` body | SSE | Bearer | 从现有 `request.rs` + `stream_parser.rs` 迁移 |
| Anthropic Messages | JSON → SSE | `/messages` body（content blocks） | SSE | x-api-key | 新增 ~300 行 |
| Gemini | JSON → SSE | `generateContent` body（contents/parts） | SSE | x-goog-api-key | 新增 ~250 行 |

**OpenAI Chat 协议**复用现有逻辑：`request.rs::build_chat_completion_request_body` 迁移为 `OpenAiChatProtocol::build_body()`；`stream_parser.rs::ChatCompletionStreamParser` 迁移为 `OpenAiChatProtocol::step()` + `OpenAiChatState`。

**Anthropic Messages 协议**（核心新增）：
- Body 构建：`Message` → Anthropic content blocks（text/image/thinking/tool_use/tool_result）
- system prompt → 顶层 `system` 字段（非 message）
- 流式事件：`message_start` / `content_block_start` / `content_block_delta` / `content_block_stop` / `message_delta` / `message_stop`
- `cache_control` 原生支持（Anthropic prompt caching）

#### Layer 4: `providers/` — Provider 配置层

每个 provider 导出一个 `configure()` 函数，返回 facade：

```rust
// providers/openai.rs
pub fn configure(api_key: String, base_url: Option<String>) -> Provider {
    let route = Route {
        protocol: OpenAiChatProtocol::default(),
        auth: Auth::Bearer(api_key),
        endpoint: Endpoint { base_url: Some(...), path: "/chat/completions".into(), .. },
        ..
    };
    Provider { id: "openai".into(), route: Arc::new(route) }
}
```

**OpenAI 兼容 profile 注册表**（一行加新 provider）：

```rust
// providers/openai_compatible.rs
pub struct Profile {
    pub id: &'static str,
    pub base_url: &'static str,
    pub default_model: &'static str,
}

pub const PROFILES: &[Profile] = &[
    Profile { id: "deepseek",    base_url: "https://api.deepseek.com",                    default_model: "deepseek-chat" },
    Profile { id: "zhipu",       base_url: "https://open.bigmodel.cn/api/paas/v4",        default_model: "glm-4.6" },
    Profile { id: "siliconflow", base_url: "https://api.siliconflow.cn/v1",              default_model: "Qwen/Qwen2.5-72B-Instruct" },
    Profile { id: "dashscope",   base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1", default_model: "qwen-plus" },
    Profile { id: "groq",        base_url: "https://api.groq.com/openai/v1",               default_model: "llama-3.3-70b-versatile" },
    Profile { id: "cerebras",    base_url: "https://api.cerebras.ai/v1",                   default_model: "llama3.1-8b" },
    Profile { id: "togetherai",  base_url: "https://api.together.xyz/v1",                 default_model: "meta-llama/Llama-3-70b-chat-hf" },
    Profile { id: "fireworks",   base_url: "https://api.fireworks.ai/inference/v1",       default_model: "accounts/fireworks/models/llama-v3p1-70b-instruct" },
    Profile { id: "openrouter",  base_url: "https://openrouter.ai/api/v1",               default_model: "anthropic/claude-3.5-sonnet" },
    Profile { id: "xai",         base_url: "https://api.x.ai/v1",                         default_model: "grok-2" },
    Profile { id: "deepinfra",   base_url: "https://api.deepinfra.com/v1/openai",        default_model: "meta-llama/Meta-Llama-3.1-70B-Instruct" },
    Profile { id: "ollama",      base_url: "http://localhost:11434/v1",                  default_model: "llama3.2" },
];
```

#### Layer 5: 兼容层 — 旧 `LlmClient` thin wrapper

保留旧 API（`complete` / `complete_stream` / `complete_with_tools` / `complete_json_mode`），内部构造 `LlmRequest` 并委托给新 `Route`：

```rust
impl LlmClient {
    pub fn new(config: ModelProviderConfig) -> Self {
        let route = build_route_from_config(&config);  // 检测 base_url → 选协议+鉴权
        Self { route: Arc::new(route), config }
    }
    pub async fn complete(&self, messages: &[ChatMessage], temp: Option<f32>) -> Result<LlmResponse, LlmError> {
        let request = build_llm_request(messages, temp, &self.config);
        self.route.generate(request).await.map(LlmResponse::into)
    }
}
```

`build_route_from_config` 根据 `base_url` 检测协议：
- `anthropic.com` → Anthropic Messages + x-api-key
- `googleapis.com`（非 openai）→ Gemini + x-goog-api-key
- 其他 → OpenAI Chat + Bearer

### SaaS 侧影响面

`LlmClient` 在 `app-chat/` 有约 15 个调用点（`synthesis.rs`、`iteration/assemble.rs`、`run_result.rs`、`unified/mod.rs` 等）。全部使用旧 API 签名（`complete` / `complete_stream` / `complete_with_tools` / `complete_json_mode`），**wrapper 保证签名不变，调用点零改动**。

旧 `ChatMessage` / `LlmResponse` / `LlmUsage` 成为 `schema` 模块类型的 alias，现有代码无需改类型。

### 工程量

| 工作包 | 内容 | 工时 |
|--------|------|------|
| schema 层 | messages.rs + events.rs + errors.rs + options.rs | 1.5d |
| route 层 | auth.rs + endpoint.rs + framing.rs + client.rs + transport.rs | 2d |
| OpenAI Chat 协议 | 从现有 request.rs + stream_parser.rs 迁移 | 1.5d |
| Anthropic Messages 协议 | 新写（body 构建 + SSE 解析） | 2d |
| Gemini 协议 | 新写 | 1.5d |
| Provider 配置层 | 12 个 profile + 3 个原生 provider | 1d |
| LlmClient 兼容 wrapper | 保留旧 API，内部转发 | 1d |
| 测试 | 单元测试 + 集成测试 | 2d |
| **合计** | | **12.5d** |

### 支持的 Provider 总览

| Provider | 接入方式 | 协议 | 鉴权 |
|----------|---------|------|------|
| OpenAI | 原生 | OpenAI Chat | Bearer |
| Anthropic | 原生 | Anthropic Messages | x-api-key |
| Google Gemini | 原生 | Gemini | x-goog-api-key |
| DeepSeek | OpenAI 兼容 | OpenAI Chat | Bearer |
| 智谱 GLM（含 Coding Plan） | OpenAI 兼容 | OpenAI Chat | Bearer |
| SiliconFlow | OpenAI 兼容 | OpenAI Chat | Bearer |
| 通义千问（DashScope） | OpenAI 兼容 | OpenAI Chat | Bearer |
| Ollama | OpenAI 兼容 | OpenAI Chat | Bearer |
| Groq | OpenAI 兼容 + profile | OpenAI Chat | Bearer |
| Cerebras | OpenAI 兼容 + profile | OpenAI Chat | Bearer |
| TogetherAI | OpenAI 兼容 + profile | OpenAI Chat | Bearer |
| Fireworks | OpenAI 兼容 + profile | OpenAI Chat | Bearer |
| OpenRouter | OpenAI 兼容 + profile | OpenAI Chat | Bearer |
| XAI | OpenAI 兼容 + profile | OpenAI Chat | Bearer |
| DeepInfra | OpenAI 兼容 + profile | OpenAI Chat | Bearer |
| 自定义 | OpenAI 兼容（泛型） | OpenAI Chat | Bearer |

共 16 个 provider，其中 3 个原生协议，13 个 OpenAI 兼容。

## Consequences

### 正面

- **Anthropic 原生能力**：prompt caching 原生格式、extended thinking、content blocks——不再依赖兼容端点。
- **架构正交**：加新 provider 只需加 profile（1 行），不再改 `LlmClient` 内部。
- **协议可扩展**：未来加 Bedrock Converse（SigV4 + binary event-stream）只需加新 Protocol + Provider，不影响现有。
- **SaaS 侧零改动**：wrapper 兼容，15 个调用点无需动。
- **桌面端统一**：SaaS 和 Desktop 共用同一套 `avrag-llm`，provider 覆盖面一致。

### 负面

- **工程量大**：12.5 天，是增量方案（3-4 天）的 3-4 倍。
- **重构风险**：`avrag-llm` 被 SaaS 后端依赖，重构期间需保证测试不回退。
- **类型擦除开销**：`AnyRoute` trait object 有动态分发成本，但 LLM 调用本身是 IO 密集型，可忽略。

## Related

- `docs/adr/0004-desktop-hybrid-business-model.md` — 混合商业模式
- `docs/desktop-llm-provider-design.md` — LLM 兼容性与诊断设计
- opencode 参考：`sst/opencode` `packages/llm/`（[架构文档](https://zread.ai/sst/opencode/10-multi-provider-llm-integration)）