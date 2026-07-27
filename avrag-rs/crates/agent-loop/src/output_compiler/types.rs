//! Core compiler types: diagnostics and compile outcomes.

/// Diagnostic severity. Errors reject the output (loop: one compile-feedback
/// continuation; post-loop: degraded with codes). Warnings are advisory only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// One rustc-style diagnostic: what is wrong, where, and how to fix it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Stable machine-readable code, e.g. "E101".
    pub code: &'static str,
    pub severity: Severity,
    /// JSON-ish location, e.g. "key_facts[2].evidence".
    pub field: Option<String>,
    /// What is wrong (model-facing).
    pub message: String,
    /// How to fix it (model-facing natural language).
    pub suggestion: String,
}

impl Diagnostic {
    pub fn error(
        code: &'static str,
        field: Option<String>,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            code,
            severity: Severity::Error,
            field,
            message: message.into(),
            suggestion: suggestion.into(),
        }
    }

    pub fn warning(
        code: &'static str,
        field: Option<String>,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            code,
            severity: Severity::Warning,
            field,
            message: message.into(),
            suggestion: suggestion.into(),
        }
    }
}

/// Result of compiling one agent output: a (possibly transformed) value plus
/// all diagnostics. `value` may be present alongside Error diagnostics so a
/// post-loop caller can still build a degraded result from what survived.
#[derive(Debug)]
pub struct CompileOutcome<T> {
    pub value: Option<T>,
    pub diagnostics: Vec<Diagnostic>,
}

impl<T> CompileOutcome<T> {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    /// Diagnostic codes in emission order, deduplicated (for
    /// `WorkerHandoff.compile_diagnostics` / logs).
    pub fn diagnostic_codes(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for d in &self.diagnostics {
            if !out.iter().any(|c| c == d.code) {
                out.push(d.code.to_string());
            }
        }
        out
    }

    /// Compact model-facing feedback for the ONE compile continuation: every
    /// error with code + suggestion, plus the re-output discipline. Warnings
    /// are excluded — they never block.
    pub fn render_feedback(&self) -> String {
        let mut out = String::from("编译失败（输出未通过契约校验）：\n");
        for d in self
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
        {
            out.push_str("- ");
            out.push_str(d.code);
            if let Some(field) = &d.field {
                out.push_str(&format!("（{field}）"));
            }
            out.push_str(&format!("：{}", d.message));
            if !d.suggestion.is_empty() {
                out.push_str(&format!("。建议：{}", d.suggestion));
            }
            out.push('\n');
        }
        out.push_str(
            "请按契约重新输出最终 JSON：不要新检索、不要代码块、不要 markdown 围栏，\
             直接输出修复后的完整 JSON 对象。",
        );
        out
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn has_errors_only_counts_error_severity() {
        let outcome: CompileOutcome<()> = CompileOutcome {
            value: None,
            diagnostics: vec![Diagnostic::warning("W101", None, "m", "s")],
        };
        assert!(!outcome.has_errors());
    }

    #[test]
    fn diagnostic_codes_dedupe_preserving_order() {
        let outcome: CompileOutcome<()> = CompileOutcome {
            value: None,
            diagnostics: vec![
                Diagnostic::error("E103", None, "a", ""),
                Diagnostic::error("E103", None, "b", ""),
                Diagnostic::warning("W101", None, "c", ""),
            ],
        };
        assert_eq!(outcome.diagnostic_codes(), vec!["E103", "W101"]);
    }

    #[test]
    fn feedback_lists_errors_with_code_and_suggestion_and_skips_warnings() {
        let outcome: CompileOutcome<()> = CompileOutcome {
            value: None,
            diagnostics: vec![
                Diagnostic::error("E101", None, "非契约外壳", "给出契约骨架"),
                Diagnostic::error(
                    "E103",
                    Some("key_facts[0].evidence".into()),
                    "指针不存在",
                    "列出合法指针",
                ),
                Diagnostic::warning("W101", None, "含推断词", "标注为推断"),
            ],
        };
        let fb = outcome.render_feedback();
        assert!(fb.contains("编译失败"), "{fb}");
        assert!(fb.contains("E101"), "{fb}");
        assert!(fb.contains("给出契约骨架"), "{fb}");
        assert!(fb.contains("E103（key_facts[0].evidence）"), "{fb}");
        assert!(fb.contains("请按契约重新输出"), "{fb}");
        assert!(fb.contains("不要新检索"), "{fb}");
        assert!(!fb.contains("W101"), "warnings never reach feedback: {fb}");
    }
}
