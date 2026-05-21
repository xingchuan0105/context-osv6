use common::{ToolResult, ToolSpec, ToolStatus};
use serde_json::Value;

use crate::agents::skills::{ExecutionContext, SkillComponent};

/// Code Interpreter Skill — executes Python in a sandboxed environment.
///
/// # Gotchas (accumulated from real failures)
/// - The sandbox always returns `success: true`; exceptions are caught and
///   printed to stderr. Do not rely on `success` to detect logic errors.
/// - `sys.exit(1)` triggers ImportError because `sys` is in the blocked list.
/// - `_result` is only set when the last statement is an expression, not
///   an assignment.
pub struct CodeInterpreterSkill;

#[async_trait::async_trait]
impl SkillComponent for CodeInterpreterSkill {
    fn id(&self) -> &str {
        "code_interpreter"
    }

    fn version(&self) -> &str {
        "1.0"
    }

    /// Index-tier routing trigger.
    fn description(&self) -> &str {
        "Load when the user needs to run Python code, analyze data, or generate a chart."
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "code_interpreter".to_string(),
            version: "1.0".to_string(),
            description: concat!(
                "Execute Python code in a sandboxed environment. ",
                "Supports math, data analysis, list/dict manipulation, and standard library modules (except os, subprocess, socket, sys, ctypes).\n",
                "Use this when the user asks to compute, analyze, or transform data programmatically."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "code": {
                        "type": "string",
                        "description": "Python code to execute in the sandbox."
                    }
                },
                "required": ["code"]
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "stdout": {"type": "string"},
                    "stderr": {"type": "string"},
                    "result": {"type": "string", "description": "Value of the last expression (if any)."},
                    "success": {"type": "boolean"}
                }
            }),
        }
    }

    fn gotchas(&self) -> &[&str] {
        &[
            "The sandbox always returns success=true. Exceptions are caught and printed to stderr — check stderr for errors.",
            "sys.exit() is blocked (sys is in the deny-list). Calling it raises ImportError.",
            "The _result field is only populated when the last statement is an expression, not an assignment.",
            "Large outputs may be truncated. Prefer writing summaries over dumping huge DataFrames.",
        ]
    }

    fn render_hint(&self) -> &str {
        "code"
    }

    async fn execute<'a>(&self, args: &Value, _ctx: &'a ExecutionContext<'a>) -> ToolResult {
        let code = args
            .get("code")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        if code.is_empty() {
            return ToolResult {
                tool: self.id().to_string(),
                version: self.version().to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({ "error": "missing code" })),
                trace: None,
            };
        }

        let code = code.to_string();
        let result = tokio::task::spawn_blocking(move || {
            let interpreter = avrag_code_interpreter::CodeInterpreter::new();
            interpreter.execute(&code)
        })
        .await;

        match result {
            Ok(Ok(exec)) => ToolResult {
                tool: self.id().to_string(),
                version: self.version().to_string(),
                status: if exec.success {
                    ToolStatus::Ok
                } else {
                    ToolStatus::Error
                },
                data: Some(serde_json::json!({
                    "stdout": exec.stdout,
                    "stderr": exec.stderr,
                    "result": exec.result,
                    "success": exec.success,
                    "exit_code": exec.exit_code,
                    "killed": exec.killed,
                })),
                trace: None,
            },
            Ok(Err(error)) => ToolResult {
                tool: self.id().to_string(),
                version: self.version().to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({ "error": error.to_string() })),
                trace: None,
            },
            Err(error) => ToolResult {
                tool: self.id().to_string(),
                version: self.version().to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({ "error": format!("task panicked: {error}") })),
                trace: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> ExecutionContext<'static> {
        ExecutionContext::new(None)
    }

    #[tokio::test]
    async fn test_code_interpreter_simple() {
        let skill = CodeInterpreterSkill;
        let result = skill.execute(&serde_json::json!({"code": "print(1 + 2)"}), &ctx()).await;
        assert_eq!(result.status, ToolStatus::Ok);
        let data = result.data.unwrap();
        assert!(data["stdout"].as_str().unwrap().contains("3"));
        assert!(data["success"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_code_interpreter_missing_code() {
        let skill = CodeInterpreterSkill;
        let result = skill.execute(&serde_json::json!({}), &ctx()).await;
        assert_eq!(result.status, ToolStatus::Error);
        let data = result.data.unwrap();
        assert!(data["error"].as_str().unwrap().contains("missing code"));
    }

    #[tokio::test]
    async fn test_code_interpreter_stderr() {
        // The sandbox always returns success=True; exceptions are captured in stderr.
        let skill = CodeInterpreterSkill;
        let result = skill
            .execute(&serde_json::json!({"code": "raise ValueError('error')"}), &ctx())
            .await;
        assert_eq!(result.status, ToolStatus::Ok);
        let data = result.data.unwrap();
        assert!(data["stderr"].as_str().unwrap().contains("ValueError"));
        assert!(data["success"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_code_interpreter_exception() {
        // Exceptions are caught by the sandbox wrapper and printed to stderr.
        let skill = CodeInterpreterSkill;
        let result = skill.execute(&serde_json::json!({"code": "1/0"}), &ctx()).await;
        assert_eq!(result.status, ToolStatus::Ok);
        let data = result.data.unwrap();
        assert!(data["stderr"].as_str().unwrap().contains("ZeroDivisionError"));
        assert!(data["success"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_code_interpreter_result_field() {
        let skill = CodeInterpreterSkill;
        let result = skill.execute(&serde_json::json!({"code": "x = 42"}), &ctx()).await;
        assert_eq!(result.status, ToolStatus::Ok);
        let data = result.data.unwrap();
        // _result is only set if the last statement is an expression, not an assignment.
        assert!(data["result"].is_null() || data["result"] == serde_json::Value::Null);
    }
}
