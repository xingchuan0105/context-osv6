use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModeConfig {
    #[serde(alias = "mode")]
    pub id: String,
    pub system_prompt_base: String,
    pub native_tools: Vec<common::ToolSpec>,
    pub skill_catalog: Vec<String>,
    pub disclosure: DisclosureConfig,
    pub budget: BudgetConfig,
    pub auto_fallback: Option<AutoFallbackConfig>,
    #[serde(default)]
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DisclosureConfig {
    pub rounds: Vec<DisclosureRound>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DisclosureRound {
    pub round_idx: u8,
    pub load: DisclosureLoad,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DisclosureLoad {
    Index,
    Skills(Vec<String>),
    Auto,
}

impl<'de> serde::Deserialize<'de> for DisclosureLoad {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct DisclosureLoadVisitor;
        impl<'de> serde::de::Visitor<'de> for DisclosureLoadVisitor {
            type Value = DisclosureLoad;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a string ('index', 'auto'), a map with 'skills', or a list of strings")
            }
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value.to_lowercase().as_str() {
                    "index" => Ok(DisclosureLoad::Index),
                    "auto" => Ok(DisclosureLoad::Auto),
                    _ => Err(serde::de::Error::custom(format!("unknown load type: {}", value))),
                }
            }
            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut skills = Vec::new();
                while let Some(skill) = seq.next_element()? {
                    skills.push(skill);
                }
                Ok(DisclosureLoad::Skills(skills))
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut skills = None;
                while let Some(key) = map.next_key::<String>()? {
                    if key == "skills" {
                        skills = Some(map.next_value::<Vec<String>>()?);
                    } else {
                        let _: serde::de::IgnoredAny = map.next_value()?;
                    }
                }
                match skills {
                    Some(s) => Ok(DisclosureLoad::Skills(s)),
                    None => Err(serde::de::Error::custom("missing field 'skills'")),
                }
            }
        }
        deserializer.deserialize_any(DisclosureLoadVisitor)
    }
}


#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BudgetConfig {
    pub max_iterations: u8,
    #[serde(default)]
    pub by_user_tier: Option<HashMap<String, u8>>,
}

impl BudgetConfig {
    /// Resolve the iteration ceiling for a concrete user tier.
    ///
    /// - If `request_tier` is provided and matches a key in `by_user_tier`,
    ///   use that value.
    /// - Otherwise fall back to `max_iterations`.
    /// - The result is clamped to at least 1 so the loop can always run
    ///   the synthesis pass.
    pub fn resolve_max_iterations(
        &self,
        request_tier: Option<&serde_json::Value>,
    ) -> u8 {
        let tier_str = request_tier
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase());
        let resolved = if let Some(tier) = tier_str {
            self.by_user_tier
                .as_ref()
                .and_then(|m| m.get(&tier).copied())
                .unwrap_or(self.max_iterations)
        } else {
            self.max_iterations
        };
        resolved.max(1)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AutoFallbackConfig {
    pub enabled: bool,
    pub tool_id: String,
    pub top_k: u8,
    #[serde(default)]
    pub vertical: Option<String>,
}

pub fn load_mode_config(mode_id: &str) -> Result<ModeConfig, common::AppError> {
    let mut resolved_path = std::path::PathBuf::from(format!("modes/{}.yaml", mode_id));
    if !resolved_path.exists() {
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let workspace_path = std::path::PathBuf::from(manifest_dir)
                .join("../..")
                .join(format!("modes/{}.yaml", mode_id));
            if workspace_path.exists() {
                resolved_path = workspace_path;
            }
        }
    }
    if !resolved_path.exists() {
        let mut dir = std::env::current_dir().unwrap_or_default();
        loop {
            let check_path = dir.join("modes").join(format!("{}.yaml", mode_id));
            if check_path.exists() {
                resolved_path = check_path;
                break;
            }
            if !dir.pop() {
                break;
            }
        }
    }

    let content = std::fs::read_to_string(&resolved_path)
        .map_err(|e| common::AppError::internal(format!("failed to read mode config at {:?}: {}", resolved_path, e)))?;
    let config: ModeConfig = serde_yaml::from_str(&content)
        .map_err(|e| common::AppError::internal(format!("failed to parse mode config: {e}")))?;
    config.validate()?;
    Ok(config)
}

impl ModeConfig {
    pub fn validate(&self) -> Result<(), common::AppError> {
        if self.id.is_empty() {
            return Err(common::AppError::validation("mode_config", "mode id is empty"));
        }
        if self.budget.max_iterations == 0 {
            return Err(common::AppError::validation(
                "mode_config",
                "budget.max_iterations must be > 0",
            ));
        }
        Ok(())
    }
}

/// Load a system prompt file, stripping SKILL.md frontmatter if present.
pub fn load_system_prompt(path: &str) -> Result<String, common::AppError> {
    let mut resolved_path = std::path::PathBuf::from(path);
    if !resolved_path.exists() {
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let workspace_path = std::path::PathBuf::from(manifest_dir)
                .join("../..")
                .join(path);
            if workspace_path.exists() {
                resolved_path = workspace_path;
            }
        }
    }
    if !resolved_path.exists() {
        let mut dir = std::env::current_dir().unwrap_or_default();
        loop {
            let check_path = dir.join(path);
            if check_path.exists() {
                resolved_path = check_path;
                break;
            }
            if !dir.pop() {
                break;
            }
        }
    }

    let content = std::fs::read_to_string(&resolved_path)
        .map_err(|e| common::AppError::internal(format!("failed to read prompt file {:?}: {}", resolved_path, e)))?;
    Ok(strip_frontmatter(&content))
}

fn strip_frontmatter(content: &str) -> String {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return content.to_string();
    }
    let after_open = &trimmed[3..];
    let after_open = after_open.strip_prefix('\r').unwrap_or(after_open);
    let after_open = after_open.strip_prefix('\n').unwrap_or(after_open);
    let Some(close_idx) = after_open.find("\n---") else {
        return content.to_string();
    };
    let body_start = close_idx + 4;
    after_open[body_start..].trim_start().to_string()
}
