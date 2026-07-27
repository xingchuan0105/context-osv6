//! Versioned judge prompt (design §4.2).
//!
//! `SYSTEM_PROMPT` is the strict Chinese RAG evaluator persona plus the fixed
//! scoring rubric (design §4.2 items 1–5). `build_user_prompt` renders one
//! query's payload from a `JudgeInput` and states the exact JSON shape the
//! judge must return. The prompt is versioned via `SCHEMA_VERSION` only —
//! any wording change must bump it.

use super::artifact::JudgeInput;

/// Version tag written into every judge request/response (`schema_version`).
pub const SCHEMA_VERSION: &str = "rag_eval_judge_v2";

/// System prompt: strict Chinese RAG evaluator, JSON-only output, plus the
/// fixed scoring rubric from design §4.2 (items 1–5, including the two 禁止
/// rules).
pub const SYSTEM_PROMPT: &str = "\
你是严格的中文 RAG 评测员。只输出合法 JSON，不要 markdown 围栏。

【评分细则】
1. answer_correctness（答案正确性，0–1 分）
- 与 reference_answer 语义等价即高分：允许改写、空格、同义、合理语序。
- reference_answer 是评分 rubric，不是字面模板。
- verdict=partial：核心事实正确但缺关键限定（例如年份对但主体张冠李戴）。
- 当 expected_should_answer=false：若模型正确拒答，verdict=not_applicable（以 refusal 维度为准）；若仍作答，correctness 给低分。

2. faithfulness（忠实度，0–1 分）
- 只根据给定 context 判定。context_source=cited 表示 context 为答案实际引用的段落；context_source=retrieved_fallback 表示答案未引用任何段落、context 为检索兜底段落（证据链已断，判分需相应从严）。
- 答案中每个实质性事实 claim 必须被 context 支持；数字、日期、专名从严。
- 允许同义改写；不允许 context 中不存在的具体数字/实体。把所有无支持的 claim 列入 unsupported_claims。

3. answer_relevancy（答案相关性，0–1 分）
- 是否在回答所问；文不对题即使「事实正确」也给低分。

4. refusal（拒答判定）
- is_refusal：答案是否为拒答（拒答话术多样化仍算拒答）。
- correct_for_expectation：拒答行为是否符合 expected_should_answer。

5. 禁止
- 不要因「答案未出现某个精确字符串」扣 correctness。
- 不要用训练知识补全；context 没有的事实就判 ungrounded / insufficient。";

/// Exact output shape the judge must return (design §4.2 schema). The
/// `schema_version` the judge echoes is validated against `SCHEMA_VERSION` at
/// parse time.
const OUTPUT_SCHEMA_SHAPE: &str = r#"{
  "schema_version": "rag_eval_judge_v2",
  "refusal": {"is_refusal": true, "correct_for_expectation": true, "score": 0.0, "rationale": "…"},
  "answer_correctness": {"score": 0.0, "verdict": "correct|partial|incorrect|not_applicable", "rationale": "…", "key_points_hit": ["…"], "key_points_missed": ["…"]},
  "faithfulness": {"score": 0.0, "verdict": "grounded|mixed|ungrounded|not_applicable", "unsupported_claims": ["…"], "rationale": "…"},
  "answer_relevancy": {"score": 0.0, "rationale": "…"},
  "context_sufficiency": {"score": 0.0, "verdict": "sufficient|partial|insufficient|unknown", "rationale": "…"}
}"#;

