/// Load a prompt template from the prompts directory at runtime.
///
/// Searches candidates in order:
/// 1. `{prompts_dir}/{base_name}.{version}.tmpl`
/// 2. `{prompts_dir}/{base_name}_{version}.tmpl`
/// 3. `{prompts_dir}/{base_name}.tmpl`
///
/// Returns `None` if no readable non-empty file is found.
pub async fn load_prompt_template(prompts_dir: &str, version: &str, base_name: &str) -> Option<String> {
    let prompts_dir = std::path::PathBuf::from(prompts_dir.trim());
    let version = version.trim();
    let mut candidates = Vec::new();
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
