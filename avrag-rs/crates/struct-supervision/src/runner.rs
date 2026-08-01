//! 薄 loop runner：LLM tool-calling 回合 + 预算降级（对齐 `supervise.supervise`）。

use std::path::PathBuf;

use avrag_llm::{ChatMessage, LlmClient, LlmResponse};
use contracts::ToolSpec;

use crate::session::{FinalState, Session};
use crate::store::{build_metas, write_duckdb};

/// 监督运行配置。
#[derive(Debug, Clone)]
pub struct SuperviseConfig {
    pub max_turns: usize,
    pub doc_name: String,
    pub out_path: PathBuf,
    pub report_path: Option<PathBuf>,
}

/// 运行报告（形状对齐 `supervise.supervise` 返回 dict）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SuperviseReport {
    pub doc: String,
    pub duckdb: String,
    pub turns: usize,
    pub done_summary: Option<String>,
    pub budget_exhausted: bool,
    pub tables: serde_json::Value,
    pub log: Vec<(String, serde_json::Value, String)>,
    /// 表级证据 chunk（`write_duckdb` 产出；ingestion 挂接时随报告带出直接入库，
    /// 不再回读 sidecar）。serde default 仅为兼容既有 JSON 反序列化。
    #[serde(default)]
    pub evidence: Vec<crate::store::EvidenceChunk>,
}

/// LLM 抽象（可注入 mock 测试）。
#[async_trait::async_trait]
pub trait SupervisorLlm: Send + Sync {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolSpec],
    ) -> anyhow::Result<LlmResponse>;
}

#[async_trait::async_trait]
impl SupervisorLlm for LlmClient {
    async fn complete(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolSpec],
    ) -> anyhow::Result<LlmResponse> {
        self.complete_with_tools(messages, tools, Some(0.7)).await
    }
}

/// assistant 消息（含 OpenAI 形状 tool_calls；合成 call_id 与 tool 回合配对）。
fn build_assistant_message(resp: &LlmResponse, call_ids: &[String]) -> ChatMessage {
    let openai_calls: Vec<serde_json::Value> = resp
        .tool_calls()
        .unwrap_or_default()
        .iter()
        .zip(call_ids.iter())
        .map(|(call, id)| {
            serde_json::json!({
                "id": id,
                "type": "function",
                "function": {
                    "name": call.tool,
                    "arguments": serde_json::to_string(&call.args).unwrap_or_else(|_| "{}".into()),
                }
            })
        })
        .collect();
    ChatMessage {
        role: "assistant".to_string(),
        content: resp.content.clone(),
        multimodal_content: None,
        name: None,
        tool_call_id: None,
        tool_calls: Some(serde_json::json!(openai_calls)),
        reasoning_content: resp.reasoning_content.clone(),
    }
}

fn build_tool_message(call_id: &str, result: &str) -> ChatMessage {
    ChatMessage {
        role: "tool".to_string(),
        content: result.to_string(),
        multimodal_content: None,
        name: None,
        tool_call_id: Some(call_id.to_string()),
        tool_calls: None,
        reasoning_content: None,
    }
}

