use common::{ToolResult, ToolSpec, ToolStatus};
use serde_json::Value;

use crate::agents::skills::{ExecutionContext, SkillComponent};

/// Calculator Skill — evaluates mathematical expressions.
///
/// # Gotchas (accumulated from real failures)
/// - Empty input returns Error, not 0.
/// - Scientific notation (e.g. `1e3`) is **not** supported by evalexpr.
/// - Division by zero produces an evalexpr error (not Infinity).
pub struct CalculatorSkill;

#[async_trait::async_trait]
impl SkillComponent for CalculatorSkill {
    fn id(&self) -> &str {
        "calculator"
    }

    fn version(&self) -> &str {
        "1.0"
    }

    /// Index-tier routing trigger.
    ///
    /// Every word here is paid by every session, every user. Keep it tight.
    fn description(&self) -> &str {
        "Load when the user asks to compute, evaluate, or solve a mathematical expression."
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "calculator".to_string(),
            version: "1.0".to_string(),
            description: concat!(
                "Evaluate a mathematical expression. ",
                "Supports arithmetic (+, -, *, /, %, ^), ",
                "functions (sin, cos, tan, sqrt, abs, log, ln, pow, min, max, floor, ceil, round), ",
                "and grouping with parentheses.\n",
                "Examples: '1 + 2 * 3', 'sin(30 * pi / 180)', 'sqrt(16) + pow(2, 3)'."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "Mathematical expression to evaluate."
                    }
                },
                "required": ["expression"]
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "result": {"type": "number", "description": "The computed numeric result."},
                    "expression": {"type": "string", "description": "The original expression."}
                }
            }),
        }
    }

    fn gotchas(&self) -> &[&str] {
        &[
            "Empty expression input returns Error — do not send empty strings.",
            "Scientific notation (e.g. 1e3, 2.5e-4) is NOT supported. Rewrite as decimals.",
            "Division by zero produces an error, not Infinity or NaN.",
        ]
    }

    fn render_hint(&self) -> &str {
        "calculator"
    }

    async fn execute<'a>(&self, args: &Value, _ctx: &'a ExecutionContext<'a>) -> ToolResult {
        let expression = args
            .get("expression")
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        if expression.is_empty() {
            return ToolResult {
                tool: self.id().to_string(),
                version: self.version().to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({ "error": "missing expression" })),
                trace: None,
            };
        }

        match evaluate_calculator_expression(expression) {
            Ok(result) => ToolResult {
                tool: self.id().to_string(),
                version: self.version().to_string(),
                status: ToolStatus::Ok,
                data: Some(serde_json::json!({
                    "result": result,
                    "expression": expression,
                })),
                trace: None,
            },
            Err(error) => ToolResult {
                tool: self.id().to_string(),
                version: self.version().to_string(),
                status: ToolStatus::Error,
                data: Some(serde_json::json!({ "error": error })),
                trace: None,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Calculator evaluation helper (moved from tool_catalog.rs)
// ---------------------------------------------------------------------------

/// Evaluate a mathematical expression string and return the numeric result.
///
/// Uses `evalexpr` for parsing and evaluation.  Supports:
/// - Arithmetic: `+`, `-`, `*`, `/`, `%`, `^`
/// - Functions: `sin`, `cos`, `tan`, `sqrt`, `abs`, `exp`, `ln`, `log2`, `log10`,
///   `floor`, `ceil`, `round`, `min`, `max`, `pow`
/// - Constants: `pi`, `e`
/// - Grouping with parentheses
pub fn evaluate_calculator_expression(expression: &str) -> Result<f64, String> {
    use evalexpr::*;

    fn to_float(v: &Value) -> Result<f64, EvalexprError> {
        match v {
            Value::Float(f) => Ok(*f),
            Value::Int(i) => Ok(*i as f64),
            other => Err(EvalexprError::expected_float(other.clone())),
        }
    }

    let mut ctx = HashMapContext::new();
    ctx.set_function(
        "sin".to_string(),
        Function::new(|arg| Ok(Value::Float(to_float(arg)?.sin()))),
    )
    .map_err(|e| format!("register sin: {e}"))?;
    ctx.set_function(
        "cos".to_string(),
        Function::new(|arg| Ok(Value::Float(to_float(arg)?.cos()))),
    )
    .map_err(|e| format!("register cos: {e}"))?;
    ctx.set_function(
        "tan".to_string(),
        Function::new(|arg| Ok(Value::Float(to_float(arg)?.tan()))),
    )
    .map_err(|e| format!("register tan: {e}"))?;
    ctx.set_function(
        "sqrt".to_string(),
        Function::new(|arg| Ok(Value::Float(to_float(arg)?.sqrt()))),
    )
    .map_err(|e| format!("register sqrt: {e}"))?;
    ctx.set_function(
        "abs".to_string(),
        Function::new(|arg| Ok(Value::Float(to_float(arg)?.abs()))),
    )
    .map_err(|e| format!("register abs: {e}"))?;
    ctx.set_function(
        "exp".to_string(),
        Function::new(|arg| Ok(Value::Float(to_float(arg)?.exp()))),
    )
    .map_err(|e| format!("register exp: {e}"))?;
    ctx.set_function(
        "ln".to_string(),
        Function::new(|arg| Ok(Value::Float(to_float(arg)?.ln()))),
    )
    .map_err(|e| format!("register ln: {e}"))?;
    ctx.set_function(
        "log2".to_string(),
        Function::new(|arg| Ok(Value::Float(to_float(arg)?.log2()))),
    )
    .map_err(|e| format!("register log2: {e}"))?;
    ctx.set_function(
        "log10".to_string(),
        Function::new(|arg| Ok(Value::Float(to_float(arg)?.log10()))),
    )
    .map_err(|e| format!("register log10: {e}"))?;
    ctx.set_function(
        "floor".to_string(),
        Function::new(|arg| Ok(Value::Float(to_float(arg)?.floor()))),
    )
    .map_err(|e| format!("register floor: {e}"))?;
    ctx.set_function(
        "ceil".to_string(),
        Function::new(|arg| Ok(Value::Float(to_float(arg)?.ceil()))),
    )
    .map_err(|e| format!("register ceil: {e}"))?;
    ctx.set_function(
        "round".to_string(),
        Function::new(|arg| Ok(Value::Float(to_float(arg)?.round()))),
    )
    .map_err(|e| format!("register round: {e}"))?;
    ctx.set_function(
        "pow".to_string(),
        Function::new(|arg| {
            let tuple = arg.as_tuple()?;
            if tuple.len() != 2 {
                return Err(EvalexprError::WrongFunctionArgumentAmount {
                    expected: 2..=2,
                    actual: tuple.len(),
                });
            }
            let a = to_float(&tuple[0])?;
            let b = to_float(&tuple[1])?;
            Ok(Value::Float(a.powf(b)))
        }),
    )
    .map_err(|e| format!("register pow: {e}"))?;
    ctx.set_function(
        "min".to_string(),
        Function::new(|arg| {
            let tuple = arg.as_tuple()?;
            let min = tuple
                .iter()
                .map(to_float)
                .collect::<Result<Vec<f64>, _>>()?
                .into_iter()
                .fold(f64::INFINITY, f64::min);
            Ok(Value::Float(min))
        }),
    )
    .map_err(|e| format!("register min: {e}"))?;
    ctx.set_function(
        "max".to_string(),
        Function::new(|arg| {
            let tuple = arg.as_tuple()?;
            let max = tuple
                .iter()
                .map(to_float)
                .collect::<Result<Vec<f64>, _>>()?
                .into_iter()
                .fold(f64::NEG_INFINITY, f64::max);
            Ok(Value::Float(max))
        }),
    )
    .map_err(|e| format!("register max: {e}"))?;
    ctx.set_value("pi".to_string(), Value::Float(std::f64::consts::PI))
        .map_err(|e| format!("register pi: {e}"))?;
    ctx.set_value("e".to_string(), Value::Float(std::f64::consts::E))
        .map_err(|e| format!("register e: {e}"))?;

    let result =
        eval_with_context_mut(expression, &mut ctx).map_err(|e| format!("evalexpr error: {e}"))?;

    match result {
        Value::Float(f) => Ok(f),
        Value::Int(i) => Ok(i as f64),
        other => Err(format!("unexpected result type: {:?}", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculator_basic_arithmetic() {
        assert!((evaluate_calculator_expression("1 + 2 * 3").unwrap() - 7.0).abs() < 1e-10);
        assert!((evaluate_calculator_expression("(1 + 2) * 3").unwrap() - 9.0).abs() < 1e-10);
        assert!((evaluate_calculator_expression("10.0 / 3.0").unwrap() - 3.33333).abs() < 1e-4);
        assert!((evaluate_calculator_expression("2 ^ 8").unwrap() - 256.0).abs() < 1e-10);
        assert!((evaluate_calculator_expression("10 % 3").unwrap() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_calculator_sin_cos() {
        let sin30 = evaluate_calculator_expression("sin(30 * pi / 180)").unwrap();
        assert!((sin30 - 0.5).abs() < 1e-10);
        let cos0 = evaluate_calculator_expression("cos(0)").unwrap();
        assert!((cos0 - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_calculator_sqrt_and_pow() {
        let sqrt16 = evaluate_calculator_expression("sqrt(16)").unwrap();
        assert!((sqrt16 - 4.0).abs() < 1e-10);
        let pow = evaluate_calculator_expression("pow(2, 3)").unwrap();
        assert!((pow - 8.0).abs() < 1e-10);
    }

    #[test]
    fn test_calculator_log() {
        assert!((evaluate_calculator_expression("ln(e)").unwrap() - 1.0).abs() < 1e-10);
        assert!((evaluate_calculator_expression("log10(100)").unwrap() - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_calculator_floor_ceil_round() {
        assert!((evaluate_calculator_expression("floor(3.7)").unwrap() - 3.0).abs() < 1e-10);
        assert!((evaluate_calculator_expression("ceil(3.2)").unwrap() - 4.0).abs() < 1e-10);
        assert!((evaluate_calculator_expression("round(3.5)").unwrap() - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_calculator_min_max() {
        assert!((evaluate_calculator_expression("min(3, 1, 2)").unwrap() - 1.0).abs() < 1e-10);
        assert!((evaluate_calculator_expression("max(3, 1, 2)").unwrap() - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_calculator_invalid_expression() {
        assert!(evaluate_calculator_expression("1 +").is_err());
        assert!(evaluate_calculator_expression("sin(1, 2)").is_err());
    }

    #[tokio::test]
    async fn test_skill_empty_expression_returns_error() {
        let skill = CalculatorSkill;
        let ctx = ExecutionContext::new(None);
        let result = skill.execute(&serde_json::json!({}), &ctx).await;
        assert_eq!(result.status, ToolStatus::Error);
    }

    #[tokio::test]
    async fn test_skill_basic_eval() {
        let skill = CalculatorSkill;
        let ctx = ExecutionContext::new(None);
        let result = skill
            .execute(&serde_json::json!({"expression": "1 + 2 * 3"}), &ctx)
            .await;
        assert_eq!(result.status, ToolStatus::Ok);
        let data = result.data.unwrap();
        assert_eq!(data["result"].as_f64().unwrap(), 7.0);
    }
}
