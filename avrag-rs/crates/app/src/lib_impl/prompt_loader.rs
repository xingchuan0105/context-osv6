use std::path::PathBuf;

/// Load a prompt template from the prompts directory at runtime.
///
/// System prompts (CDS C-family): searches under `{prompts_dir}/pipeline/`:
/// 1. `{base}.system.{version}.md`
/// 2. `{base}.system.md`
/// 3. legacy `{base}.{version}.tmpl` / `{base}.tmpl` at prompts root
///
/// User templates: searches under `{prompts_dir}/templates/` then legacy root.
pub async fn load_prompt_template(
    prompts_dir: &str,
    version: &str,
    base_name: &str,
) -> Option<String> {
    let prompts_dir = PathBuf::from(prompts_dir.trim());
    let version = version.trim();
    let kebab = base_name.replace('_', "-");

    let mut candidates = Vec::new();
    if !version.is_empty() {
        candidates.push(prompts_dir.join(format!("pipeline/{kebab}.system.{version}.md")));
        candidates.push(prompts_dir.join(format!("pipeline/{base_name}.system.{version}.md")));
    }
    candidates.push(prompts_dir.join(format!("pipeline/{kebab}.system.md")));
    candidates.push(prompts_dir.join(format!("pipeline/{base_name}.system.md")));
    candidates.push(prompts_dir.join(format!("templates/{base_name}.tmpl")));
    candidates.push(prompts_dir.join(format!("templates/{kebab}.tmpl")));
    if !version.is_empty() {
        candidates.push(prompts_dir.join(format!("{base_name}.{version}.tmpl")));
        candidates.push(prompts_dir.join(format!("{base_name}_{version}.tmpl")));
    }
    candidates.push(prompts_dir.join(format!("{base_name}.tmpl")));

    for path in candidates {
        match tokio::fs::read_to_string(&path).await {
            Ok(template) => {
                let template = template.trim().to_string();
                if !template.is_empty() {
                    return Some(template);
                }
            }
            Err(_) => continue,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_prompts_dir() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "avrag-prompt-loader-{}-{nonce}",
            std::process::id()
        ))
    }

    #[tokio::test]
    async fn user_templates_prefer_templates_dir_over_legacy_root() {
        let dir = temp_prompts_dir();
        let templates_dir = dir.join("templates");
        std::fs::create_dir_all(&templates_dir).expect("create templates dir");
        std::fs::write(dir.join("summary-user.tmpl"), "legacy root")
            .expect("write legacy template");
        std::fs::write(templates_dir.join("summary-user.tmpl"), "templates dir")
            .expect("write templates template");

        let loaded = load_prompt_template(
            dir.to_str().expect("temp path should be utf-8"),
            "",
            "summary-user",
        )
        .await;

        assert_eq!(loaded.as_deref(), Some("templates dir"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
