//! Eval framework for SkillComponent routing accuracy.
//!
//! Perplexity best practice: write evals **before** (or alongside) the Skill.
//! Negative examples are often more valuable than positive ones.

/// A single routing eval case.
///
/// `query` is a natural-language user request.
/// `expected_tools` are skills that **must** appear in the Plan-phase disclosure.
/// `forbidden_tools` are skills that **must not** appear.
pub struct RoutingEvalCase {
    pub query: &'static str,
    pub expected_tools: &'static [&'static str],
    pub forbidden_tools: &'static [&'static str],
}

/// Categorised eval suites for built-in atomic skills.
pub struct AtomicSkillEvalSuites;

impl AtomicSkillEvalSuites {
    /// Positive + negative routing cases for the calculator skill.
    pub fn calculator() -> &'static [RoutingEvalCase] {
        &[
            RoutingEvalCase {
                query: "what is 2 + 2",
                expected_tools: &["calculator"],
                forbidden_tools: &["web_search", "code_interpreter"],
            },
            RoutingEvalCase {
                query: "calculate sin(30 degrees)",
                expected_tools: &["calculator"],
                forbidden_tools: &["web_search"],
            },
            RoutingEvalCase {
                query: "solve sqrt(144) * 3",
                expected_tools: &["calculator"],
                forbidden_tools: &[],
            },
            // Negative: nearby domain that should NOT route to calculator
            RoutingEvalCase {
                query: "what is the weather like today",
                expected_tools: &["weather_query"],
                forbidden_tools: &["calculator"],
            },
        ]
    }

    /// Positive + negative routing cases for the code_interpreter skill.
    pub fn code_interpreter() -> &'static [RoutingEvalCase] {
        &[
            RoutingEvalCase {
                query: "run python to sort this list",
                expected_tools: &["code_interpreter"],
                forbidden_tools: &["calculator"],
            },
            RoutingEvalCase {
                query: "write a python script that counts words",
                expected_tools: &["code_interpreter"],
                forbidden_tools: &["web_search"],
            },
            // Negative: math-only should prefer calculator
            RoutingEvalCase {
                query: "what is 15 factorial",
                expected_tools: &["calculator"],
                forbidden_tools: &["code_interpreter"],
            },
        ]
    }

    /// Positive + negative routing cases for the weather_query skill.
    pub fn weather_query() -> &'static [RoutingEvalCase] {
        &[
            RoutingEvalCase {
                query: "weather in Beijing",
                expected_tools: &["weather_query"],
                forbidden_tools: &["web_search"],
            },
            RoutingEvalCase {
                query: "temperature in Tokyo today",
                expected_tools: &["weather_query"],
                forbidden_tools: &["calculator", "code_interpreter"],
            },
            // Negative: historical weather data may need web_search
            RoutingEvalCase {
                query: "what was the weather on D-Day in 1944",
                expected_tools: &["web_search"],
                forbidden_tools: &["weather_query"],
            },
        ]
    }

    /// Positive + negative routing cases for the web_search skill.
    pub fn web_search() -> &'static [RoutingEvalCase] {
        &[
            RoutingEvalCase {
                query: "latest news about AI regulation",
                expected_tools: &["web_search"],
                forbidden_tools: &["calculator"],
            },
            RoutingEvalCase {
                query: "who won the world cup in 2022",
                expected_tools: &["web_search"],
                forbidden_tools: &["code_interpreter"],
            },
            // Boundary: currency conversion — could go either way
            RoutingEvalCase {
                query: "convert 100 USD to EUR today",
                expected_tools: &["web_search"],
                forbidden_tools: &[],
            },
            // Negative: pure math should not search
            RoutingEvalCase {
                query: "what is 123 * 456",
                expected_tools: &["calculator"],
                forbidden_tools: &["web_search"],
            },
        ]
    }

    /// All eval cases across every atomic skill.
    pub fn all() -> Vec<&'static RoutingEvalCase> {
        let mut all = Vec::new();
        all.extend(Self::calculator());
        all.extend(Self::code_interpreter());
        all.extend(Self::weather_query());
        all.extend(Self::web_search());
        all
    }
}

/// Simple eval runner that prints a report.
pub struct EvalReport {
    pub passed: usize,
    pub failed: usize,
    pub failures: Vec<String>,
}

impl EvalReport {
    pub fn pass_rate(&self) -> f64 {
        let total = self.passed + self.failed;
        if total == 0 {
            0.0
        } else {
            self.passed as f64 / total as f64
        }
    }
}
