//! Provider metadata for diagnostic UI (mirrors `frontend_next/lib/desktop/llm-presets.ts`).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthStyle {
    Bearer,
    XApiKey,
    XGoogApiKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolKind {
    OpenAiChat,
    AnthropicMessages,
    Gemini,
}

#[derive(Debug, Clone, Copy)]
pub struct ProviderProfile {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub base_url: &'static str,
    pub default_model: &'static str,
    pub api_key_url: &'static str,
    pub docs_url: &'static str,
    pub pricing_note: &'static str,
    pub auth_style: AuthStyle,
    pub protocol: ProtocolKind,
}

pub const PROVIDER_PROFILES: &[ProviderProfile] = &[
    ProviderProfile {
        id: "zhipu",
        label: "智谱 GLM（含 Coding Plan）",
        description: "按月订阅 Coding Plan 无 token 计费，或按 token 付费",
        base_url: "https://open.bigmodel.cn/api/paas/v4",
        default_model: "glm-4.6",
        api_key_url: "https://open.bigmodel.cn/console/apikey",
        docs_url: "https://open.bigmodel.cn/dev/api",
        pricing_note: "Coding Plan ¥20/月 · 或按 token",
        auth_style: AuthStyle::Bearer,
        protocol: ProtocolKind::OpenAiChat,
    },
    ProviderProfile {
        id: "anthropic",
        label: "Anthropic Claude",
        description: "Claude Sonnet/Opus，原生协议支持 prompt caching",
        base_url: "https://api.anthropic.com/v1",
        default_model: "claude-sonnet-4-20250514",
        api_key_url: "https://console.anthropic.com/settings/keys",
        docs_url: "https://docs.anthropic.com",
        pricing_note: "$3-15/百万 token",
        auth_style: AuthStyle::XApiKey,
        protocol: ProtocolKind::AnthropicMessages,
    },
    ProviderProfile {
        id: "deepseek",
        label: "DeepSeek",
        description: "高性价比，支持 thinking 模式",
        base_url: "https://api.deepseek.com",
        default_model: "deepseek-chat",
        api_key_url: "https://platform.deepseek.com/api_keys",
        docs_url: "https://api-docs.deepseek.com",
        pricing_note: "¥1-8/百万 token",
        auth_style: AuthStyle::Bearer,
        protocol: ProtocolKind::OpenAiChat,
    },
    ProviderProfile {
        id: "openai",
        label: "OpenAI",
        description: "GPT-4o / o1 / o3 系列",
        base_url: "https://api.openai.com/v1",
        default_model: "gpt-4o",
        api_key_url: "https://platform.openai.com/api-keys",
        docs_url: "https://platform.openai.com/docs",
        pricing_note: "$2.50-15/百万 token",
        auth_style: AuthStyle::Bearer,
        protocol: ProtocolKind::OpenAiChat,
    },
    ProviderProfile {
        id: "google",
        label: "Google Gemini",
        description: "Gemini 2.0 Flash / Pro，原生协议",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        default_model: "gemini-2.0-flash",
        api_key_url: "https://aistudio.google.com/apikey",
        docs_url: "https://ai.google.dev/gemini-api/docs",
        pricing_note: "免费额度 · 或 $1.25-5/百万 token",
        auth_style: AuthStyle::XGoogApiKey,
        protocol: ProtocolKind::Gemini,
    },
    ProviderProfile {
        id: "siliconflow",
        label: "SiliconFlow",
        description: "多模型聚合，含 Qwen / DeepSeek 等",
        base_url: "https://api.siliconflow.cn/v1",
        default_model: "Qwen/Qwen2.5-72B-Instruct",
        api_key_url: "https://cloud.siliconflow.cn/account/ak",
        docs_url: "https://docs.siliconflow.cn",
        pricing_note: "¥1-4/百万 token",
        auth_style: AuthStyle::Bearer,
        protocol: ProtocolKind::OpenAiChat,
    },
    ProviderProfile {
        id: "dashscope",
        label: "通义千问（DashScope）",
        description: "阿里云通义千问系列",
        base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
        default_model: "qwen-plus",
        api_key_url: "https://dashscope.console.aliyun.com/apiKey",
        docs_url: "https://help.aliyun.com/zh/dashscope",
        pricing_note: "¥0.8-4/百万 token",
        auth_style: AuthStyle::Bearer,
        protocol: ProtocolKind::OpenAiChat,
    },
    ProviderProfile {
        id: "groq",
        label: "Groq",
        description: "超低延迟推理，Llama 系列",
        base_url: "https://api.groq.com/openai/v1",
        default_model: "llama-3.3-70b-versatile",
        api_key_url: "https://console.groq.com/keys",
        docs_url: "https://console.groq.com/docs",
        pricing_note: "免费额度 · 或 $0.59-0.79/百万 token",
        auth_style: AuthStyle::Bearer,
        protocol: ProtocolKind::OpenAiChat,
    },
    ProviderProfile {
        id: "ollama",
        label: "本地 Ollama",
        description: "完全离线，无需 API key",
        base_url: "http://localhost:11434/v1",
        default_model: "llama3.2",
        api_key_url: "",
        docs_url: "https://ollama.com",
        pricing_note: "免费（本地运行）",
        auth_style: AuthStyle::Bearer,
        protocol: ProtocolKind::OpenAiChat,
    },
    ProviderProfile {
        id: "openrouter",
        label: "OpenRouter",
        description: "聚合 100+ 模型，统一计费",
        base_url: "https://openrouter.ai/api/v1",
        default_model: "anthropic/claude-3.5-sonnet",
        api_key_url: "https://openrouter.ai/keys",
        docs_url: "https://openrouter.ai/docs",
        pricing_note: "按模型不同",
        auth_style: AuthStyle::Bearer,
        protocol: ProtocolKind::OpenAiChat,
    },
    ProviderProfile {
        id: "togetherai",
        label: "Together AI",
        description: "开源模型托管",
        base_url: "https://api.together.xyz/v1",
        default_model: "meta-llama/Llama-3-70b-chat-hf",
        api_key_url: "https://api.together.ai/settings/api-keys",
        docs_url: "https://docs.together.ai",
        pricing_note: "$0.20-5/百万 token",
        auth_style: AuthStyle::Bearer,
        protocol: ProtocolKind::OpenAiChat,
    },
    ProviderProfile {
        id: "cerebras",
        label: "Cerebras",
        description: "超快推理速度",
        base_url: "https://api.cerebras.ai/v1",
        default_model: "llama3.1-8b",
        api_key_url: "https://cloud.cerebras.ai",
        docs_url: "https://inference-docs.cerebras.ai",
        pricing_note: "$0.10-1/百万 token",
        auth_style: AuthStyle::Bearer,
        protocol: ProtocolKind::OpenAiChat,
    },
    ProviderProfile {
        id: "fireworks",
        label: "Fireworks AI",
        description: "开源模型高速推理",
        base_url: "https://api.fireworks.ai/inference/v1",
        default_model: "accounts/fireworks/models/llama-v3p1-70b-instruct",
        api_key_url: "https://fireworks.ai/account/api-keys",
        docs_url: "https://docs.fireworks.ai",
        pricing_note: "$0.20-3/百万 token",
        auth_style: AuthStyle::Bearer,
        protocol: ProtocolKind::OpenAiChat,
    },
    ProviderProfile {
        id: "xai",
        label: "xAI Grok",
        description: "Grok 系列模型",
        base_url: "https://api.x.ai/v1",
        default_model: "grok-2",
        api_key_url: "https://console.x.ai",
        docs_url: "https://docs.x.ai",
        pricing_note: "$2-10/百万 token",
        auth_style: AuthStyle::Bearer,
        protocol: ProtocolKind::OpenAiChat,
    },
    ProviderProfile {
        id: "deepinfra",
        label: "DeepInfra",
        description: "开源模型推理",
        base_url: "https://api.deepinfra.com/v1/openai",
        default_model: "meta-llama/Meta-Llama-3.1-70B-Instruct",
        api_key_url: "https://deepinfra.com/dash/api_keys",
        docs_url: "https://deepinfra.com/docs",
        pricing_note: "$0.20-3/百万 token",
        auth_style: AuthStyle::Bearer,
        protocol: ProtocolKind::OpenAiChat,
    },
    ProviderProfile {
        id: "custom",
        label: "自定义 API",
        description: "任何 OpenAI / Anthropic 兼容端点",
        base_url: "",
        default_model: "",
        api_key_url: "",
        docs_url: "",
        pricing_note: "",
        auth_style: AuthStyle::Bearer,
        protocol: ProtocolKind::OpenAiChat,
    },
];

pub fn find_provider_profile(id: &str) -> Option<&'static ProviderProfile> {
    PROVIDER_PROFILES.iter().find(|profile| profile.id == id)
}

pub fn api_key_url_for_provider(id: &str) -> &'static str {
    find_provider_profile(id)
        .map(|profile| profile.api_key_url)
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::{find_provider_profile, PROVIDER_PROFILES};

    #[test]
    fn provider_profiles_include_all_frontend_preset_ids() {
        let frontend_ids = [
            "zhipu",
            "anthropic",
            "deepseek",
            "openai",
            "google",
            "siliconflow",
            "dashscope",
            "groq",
            "ollama",
            "openrouter",
            "togetherai",
            "cerebras",
            "fireworks",
            "xai",
            "deepinfra",
            "custom",
        ];
        for id in frontend_ids {
            let profile = find_provider_profile(id).unwrap_or_else(|| panic!("missing {id}"));
            assert_eq!(profile.id, id);
        }
        assert_eq!(PROVIDER_PROFILES.len(), frontend_ids.len());
    }
}