/// 监督主入口：简报 → loop → 兜底终态 → 落库 + sidecar + 报告。
pub async fn supervise(
    input: &crate::SuperviseInput,
    llm: &dyn SupervisorLlm,
    cfg: &SuperviseConfig,
) -> anyhow::Result<SuperviseReport> {
    let mut session = Session::new(input)?;
    let mut messages = vec![
        ChatMessage::system(crate::prompts::system_prompt()),
        ChatMessage::user(session.briefing(&cfg.doc_name)),
    ];
    let mut turns = 0usize;
    let mut done_summary: Option<String> = None;
    let mut log: Vec<(String, serde_json::Value, String)> = Vec::new();

    while turns < cfg.max_turns && done_summary.is_none() {
        turns += 1;
        let resp = llm.complete(&messages, &crate::tools::specs()).await?;
        let calls: Vec<contracts::ToolCall> = resp.tool_calls().unwrap_or_default().to_vec();
        let call_ids: Vec<String> = (0..calls.len()).map(|i| format!("call_{i}")).collect();
        messages.push(build_assistant_message(&resp, &call_ids));

        if calls.is_empty() {
            // 模型未调工具：以观察提示当前未完成表数（第三人称）
            let un = session.unfinished();
            if !un.is_empty() {
                messages.push(ChatMessage::user(format!(
                    "本轮未发生工具调用。仍处于未终态的表:{un:?}(共 {} 张)。",
                    un.len()
                )));
            }
            continue;
        }
        for (idx, call) in calls.iter().enumerate() {
            match crate::tools::dispatch(&mut session, call, &mut log) {
                Ok(Some(summary)) => {
                    done_summary = Some(summary);
                    messages.push(build_tool_message(&call_ids[idx], "监督结束。"));
                }
                Ok(None) => {
                    let out = log.last().map(|(_, _, o)| o.clone()).unwrap_or_default();
                    messages.push(build_tool_message(&call_ids[idx], &out));
                }
                Err(e) => {
                    log.push((call.tool.clone(), call.args.clone(), e.clone()));
                    messages.push(build_tool_message(&call_ids[idx], &e));
                }
            }
        }
        // 进度观察：至少一次工具调用后每 8 轮提示（对齐 supervise.py:333）
        if done_summary.is_none()
            && !session.unfinished().is_empty()
            && !log.is_empty()
            && turns % 8 == 0
        {
            let un = session.unfinished();
            messages.push(ChatMessage::user(format!(
                "进度观察:已进行 {turns} 轮;仍未终态的表:{un:?}。"
            )));
        }
    }
    finish(input, session, cfg, turns, done_summary, log)
}

