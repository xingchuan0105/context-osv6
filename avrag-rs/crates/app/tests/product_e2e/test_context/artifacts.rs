//! Failure artifacts, observability capture, and mock-server controls.

use std::sync::atomic::Ordering;

use super::super::{
    ChatResponse, StreamReasoningCapture,
    mock_servers::{
        reset_mock_rag_state, set_mock_emit_memory_tool, set_mock_rag_codegen_chunk_id,
        set_mock_rag_codegen_doc_id, set_mock_rag_multiround_profile, set_mock_rag_skill_request_memory,
        set_mock_rag_skip_codegen,
    },
};
use super::TestContext;

impl TestContext {
    pub fn set_mock_rag_chunk_id(&self, chunk_id: &str) {
        let _ = self;
        set_mock_rag_codegen_chunk_id(chunk_id);
    }

    pub fn set_mock_rag_skip_codegen(&self, skip: bool) {
        let _ = self;
        set_mock_rag_skip_codegen(skip);
    }

    pub fn set_mock_rag_multiround_profile(&self, enabled: bool) {
        let _ = self;
        set_mock_rag_multiround_profile(enabled);
    }

    pub fn set_mock_rag_codegen_doc_id(&self, doc_id: &str) {
        let _ = self;
        set_mock_rag_codegen_doc_id(doc_id);
    }

    pub fn set_mock_emit_memory_tool(&self, tool: Option<&str>) {
        let _ = self;
        set_mock_emit_memory_tool(tool.map(str::to_string));
    }

    pub fn set_mock_rag_skill_request_memory(&self, enabled: bool) {
        let _ = self;
        set_mock_rag_skill_request_memory(enabled);
    }

    pub fn reset_mock_state(&self) {
        reset_mock_rag_state();
    }

    pub fn set_search_429(&self, value: bool) {
        if let Some(ref flag) = self.search_should_429 {
            flag.store(value, Ordering::SeqCst);
        }
    }

    pub fn set_embedding_503(&self, value: bool) {
        if let Some(ref flag) = self.embedding_should_503 {
            flag.store(value, Ordering::SeqCst);
        }
    }

    pub fn llm_real_artifact_dir(&self, test_name: &str) -> std::path::PathBuf {
        self.artifact_dir(test_name, "llm_real")
    }

    pub fn save_failure_artifacts(
        &self,
        test_name: &str,
        response_json: Option<&serde_json::Value>,
    ) {
        let out_dir = self.artifact_dir(test_name, "failures");
        let _ = std::fs::create_dir_all(&out_dir);

        if let Some(body) = response_json {
            let _ = std::fs::write(
                out_dir.join("response_body.json"),
                serde_json::to_string_pretty(body).unwrap_or_default(),
            );
        }

        if let Some(ref log_path) = self.worker_log_path {
            if log_path.exists() {
                let _ = std::fs::copy(log_path, out_dir.join("worker_logs.txt"));
            }
        }
    }

    fn write_reasoning_capture_files(
        out_dir: &std::path::Path,
        capture: &StreamReasoningCapture,
    ) {
        let _ = std::fs::write(out_dir.join("reasoning_summary.txt"), &capture.summary);

        let trace_lines: String = capture
            .trace_reasoning
            .iter()
            .filter_map(|rec| serde_json::to_string(rec).ok())
            .collect::<Vec<_>>()
            .join("\n");
        let _ = std::fs::write(out_dir.join("trace_reasoning.jsonl"), trace_lines);

        let _ = std::fs::write(
            out_dir.join("prompt_snapshots.json"),
            serde_json::to_string_pretty(&capture.prompt_snapshots).unwrap_or_default(),
        );
    }

