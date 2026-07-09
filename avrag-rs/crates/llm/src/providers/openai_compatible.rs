use super::Provider;
use crate::protocols::OpenAiChatProtocol;
use crate::route::{Auth, Endpoint, Framing, Route};

#[derive(Debug, Clone, Copy)]
pub struct Profile {
    pub id: &'static str,
    pub base_url: &'static str,
    pub default_model: &'static str,
}

pub const PROFILES: &[Profile] = &[
    Profile {
        id: "deepseek",
        base_url: "https://api.deepseek.com",
        default_model: "deepseek-chat",
    },
    Profile {
        id: "zhipu",
        base_url: "https://open.bigmodel.cn/api/paas/v4",
        default_model: "glm-4.6",
    },
    Profile {
        id: "siliconflow",
        base_url: "https://api.siliconflow.cn/v1",
        default_model: "Qwen/Qwen2.5-72B-Instruct",
    },
    Profile {
        id: "dashscope",
        base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
        default_model: "qwen-plus",
    },
    Profile {
        id: "groq",
        base_url: "https://api.groq.com/openai/v1",
        default_model: "llama-3.3-70b-versatile",
    },
    Profile {
        id: "cerebras",
        base_url: "https://api.cerebras.ai/v1",
        default_model: "llama3.1-8b",
    },
    Profile {
        id: "togetherai",
        base_url: "https://api.together.xyz/v1",
        default_model: "meta-llama/Llama-3-70b-chat-hf",
    },
    Profile {
        id: "fireworks",
        base_url: "https://api.fireworks.ai/inference/v1",
        default_model: "accounts/fireworks/models/llama-v3p1-70b-instruct",
    },
    Profile {
        id: "openrouter",
        base_url: "https://openrouter.ai/api/v1",
        default_model: "anthropic/claude-3.5-sonnet",
    },
    Profile {
        id: "xai",
        base_url: "https://api.x.ai/v1",
        default_model: "grok-2",
    },
    Profile {
        id: "deepinfra",
        base_url: "https://api.deepinfra.com/v1/openai",
        default_model: "meta-llama/Meta-Llama-3.1-70B-Instruct",
    },
    Profile {
        id: "ollama",
        base_url: "http://localhost:11434/v1",
        default_model: "llama3.2",
    },
];

pub fn find_profile(id: &str) -> Option<&'static Profile> {
    PROFILES.iter().find(|profile| profile.id == id)
}

pub fn configure(
    profile: &Profile,
    api_key: String,
    base_url: Option<String>,
) -> Provider {
    let base = base_url
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| profile.base_url.to_string());
    configure_with_id(profile.id, api_key, base)
}

pub fn configure_generic(api_key: String, base_url: String) -> Provider {
    configure_with_id("custom", api_key, base_url)
}

fn configure_with_id(id: &str, api_key: String, base_url: String) -> Provider {
    let auth = if api_key.is_empty() && base_url.contains("localhost") {
        Auth::None
    } else if api_key.is_empty() {
        Auth::None
    } else {
        Auth::Bearer(api_key)
    };
    let route = Route {
        id: id.to_string(),
        provider: id.to_string(),
        protocol: OpenAiChatProtocol,
        endpoint: Endpoint::new(base_url, "/chat/completions"),
        auth,
        framing: Framing::Sse,
        http_client: default_http_client(),
    };
    Provider::from_route(id, route)
}

fn default_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("reqwest client should build")
}

#[cfg(test)]
mod tests {
    use super::{find_profile, PROFILES};

    #[test]
    fn profiles_match_frontend_preset_ids() {
        let expected = [
            "deepseek",
            "zhipu",
            "siliconflow",
            "dashscope",
            "groq",
            "cerebras",
            "togetherai",
            "fireworks",
            "openrouter",
            "xai",
            "deepinfra",
            "ollama",
        ];
        assert_eq!(PROFILES.len(), expected.len());
        for id in expected {
            assert!(find_profile(id).is_some(), "missing profile {id}");
        }
    }
}