/// loop 结束后的收尾：兜底终态 + 落库 + sidecar + 报告（与 `supervise.supervise` 尾部对齐）。
fn finish(
    input: &crate::SuperviseInput,
    mut session: Session,
    cfg: &SuperviseConfig,
    turns: usize,
    done_summary: Option<String>,
    log: Vec<(String, serde_json::Value, String)>,
) -> anyhow::Result<SuperviseReport> {
    // 兜底终态：未处理表保持确定性初态并附说明（pipeline 不被 LLM 卡死）
    for tid in session.unfinished() {
        session.finals.insert(
            tid.clone(),
            FinalState {
                table_id: tid.clone(),
                confidence: Some("low".into()),
                table_kind: None,
                excluded: false,
                reason: Some("supervision_incomplete".into()),
                notes_add: Some(vec!["supervision_incomplete".into()]),
                ..Default::default()
            },
        );
    }

    let metas = build_metas(&session.grids, &session.reports, &session.finals);
    let evidence = write_duckdb(&session.grids, &metas, &cfg.out_path)?;
    let sidecar = cfg.out_path.with_extension("duckdb.evidence.json");
    let _ = std::fs::write(
        &sidecar,
        serde_json::to_string_pretty(&serde_json::json!({
            "doc_id": input.doc_id.clone().unwrap_or_default(),
            "chunks": evidence,
        }))
        .unwrap_or_default(),
    );

    let tables: serde_json::Value = {
        let mut m = serde_json::Map::new();
        let all: std::collections::BTreeSet<String> = session
            .reports
            .keys()
            .chain(session.finals.keys())
            .cloned()
            .collect();
        for tid in all {
            m.insert(
                tid.clone(),
                serde_json::json!({
                    "final": session.finals.get(&tid).cloned().unwrap_or_default(),
                    "status": session.reports.get(&tid).map(|r| r.status.clone())
                        .unwrap_or_else(|| "merged/quarantined".into()),
                }),
            );
        }
        serde_json::Value::Object(m)
    };

    let budget_exhausted = done_summary.is_none();
    let report = SuperviseReport {
        doc: cfg.doc_name.clone(),
        duckdb: cfg.out_path.display().to_string(),
        turns,
        done_summary,
        budget_exhausted,
        tables,
        log,
        evidence,
    };
    if let Some(path) = &cfg.report_path {
        let _ = std::fs::write(
            path,
            serde_json::to_string_pretty(&report).unwrap_or_default(),
        );
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Grid, Row, SuperviseInput};

    fn row(line: usize, cells: &[&str]) -> Row {
        Row {
            line,
            cells: cells.iter().map(|c| c.to_string()).collect(),
        }
    }

    fn fixture() -> SuperviseInput {
        let text = ["| h |", "| --- |", "| a |"].join("\n");
        let grids = vec![Grid {
            start_line: 1,
            rows: vec![row(1, &["h"]), row(3, &["a"])],
            notes: vec![],
        }];
        SuperviseInput {
            doc_id: Some("mock".into()),
            source_text: text,
            grids,
        }
    }

    /// 脚本化假 LLM：按轮返回预设响应。
    struct FakeLlm {
        responses: std::sync::Mutex<std::collections::VecDeque<LlmResponse>>,
    }

    impl FakeLlm {
        fn tool_call(tool: &str, args: serde_json::Value) -> LlmResponse {
            LlmResponse {
                content: String::new(),
                reasoning_content: None,
                usage: avrag_llm::LlmUsage {
                    prompt_tokens: 1,
                    completion_tokens: 1,
                    total_tokens: 2,
                    provider: "mock".into(),
                    model: "mock".into(),
                    cached_tokens: 0,
                    reasoning_tokens: 0,
                },
                model: "mock".into(),
                tool_calls: Some(vec![contracts::ToolCall {
                    tool: tool.into(),
                    version: "v1".into(),
                    args,
                }]),
            }
        }

        fn no_tool() -> LlmResponse {
            LlmResponse {
                content: "继续".into(),
                reasoning_content: None,
                usage: avrag_llm::LlmUsage {
                    prompt_tokens: 1,
                    completion_tokens: 1,
                    total_tokens: 2,
                    provider: "mock".into(),
                    model: "mock".into(),
                    cached_tokens: 0,
                    reasoning_tokens: 0,
                },
                model: "mock".into(),
                tool_calls: None,
            }
        }
    }

    #[async_trait::async_trait]
    impl SupervisorLlm for FakeLlm {
        async fn complete(
            &self,
            _messages: &[ChatMessage],
            _tools: &[ToolSpec],
        ) -> anyhow::Result<LlmResponse> {
            let mut q = self.responses.lock().unwrap();
            Ok(q.pop_front().unwrap_or_else(FakeLlm::no_tool))
        }
    }

    fn cfg() -> SuperviseConfig {
        SuperviseConfig {
            max_turns: 10,
            doc_name: "mock.md".into(),
            out_path: std::env::temp_dir().join(format!("sup_run_{}.duckdb", uuid::Uuid::new_v4())),
            report_path: None,
        }
    }

    #[tokio::test]
    async fn loop_terminates_on_done() {
        let llm = FakeLlm {
            responses: std::sync::Mutex::new(
                [
                    FakeLlm::tool_call(
                        "quarantine",
                        serde_json::json!({"table_id": "t0", "reason": "mock"}),
                    ),
                    FakeLlm::tool_call("done", serde_json::json!({"summary": "完成"})),
                ]
                .into(),
            ),
        };
        let rep = supervise(&fixture(), &llm, &cfg()).await.unwrap();
        assert_eq!(rep.turns, 2);
        assert_eq!(rep.done_summary.as_deref(), Some("完成"));
        assert!(!rep.budget_exhausted);
        assert_eq!(
            rep.tables["t0"]["final"]["excluded"],
            serde_json::json!(true)
        );
    }

    #[tokio::test]
    async fn budget_exhaustion_falls_back_to_low() {
        let llm = FakeLlm {
            responses: std::sync::Mutex::new(std::collections::VecDeque::new()),
        };
        let mut c = cfg();
        c.max_turns = 3;
        let rep = supervise(&fixture(), &llm, &c).await.unwrap();
        assert!(rep.budget_exhausted);
        assert_eq!(rep.turns, 3);
        // 兜底终态：low + supervision_incomplete，非 excluded
        let f = &rep.tables["t0"]["final"];
        assert_eq!(f["confidence"], serde_json::json!("low"));
        assert_eq!(f["reason"], serde_json::json!("supervision_incomplete"));
    }

    #[tokio::test]
    async fn unknown_tool_returns_error_observation() {
        let llm = FakeLlm {
            responses: std::sync::Mutex::new(
                [
                    FakeLlm::tool_call("nope", serde_json::json!({})),
                    FakeLlm::tool_call("done", serde_json::json!({"summary": "x"})),
                ]
                .into(),
            ),
        };
        let rep = supervise(&fixture(), &llm, &cfg()).await.unwrap();
        assert_eq!(rep.turns, 2);
        assert!(
            rep.log
                .iter()
                .any(|(t, _, o)| t == "nope" || o.contains("未知工具"))
        );
    }
}