    pub fn save_observability_artifact(
        &self,
        test_name: &str,
        resp: &ChatResponse,
        capture: Option<&StreamReasoningCapture>,
        extra: Option<&serde_json::Value>,
    ) {
        let out_dir = self.artifact_dir(test_name, "observability");
        let _ = std::fs::create_dir_all(&out_dir);

        let _ = std::fs::write(
            out_dir.join("response.json"),
            serde_json::to_string_pretty(resp).unwrap_or_default(),
        );

        if let Some(capture) = capture {
            Self::write_reasoning_capture_files(&out_dir, capture);
        }

        let stream_error_with_done = extra
            .and_then(|v| v.get("stream_error_with_done"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut metadata = serde_json::json!({
            "test_name": test_name,
            "run_id": self.artifact_run_id,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "degrade_trace_count": resp.degrade_trace.len(),
            "usage": resp.usage,
            "citation_count": resp.citations.len(),
            "agent_type": resp.agent_type,
            "session_id": resp.session_id,
            "message_id": resp.message_id,
            "stream_error_with_done": stream_error_with_done,
            "extra": extra.cloned().unwrap_or(serde_json::Value::Null),
        });

        if let Some(capture) = capture {
            let reasoning_empty_warning =
                capture.summary.is_empty() && capture.trace_reasoning.is_empty();
            metadata["reasoning_delta_count"] = serde_json::json!(capture.delta_count);
            metadata["reasoning_summary_chars"] =
                serde_json::json!(capture.summary.chars().count());
            metadata["reasoning_summary_present"] = serde_json::json!(!capture.summary.is_empty());
            metadata["trace_reasoning_count"] = serde_json::json!(capture.trace_reasoning.len());
            metadata["prompt_snapshot_count"] = serde_json::json!(capture.prompt_snapshots.len());
            metadata["reasoning_empty_warning"] = serde_json::json!(reasoning_empty_warning);
        }

        let _ = std::fs::write(
            out_dir.join("metadata.json"),
            serde_json::to_string_pretty(&metadata).unwrap_or_default(),
        );

        if let Some(ref log_path) = self.worker_log_path {
            if log_path.exists() {
                let _ = std::fs::copy(log_path, out_dir.join("worker_logs.txt"));
            }
        }
    }

    pub fn save_llm_artifact(
        &self,
        test_name: &str,
        resp: &ChatResponse,
        extra: Option<serde_json::Value>,
        capture: Option<StreamReasoningCapture>,
    ) {
        let capture = capture.unwrap_or(StreamReasoningCapture {
            summary: String::new(),
            delta_count: 0,
            trace_reasoning: Vec::new(),
            prompt_snapshots: Vec::new(),
        });

        let extra_value = extra.unwrap_or(serde_json::Value::Null);
        self.save_observability_artifact(test_name, resp, Some(&capture), Some(&extra_value));

        let out_dir = self.llm_real_artifact_dir(test_name);
        let _ = std::fs::create_dir_all(&out_dir);

        let _ = std::fs::write(
            out_dir.join("response.json"),
            serde_json::to_string_pretty(resp).unwrap_or_default(),
        );

        Self::write_reasoning_capture_files(&out_dir, &capture);

        let reasoning_empty_warning =
            capture.summary.is_empty() && capture.trace_reasoning.is_empty();
        let stream_error_with_done = extra_value
            .get("stream_error_with_done")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let metadata = serde_json::json!({
            "test_name": test_name,
            "run_id": self.artifact_run_id,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "usage": resp.usage,
            "degrade_trace_count": resp.degrade_trace.len(),
            "citation_count": resp.citations.len(),
            "agent_type": resp.agent_type,
            "session_id": resp.session_id,
            "message_id": resp.message_id,
            "reasoning_delta_count": capture.delta_count,
            "reasoning_summary_chars": capture.summary.chars().count(),
            "reasoning_summary_present": !capture.summary.is_empty(),
            "trace_reasoning_count": capture.trace_reasoning.len(),
            "prompt_snapshot_count": capture.prompt_snapshots.len(),
            "reasoning_empty_warning": reasoning_empty_warning,
            "stream_error_with_done": stream_error_with_done,
            "models": {
                "agent_llm": std::env::var("AGENT_LLM_MODEL").unwrap_or_default(),
                "embedding": std::env::var("EMBEDDING_MODEL").unwrap_or_default(),
            },
            "extra": extra_value,
        });
        let _ = std::fs::write(
            out_dir.join("metadata.json"),
            serde_json::to_string_pretty(&metadata).unwrap_or_default(),
        );

        if let Some(ref log_path) = self.worker_log_path {
            if log_path.exists() {
                let _ = std::fs::copy(log_path, out_dir.join("worker_logs.txt"));
            }
        }
    }

    pub fn assert_tool_called(&self, tool_name: &str) {
        let Some(ref log_path) = self.worker_log_path else {
            eprintln!("[assert_tool_called] no worker log path — skipping");
            return;
        };
        let content = match std::fs::read_to_string(log_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[assert_tool_called] cannot read worker log: {e}");
                return;
            }
        };

        let found = content.contains(&format!("\"tool\":\"{tool_name}\""))
            || content.contains(&format!("\"tool\": \"{tool_name}\""))
            || content.contains(&format!("tool={tool_name}"))
            || content.contains(tool_name);

        if !found {
            eprintln!(
                "[assert_tool_called] WARNING: no evidence of '{tool_name}' in worker log. \
                 (RAG tool calls run in the HTTP server, not the worker — this is expected.)"
            );
        }
    }

    fn artifact_dir(&self, test_name: &str, bucket: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("e2e_output")
            .join(bucket)
            .join(&self.artifact_run_id)
            .join(test_name)
    }
}
