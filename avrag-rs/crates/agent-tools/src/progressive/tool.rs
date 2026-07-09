/// A Tool wraps a [`contracts::ToolSpec`] (JSON schema + description) so it can be
/// progressively disclosed to the LLM planner.
#[derive(Clone)]
pub struct Tool {
    spec: contracts::ToolSpec,
    gotchas: Vec<String>,
}

impl Tool {
    pub fn new(spec: contracts::ToolSpec) -> Self {
        Self {
            spec,
            gotchas: Vec::new(),
        }
    }

    pub fn with_gotchas(mut self, gotchas: Vec<String>) -> Self {
        self.gotchas = gotchas;
        self
    }

    pub fn spec(&self) -> &contracts::ToolSpec {
        &self.spec
    }
}

impl super::DisclosureUnit for Tool {
    fn id(&self) -> &str {
        &self.spec.name
    }

    fn render(&self, ctx: &super::DisclosureContext) -> String {
        match ctx.tier {
            super::DisclosureTier::Index => {
                // Plan phase: lightweight — name, description, param summary.
                let params = schema_param_summary(&self.spec.input_schema);
                format!(
                    "### {} (v{})\n{}\n\nParameters:\n{}",
                    self.spec.name, self.spec.version, self.spec.description, params
                )
            }
            _ => {
                // Load / Runtime: full schema + gotchas.
                let is_first = !ctx.seen_unit_ids.contains(&self.spec.name);
                let schema = serde_json::to_string_pretty(&self.spec.input_schema)
                    .unwrap_or_else(|_| "{}".to_string());
                let mut output = if is_first {
                    format!(
                        "### {} (v{})\n{}\n\nSchema:\n{}",
                        self.spec.name, self.spec.version, self.spec.description, schema
                    )
                } else {
                    format!(
                        "### {} (v{})\n{}\n\nSchema:\n{}\n\n_(Subsequent disclosure — full description shown in first iteration.)_",
                        self.spec.name, self.spec.version, self.spec.description, schema
                    )
                };
                if is_first && !self.gotchas.is_empty() {
                    output.push_str("\n\nGotchas:\n");
                    for g in &self.gotchas {
                        output.push_str(&format!("- {}\n", g));
                    }
                }
                output
            }
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Extract a human-readable parameter summary from a JSON schema object.
fn schema_param_summary(schema: &serde_json::Value) -> String {
    let mut lines = Vec::new();
    if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
        let required: std::collections::HashSet<&str> = schema
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();
        for (name, def) in props {
            let ty = def.get("type").and_then(|t| t.as_str()).unwrap_or("any");
            let desc = def
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("");
            let req = if required.contains(name.as_str()) {
                " (required)"
            } else {
                ""
            };
            let default = def
                .get("default")
                .map(|d| format!(", default: {}", d))
                .unwrap_or_default();
            let line = format!("- `{}`: {}{}{} — {}", name, ty, req, default, desc);
            lines.push(line);
        }
    }
    if lines.is_empty() {
        "(no parameters)".to_string()
    } else {
        lines.join("\n")
    }
}