/// Render the per-query user prompt (design §4.2 user-message payload):
/// question, reference answer (as rubric), refusal expectation, model answer,
/// numbered context chunks flagged by `context_source`, optional rubric
/// notes, and the required output shape. Ends with a short restatement of the
/// two 禁止 rules (design allows the rubric in system/user 固定段落; the
/// repetition is deliberate end-of-prompt emphasis).
pub fn build_user_prompt(input: &JudgeInput) -> String {
    let mut p = String::new();
    p.push_str("【问题】\n");
    p.push_str(&input.question);
    p.push_str("\n\n【参考答案（评分 rubric，非字面模板）】\n");
    p.push_str(&input.reference_answer);
    p.push_str("\n\n【expected_should_answer】\n");
    p.push_str(if input.expected_should_answer {
        "true"
    } else {
        "false"
    });
    p.push_str("\n\n【模型答案】\n");
    p.push_str(&input.model_answer);
    p.push_str("\n\n【评测 context（context_source=");
    p.push_str(input.context_source.as_str());
    p.push_str("）】\n");
    if input.cited_context.is_empty() {
        p.push_str("（空）\n");
    } else {
        for (i, chunk) in input.cited_context.iter().enumerate() {
            p.push_str(&format!("[{}] {}\n", i + 1, chunk));
        }
    }
    if let Some(notes) = &input.rubric_notes {
        p.push_str("\n【补充评分约定 rubric_notes】\n");
        p.push_str(notes);
        p.push('\n');
    }
    p.push_str("\n【输出要求】\n只输出一个合法 JSON 对象，不要 markdown 围栏，不要输出解释文字。\nschema_version 固定为 \"");
    p.push_str(SCHEMA_VERSION);
    p.push_str("\"。结构：\n");
    p.push_str(OUTPUT_SCHEMA_SHAPE);
    p.push_str(
        "\n\n【禁止】\n- 不要因「答案未出现某个精确字符串」扣 correctness。\n- 不要用训练知识补全；context 没有的事实就判 ungrounded / insufficient。\n",
    );
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval_v2::artifact::ContextSource;

    fn input(context_source: ContextSource, rubric_notes: Option<&str>) -> JudgeInput {
        JudgeInput {
            question: "Y公司哪一年在大连建厂？".to_string(),
            reference_answer: "Y公司2019年在大连建厂。".to_string(),
            expected_should_answer: true,
            model_answer: "2019 年，Y公司在大连投资建厂。".to_string(),
            cited_context: vec!["Y冷冻设备公司2019年于大连市投资建厂".to_string()],
            context_source,
            rubric_notes: rubric_notes.map(str::to_string),
        }
    }

    #[test]
    fn system_prompt_has_persona_rubric_and_forbidden_rules() {
        assert!(SYSTEM_PROMPT.contains("你是严格的中文 RAG 评测员"));
        assert!(SYSTEM_PROMPT.contains("语义等价"));
        assert!(SYSTEM_PROMPT.contains("retrieved_fallback"));
        // Both 禁止 rules (design §4.2 item 5).
        assert!(SYSTEM_PROMPT.contains("不要因「答案未出现某个精确字符串」扣 correctness"));
        assert!(SYSTEM_PROMPT.contains("不要用训练知识补全"));
    }

    #[test]
    fn builder_renders_all_fields_and_schema_version() {
        let p = build_user_prompt(&input(ContextSource::Cited, Some("接受「2019 年」「2019年」")));
        assert!(p.contains("Y公司哪一年在大连建厂？"));
        assert!(p.contains("Y公司2019年在大连建厂。"));
        assert!(p.contains("expected_should_answer】\ntrue"));
        assert!(p.contains("2019 年，Y公司在大连投资建厂。"));
        assert!(p.contains("context_source=cited"));
        assert!(p.contains("[1] Y冷冻设备公司2019年于大连市投资建厂"));
        assert!(p.contains("接受「2019 年」「2019年」"));
        assert!(p.contains(SCHEMA_VERSION));
        assert!(p.contains("\"answer_correctness\""));
        assert!(p.contains("\"context_sufficiency\""));
        // Closing reminder repeats both 禁止 rules.
        assert!(p.contains("不要因「答案未出现某个精确字符串」扣 correctness"));
        assert!(p.contains("不要用训练知识补全"));
    }

    #[test]
    fn builder_marks_retrieved_fallback() {
        let p = build_user_prompt(&input(ContextSource::RetrievedFallback, None));
        assert!(p.contains("context_source=retrieved_fallback"));
    }

    #[test]
    fn builder_omits_rubric_notes_section_when_none() {
        let p = build_user_prompt(&input(ContextSource::Cited, None));
        assert!(!p.contains("rubric_notes"));
    }
}
