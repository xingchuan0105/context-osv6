use super::config_types::{LoopExitConfig, ModeConfig};

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

    let content = std::fs::read_to_string(&resolved_path).map_err(|e| {
        common::AppError::internal(format!(
            "failed to read mode config at {:?}: {}",
            resolved_path, e
        ))
    })?;
    let mut config: ModeConfig = serde_yaml::from_str(&content)
        .map_err(|e| common::AppError::internal(format!("failed to parse mode config: {e}")))?;
    config.normalize();
    config.validate()?;
    Ok(config)
}

impl ModeConfig {
    pub fn loop_exit_for_mode(&self) -> LoopExitConfig {
        let mut cfg = self.loop_exit.clone();
        if self.id == "chat" {
            if !self.loop_exit.require_evidence
                && !self.loop_exit.allow_content_early_stop
                && !self.loop_exit.skip_synthesis_on_direct_answer
            {
                cfg.require_evidence = false;
                cfg.allow_content_early_stop = true;
                cfg.skip_synthesis_on_direct_answer = true;
            }
        } else if (self.id == "rag" || self.id == "search")
            && !self.loop_exit.require_evidence
            && self.loop_exit.allow_content_early_stop
        {
            cfg.require_evidence = true;
            cfg.allow_content_early_stop = false;
            cfg.skip_synthesis_on_direct_answer = false;
        }
        cfg
    }

    pub fn normalize(&mut self) {
        self.skill_catalog.hydrate_clusters();
    }

    pub fn validate(&self) -> Result<(), common::AppError> {
        if self.id.is_empty() {
            return Err(common::AppError::validation(
                "mode_config",
                "mode id is empty",
            ));
        }
        if self.budget.max_iterations == 0 {
            return Err(common::AppError::validation(
                "mode_config",
                "budget.max_iterations must be > 0",
            ));
        }
        Ok(())
    }

    pub fn mandatory_synthesis_skills(&self) -> &[String] {
        &self.skill_catalog.mandatory.synthesis
    }

    pub fn resolve_tool_specs(
        &self,
        registry: &agent_tools::capability::CapabilityRegistry,
        ids: &[String],
    ) -> Vec<contracts::ToolSpec> {
        ids.iter()
            .filter_map(|id| registry.tool(id).map(tool_metadata_to_spec))
            .collect()
    }

    /// Resolve tool specs for the retrieve phase from `tool_pool`.
    pub fn tools_for_retrieve(
        &self,
        registry: &agent_tools::capability::CapabilityRegistry,
    ) -> Vec<contracts::ToolSpec> {
        if self.tool_pool.is_empty() {
            return vec![];
        }
        self.resolve_tool_specs(registry, &self.tool_pool)
    }
}

fn tool_metadata_to_spec(meta: &agent_tools::capability::ToolMetadata) -> contracts::ToolSpec {
    contracts::ToolSpec {
        name: meta.id.clone(),
        version: meta.version.clone(),
        description: meta.description.clone(),
        input_schema: meta.input_schema.clone(),
        output_schema: meta.output_schema.clone(),
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

    let content = std::fs::read_to_string(&resolved_path).map_err(|e| {
        common::AppError::internal(format!(
            "failed to read prompt file {:?}: {}",
            resolved_path, e
        ))
    })?;
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
