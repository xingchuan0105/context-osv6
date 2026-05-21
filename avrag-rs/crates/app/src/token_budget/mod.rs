//! TokenBudgetSimulator — offline token-consumption analysis for development.
//!
//! Given representative queries, simulates each agent mode's full execution
//! pipeline and produces a precise per-stage token breakdown using tiktoken.

use avrag_llm::count_tokens;
use serde::Serialize;

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct StageEstimate {
    pub stage: String,
    pub iteration: u8,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SimulationResult {
    pub scenario_name: String,
    pub mode: String,
    pub total_prompt_tokens: usize,
    pub total_completion_tokens: usize,
    pub total_tokens: usize,
    pub stages: Vec<StageEstimate>,
}

#[derive(Debug, Clone)]
pub struct Scenario {
    pub name: &'static str,
    pub mode: &'static str,
    pub query: &'static str,
    pub history: Vec<(&'static str, &'static str)>,
    pub session_summary: Option<&'static str>,
    pub user_preferences: Option<serde_json::Value>,
    /// Simulated search results (title + snippet).
    pub search_results: Vec<(&'static str, &'static str)>,
    /// Simulated RAG chunks (text).
    pub rag_chunks: Vec<&'static str>,
}

// ---------------------------------------------------------------------------
// Default scenario catalogue
// ---------------------------------------------------------------------------

pub fn default_scenarios() -> Vec<Scenario> {
    vec![
        // --- Chat ---
        Scenario {
            name: "chat_simple_cn",
            mode: "chat",
            query: "你好",
            history: vec![],
            session_summary: None,
            user_preferences: None,
            search_results: vec![],
            rag_chunks: vec![],
        },
        Scenario {
            name: "chat_medium_cn",
            mode: "chat",
            query: "请总结量子计算的基本原理和应用场景",
            history: vec![
                ("user", "什么是量子比特？"),
                ("assistant", "量子比特是量子计算的基本单位..."),
            ],
            session_summary: Some("User is learning quantum computing basics."),
            user_preferences: Some(serde_json::json!({"style": "concise"})),
            search_results: vec![],
            rag_chunks: vec![],
        },
        Scenario {
            name: "chat_complex_en",
            mode: "chat",
            query: "Compare the memory safety guarantees of Rust, Swift, and ATS, focusing on how each language handles dangling pointers and use-after-free. Provide concrete code examples.",
            history: vec![],
            session_summary: None,
            user_preferences: None,
            search_results: vec![],
            rag_chunks: vec![],
        },
        // --- Search ---
        Scenario {
            name: "search_simple",
            mode: "search",
            query: "Rust async runtime comparison",
            history: vec![],
            session_summary: None,
            user_preferences: None,
            search_results: vec![
                ("Tokio vs async-std", "Tokio is the most widely used async runtime in Rust..."),
                ("Rust Async Book", "The async book covers the fundamentals of async/await in Rust..."),
                ("Comparing Rust Runtimes", "A detailed benchmark comparing Tokio, async-std, and smol..."),
            ],
            rag_chunks: vec![],
        },
        Scenario {
            name: "search_complex",
            mode: "search",
            query: "2026年最新的大语言模型推理优化技术有哪些？对比 DeepSeek、Qwen 和 Gemini 的推理架构差异",
            history: vec![],
            session_summary: Some("User works on LLM inference optimization."),
            user_preferences: Some(serde_json::json!({"style": "detailed", "language": "zh"})),
            search_results: vec![
                ("DeepSeek V4 推理优化", "DeepSeek V4 introduces speculative decoding with tree attention..."),
                ("Qwen3 技术报告", "Qwen3 employs a mixture-of-experts architecture with 128 experts..."),
                ("Gemini 3.5 Flash 架构", "Gemini 3.5 Flash uses a novel attention mechanism called multi-query..."),
                ("LLM 推理优化综述 2026", "A comprehensive survey covering quantization, pruning, distillation..."),
                ("Speculative Decoding Survey", "Speculative decoding has become the standard for latency reduction..."),
            ],
            rag_chunks: vec![],
        },
        // --- RAG ---
        Scenario {
            name: "rag_simple",
            mode: "rag",
            query: "这份合同中的违约责任条款是什么？",
            history: vec![],
            session_summary: None,
            user_preferences: None,
            search_results: vec![],
            rag_chunks: vec![
                "第七条 违约责任\n7.1 任何一方违反本合同约定，应当向守约方承担违约责任。",
                "7.2 违约金计算方式：按合同总金额的10%计算。",
                "7.3 因不可抗力导致无法履行合同的，双方均不承担违约责任。",
            ],
        },
        Scenario {
            name: "rag_complex",
            mode: "rag",
            query: "分析这份技术方案中数据库架构的风险点，并提出优化建议。重点关注高可用性、数据一致性和扩展性。",
            history: vec![
                ("user", "请先概述整体架构"),
                ("assistant", "该方案采用微服务架构，数据库层使用 PostgreSQL 主从复制..."),
            ],
            session_summary: Some("User is reviewing a technical architecture document."),
            user_preferences: Some(serde_json::json!({"style": "structured", "expertise": "senior engineer"})),
            search_results: vec![],
            rag_chunks: vec![
                "数据库架构设计\n本文档描述了一套基于 PostgreSQL 的高可用数据库架构。",
                "3.1 主从复制\n采用流复制（Streaming Replication）机制，主节点写入，从节点读取。",
                "3.2 故障切换\n使用 Patroni + etcd 实现自动故障检测和主从切换，RTO < 30s。",
                "3.3 数据一致性\n同步复制模式确保主从数据强一致，但会增加写入延迟约 5-10ms。",
                "3.4 扩展性\n通过 Citus 实现分片扩展，支持水平扩展到 100+ 节点。",
                "3.5 风险分析\n主要风险包括：脑裂场景、网络分区时的数据不一致、以及分片键选择不当导致的查询热点。",
                "4.1 优化建议\n建议引入读写分离中间件（如 PgPool-II），并考虑使用逻辑复制替代物理复制以提高灵活性。",
            ],
        },
    ]
}

// ---------------------------------------------------------------------------
// Simulation engine
// ---------------------------------------------------------------------------

pub fn simulate_all() -> Vec<SimulationResult> {
    default_scenarios()
        .into_iter()
        .map(|s| simulate_scenario(&s))
        .collect()
}

pub fn simulate_scenario(scenario: &Scenario) -> SimulationResult {
    match scenario.mode {
        "chat" => simulate_chat(scenario),
        "search" => simulate_search(scenario),
        "rag" => simulate_rag(scenario),
        _ => SimulationResult {
            scenario_name: scenario.name.to_string(),
            mode: scenario.mode.to_string(),
            total_prompt_tokens: 0,
            total_completion_tokens: 0,
            total_tokens: 0,
            stages: vec![],
        },
    }
}

fn simulate_chat(scenario: &Scenario) -> SimulationResult {
    let mut stages = Vec::new();

    // Build system prompt (same logic as ChatAgent)
    let mut system = String::from(
        "You are a direct chat assistant. Answer from the current conversation and general knowledge only. Do not invent document or web citations; if the user asks for document evidence or fresh web facts, explain that they should switch to RAG or WebSearch mode.",
    );
    if let Some(summary) = scenario.session_summary {
        system.push_str("\n\nSession summary:\n");
        system.push_str(summary);
    }
    if let Some(prefs) = scenario.user_preferences.as_ref() {
        system.push_str("\n\nUser preferences:\n");
        system.push_str(&prefs.to_string());
    }

    // Build messages
    let mut messages_text = system.clone();
    messages_text.push_str("\n\n[system]\n");
    for (role, content) in &scenario.history {
        messages_text.push_str(&format!("{}: {}\n", role, content));
    }
    messages_text.push_str(&format!("user: {}", scenario.query));

    let prompt_tokens = count_tokens(&messages_text);
    let completion_tokens = estimate_completion_for_query(scenario.query);

    stages.push(StageEstimate {
        stage: "llm_chat".to_string(),
        iteration: 0,
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens + completion_tokens,
    });

    let total_prompt: usize = stages.iter().map(|s| s.prompt_tokens).sum();
    let total_completion: usize = stages.iter().map(|s| s.completion_tokens).sum();

    SimulationResult {
        scenario_name: scenario.name.to_string(),
        mode: "chat".to_string(),
        total_prompt_tokens: total_prompt,
        total_completion_tokens: total_completion,
        total_tokens: total_prompt + total_completion,
        stages,
    }
}

fn simulate_search(scenario: &Scenario) -> SimulationResult {
    let mut stages = Vec::new();

    // --- Iteration 0: search + evaluator + (if needed) iteration 1 ---
    // Search provider (Brave) usually returns LLM Context which is provider-side,
    // not counted in our LLM tokens.  We count only our LLM calls:
    //   - evaluator (per iteration)
    //   - final synthesizer

    let iterations: u8 = if scenario.search_results.len() > 3 { 2 } else { 1 };

    for iter in 0..iterations {
        // Evaluator prompt: system + query + sub_queries + result metadata
        let eval_system = include_str!("../../../../prompts/skills/search-eval/SKILL.md");
        let eval_prompt = format!(
            "User question: {}\n\nSub-queries: {}\n\nResults this iteration: {}\nAccumulated results: {}\nIteration: {}",
            scenario.query,
            scenario.query, // simplified: query as sub-query
            scenario.search_results.len(),
            scenario.search_results.len(),
            iter
        );
        let eval_prompt_tokens = count_tokens(eval_system) + count_tokens(&eval_prompt);
        let eval_completion_tokens = 180; // JSON evaluation output

        stages.push(StageEstimate {
            stage: "evaluator".to_string(),
            iteration: iter,
            prompt_tokens: eval_prompt_tokens,
            completion_tokens: eval_completion_tokens,
            total_tokens: eval_prompt_tokens + eval_completion_tokens,
        });
    }

    // Final synthesizer
    let synth_system = "Answer the user's web-search question using only the provided Brave LLM Context evidence. Cite sources with [[n]] markers that match the evidence ids. If the evidence is insufficient, say so plainly.";
    let mut evidence = String::new();
    for (i, (title, snippet)) in scenario.search_results.iter().enumerate() {
        evidence.push_str(&format!(
            "[[{}]] title: {}\nurl: https://example.com/{}\nsnippet:\n{}\n\n",
            i + 1,
            title,
            i,
            snippet
        ));
    }
    let synth_prompt = format!("Question:\n{}\n\nBrave LLM Context evidence:\n{}", scenario.query, evidence);
    let synth_prompt_tokens = count_tokens(synth_system) + count_tokens(&synth_prompt);
    let synth_completion_tokens = estimate_completion_for_query(scenario.query);

    stages.push(StageEstimate {
        stage: "synthesizer".to_string(),
        iteration: iterations.saturating_sub(1),
        prompt_tokens: synth_prompt_tokens,
        completion_tokens: synth_completion_tokens,
        total_tokens: synth_prompt_tokens + synth_completion_tokens,
    });

    let total_prompt: usize = stages.iter().map(|s| s.prompt_tokens).sum();
    let total_completion: usize = stages.iter().map(|s| s.completion_tokens).sum();

    SimulationResult {
        scenario_name: scenario.name.to_string(),
        mode: "search".to_string(),
        total_prompt_tokens: total_prompt,
        total_completion_tokens: total_completion,
        total_tokens: total_prompt + total_completion,
        stages,
    }
}

fn simulate_rag(scenario: &Scenario) -> SimulationResult {
    let mut stages = Vec::new();

    let iterations: u8 = 3; // max ReAct iterations for RAG

    for iter in 0..iterations {
        // --- Planner ---
        let plan_system = include_str!("../../../../prompts/skills/rag-plan/SKILL.md");
        let mut plan_user = format!("Query: {}\n", scenario.query);
        if !scenario.history.is_empty() {
            plan_user.push_str("Conversation history:\n");
            for (role, content) in &scenario.history {
                plan_user.push_str(&format!("{}: {}\n", role, content));
            }
        }
        plan_user.push_str(&format!("\n[iteration]: {}\n", iter));

        // Add memory to planner system (same as recent change)
        let mut plan_system_text = plan_system.to_string();
        if let Some(summary) = scenario.session_summary {
            plan_system_text.push_str("\n\nSession summary:\n");
            plan_system_text.push_str(summary);
        }
        if let Some(prefs) = scenario.user_preferences.as_ref() {
            plan_system_text.push_str("\n\nUser preferences:\n");
            plan_system_text.push_str(&prefs.to_string());
        }

        let plan_prompt_tokens = count_tokens(&plan_system_text) + count_tokens(&plan_user);
        let plan_completion_tokens = 250; // JSON tool-call plan

        stages.push(StageEstimate {
            stage: "planner".to_string(),
            iteration: iter,
            prompt_tokens: plan_prompt_tokens,
            completion_tokens: plan_completion_tokens,
            total_tokens: plan_prompt_tokens + plan_completion_tokens,
        });

        // --- Retrieval context (not an LLM call, but contributes to synthesizer prompt) ---
        // We record it separately so the report shows context size.
        let retrieval_tokens: usize = scenario.rag_chunks.iter().map(|c| count_tokens(c)).sum();
        stages.push(StageEstimate {
            stage: "retrieval_context".to_string(),
            iteration: iter,
            prompt_tokens: retrieval_tokens,
            completion_tokens: 0,
            total_tokens: retrieval_tokens,
        });

        // --- Evaluator ---
        let eval_system = include_str!("../../../../prompts/skills/rag-eval/SKILL.md");
        let sub_queries = scenario.query; // simplified
        let eval_prompt = format!(
            "Original query: {}\n\nSub-queries: {}\n\nRetrieval results: {} chunks\nAccumulated unique chunks: {}\nIteration: {}",
            scenario.query,
            sub_queries,
            scenario.rag_chunks.len(),
            scenario.rag_chunks.len(),
            iter
        );
        let eval_prompt_tokens = count_tokens(eval_system) + count_tokens(&eval_prompt);
        let eval_completion_tokens = 150;

        stages.push(StageEstimate {
            stage: "evaluator".to_string(),
            iteration: iter,
            prompt_tokens: eval_prompt_tokens,
            completion_tokens: eval_completion_tokens,
            total_tokens: eval_prompt_tokens + eval_completion_tokens,
        });
    }

    // --- Final synthesizer ---
    let synth_system = "You are a grounded answer agent.";
    let mut context = String::new();
    for (i, chunk) in scenario.rag_chunks.iter().enumerate() {
        context.push_str(&format!("[{}] {}\n\n", i + 1, chunk));
    }
    let mut history_text = String::new();
    for (role, content) in &scenario.history {
        history_text.push_str(&format!("{}: {}\n", role, content));
    }
    let synth_prompt = format!(
        "Question: {}\n\nHistory:\n{}\n\nRetrieved context:\n{}",
        scenario.query, history_text, context
    );
    let synth_prompt_tokens = count_tokens(synth_system) + count_tokens(&synth_prompt);
    let synth_completion_tokens = estimate_completion_for_query(scenario.query);

    stages.push(StageEstimate {
        stage: "synthesizer".to_string(),
        iteration: iterations.saturating_sub(1),
        prompt_tokens: synth_prompt_tokens,
        completion_tokens: synth_completion_tokens,
        total_tokens: synth_prompt_tokens + synth_completion_tokens,
    });

    let total_prompt: usize = stages.iter().map(|s| s.prompt_tokens).sum();
    let total_completion: usize = stages.iter().map(|s| s.completion_tokens).sum();

    SimulationResult {
        scenario_name: scenario.name.to_string(),
        mode: "rag".to_string(),
        total_prompt_tokens: total_prompt,
        total_completion_tokens: total_completion,
        total_tokens: total_prompt + total_completion,
        stages,
    }
}

// ---------------------------------------------------------------------------
// Completion estimation heuristic
// ---------------------------------------------------------------------------

fn estimate_completion_for_query(query: &str) -> usize {
    let query_tokens = count_tokens(query);
    if query_tokens < 5 {
        200 // Simple greeting -> short answer
    } else if query_tokens < 20 {
        400 // Medium question
    } else if query_tokens < 50 {
        800 // Complex question
    } else {
        1200 // Very complex / multi-part
    }
}

// ---------------------------------------------------------------------------
// Report formatting
// ---------------------------------------------------------------------------

pub fn print_report(results: &[SimulationResult]) {
    println!("\n{:=^80}", " Token Budget Simulation Report ");
    println!();

    for r in results {
        println!("Scenario: {:<30} | Mode: {:<8}", r.scenario_name, r.mode);
        println!("  Total prompt:     {:>6} tokens", r.total_prompt_tokens);
        println!("  Total completion: {:>6} tokens", r.total_completion_tokens);
        println!("  Total:            {:>6} tokens", r.total_tokens);
        println!("  Breakdown:");
        for s in &r.stages {
            if s.iteration > 0 && s.stage != "retrieval_context" {
                println!(
                    "    {:>20} [iter {}]  prompt={:>5}  completion={:>5}  total={:>5}",
                    s.stage, s.iteration, s.prompt_tokens, s.completion_tokens, s.total_tokens
                );
            } else {
                println!(
                    "    {:>20}           prompt={:>5}  completion={:>5}  total={:>5}",
                    s.stage, s.prompt_tokens, s.completion_tokens, s.total_tokens
                );
            }
        }
        println!();
    }

    // Summary table
    println!("{:-^80}", " Summary ");
    println!("{:<25} {:>10} {:>10} {:>10}", "Scenario", "Prompt", "Completion", "Total");
    println!("{}", "-".repeat(60));
    for r in results {
        println!(
            "{:<25} {:>10} {:>10} {:>10}",
            r.scenario_name, r.total_prompt_tokens, r.total_completion_tokens, r.total_tokens
        );
    }
    println!("\n");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulate_chat_simple() {
        let scenarios = default_scenarios();
        let chat = scenarios.iter().find(|s| s.name == "chat_simple_cn").unwrap();
        let result = simulate_scenario(chat);
        assert_eq!(result.mode, "chat");
        assert!(result.total_prompt_tokens > 0);
        assert!(result.total_completion_tokens > 0);
        assert_eq!(result.stages.len(), 1);
    }

    #[test]
    fn simulate_search_with_memory() {
        let scenarios = default_scenarios();
        let search = scenarios.iter().find(|s| s.name == "search_complex").unwrap();
        let result = simulate_scenario(search);
        assert_eq!(result.mode, "search");
        // evaluator (2 iterations) + synthesizer = 3 stages
        assert!(result.stages.len() >= 3);
        assert!(result.total_prompt_tokens > 0);
    }

    #[test]
    fn simulate_rag_multi_iteration() {
        let scenarios = default_scenarios();
        let rag = scenarios.iter().find(|s| s.name == "rag_complex").unwrap();
        let result = simulate_scenario(rag);
        assert_eq!(result.mode, "rag");
        // 3 iterations × (planner + retrieval + evaluator) + synthesizer
        assert!(result.stages.len() >= 9);
        assert!(result.total_prompt_tokens > 500);
    }

    #[test]
    fn all_scenarios_run_without_panic() {
        let results = simulate_all();
        assert_eq!(results.len(), default_scenarios().len());
        for r in &results {
            assert!(r.total_tokens > 0, "{} should have >0 tokens", r.scenario_name);
        }
    }

    #[test]
    fn report_prints_without_panic() {
        let results = simulate_all();
        print_report(&results);
    }

    #[test]
    fn rag_is_most_expensive() {
        let results = simulate_all();
        let rag_complex = results.iter().find(|r| r.scenario_name == "rag_complex").unwrap();
        let chat_simple = results.iter().find(|r| r.scenario_name == "chat_simple_cn").unwrap();
        assert!(
            rag_complex.total_tokens > chat_simple.total_tokens * 5,
            "RAG complex should be much more expensive than simple chat"
        );
    }

    /// Estimate a typical single-user session for each mode.
    #[test]
    fn typical_user_single_session_estimate() {
        // --- Measure actual system-prompt sizes ---
        let chat_system = "You are a direct chat assistant. Answer from the current conversation and general knowledge only. Do not invent document or web citations; if the user asks for document evidence or fresh web facts, explain that they should switch to RAG or WebSearch mode.";
        let rag_plan_sys = include_str!("../../../../prompts/skills/rag-plan/SKILL.md");
        let rag_eval_sys = include_str!("../../../../prompts/skills/rag-eval/SKILL.md");
        let search_eval_sys = include_str!("../../../../prompts/skills/search-eval/SKILL.md");

        let chat_system_tokens = count_tokens(chat_system);
        let rag_plan_sys_tokens = count_tokens(rag_plan_sys);
        let rag_eval_sys_tokens = count_tokens(rag_eval_sys);
        let search_eval_sys_tokens = count_tokens(search_eval_sys);

        // --- Typical parameters ---
        let typical_query = "帮我分析这份合同里的风险条款，并给出修改建议";
        let typical_query_tokens = count_tokens(typical_query);

        // History: 4 turns (2 user + 2 assistant), ~80 tokens each
        let history_tokens_per_turn = 80;
        let history_turns = 4;
        let typical_history_tokens = history_turns * history_tokens_per_turn;

        // Memory
        let typical_summary = "User is reviewing legal contracts. Prefers concise answers.";
        let typical_prefs = serde_json::json!({"style": "concise", "language": "zh"});
        let memory_tokens = count_tokens(typical_summary) + count_tokens(&typical_prefs.to_string());

        // RAG chunks: 8 chunks, ~300 tokens each
        let chunks_count = 8;
        let chunk_tokens = 300;
        let retrieval_tokens = chunks_count * chunk_tokens;

        // Search results: 4 results, ~150 tokens each (title+url+snippet)
        let search_results_count = 4;
        let search_result_tokens = 150;
        let search_evidence_tokens = search_results_count * search_result_tokens;

        // --- Chat estimate ---
        let chat_prompt = chat_system_tokens
            + memory_tokens
            + typical_history_tokens
            + typical_query_tokens;
        let chat_completion = estimate_completion_for_query(typical_query);
        let chat_total = chat_prompt + chat_completion;

        // --- Search estimate (1 iteration + synthesizer) ---
        // Evaluator: system + query + sub_queries + result metadata
        let search_eval_prompt = search_eval_sys_tokens
            + typical_query_tokens
            + 50 // sub_queries metadata
            + 50; // result stats
        let search_eval_completion = 180;
        // Synthesizer: system + query + evidence
        let search_synth_prompt = 50 // synth system
            + typical_query_tokens
            + search_evidence_tokens;
        let search_synth_completion = 600;
        let search_total_prompt = search_eval_prompt + search_synth_prompt;
        let search_total_completion = search_eval_completion + search_synth_completion;
        let search_total = search_total_prompt + search_total_completion;

        // --- RAG estimate (3 iterations) ---
        // Planner (per iteration): system + query + history + iteration annotation
        let rag_plan_prompt_per_iter = rag_plan_sys_tokens
            + typical_query_tokens
            + typical_history_tokens
            + 50; // iteration annotation
        let rag_plan_completion_per_iter = 200;

        // Evaluator (per iteration): system + query + sub_queries + chunk stats
        let rag_eval_prompt_per_iter = rag_eval_sys_tokens
            + typical_query_tokens
            + 100 // sub_queries + stats
            + 50; // iteration info
        let rag_eval_completion_per_iter = 150;

        // Retrieval context (not LLM call, but in synthesizer prompt)
        // Synthesizer: system + query + history + all chunks
        let rag_synth_prompt = 20 // synth system
            + typical_query_tokens
            + typical_history_tokens
            + retrieval_tokens;
        let rag_synth_completion = 800;

        let rag_iterations = 3;
        let rag_total_prompt = rag_iterations * (rag_plan_prompt_per_iter + rag_eval_prompt_per_iter)
            + rag_synth_prompt;
        let rag_total_completion = rag_iterations * (rag_plan_completion_per_iter + rag_eval_completion_per_iter)
            + rag_synth_completion;
        let rag_total = rag_total_prompt + rag_total_completion;

        // --- Print report ---
        println!("\n{:=^70}", " Typical Single-Session Token Estimate ");
        println!();
        println!("Assumptions:");
        println!("  - Query: \"{}\" ({} tokens)", typical_query, typical_query_tokens);
        println!("  - History: {} turns (~{} tokens each)", history_turns, history_tokens_per_turn);
        println!("  - Memory: summary + preferences = {} tokens", memory_tokens);
        println!("  - RAG chunks: {} chunks @ {} tokens each", chunks_count, chunk_tokens);
        println!("  - Search results: {} results @ {} tokens each", search_results_count, search_result_tokens);
        println!("  - RAG ReAct iterations: {}", rag_iterations);
        println!();
        println!("System prompt sizes (measured with tiktoken):");
        println!("  - Chat system:        {} tokens", chat_system_tokens);
        println!("  - RAG planner system: {} tokens", rag_plan_sys_tokens);
        println!("  - RAG evaluator sys:  {} tokens", rag_eval_sys_tokens);
        println!("  - Search evaluator:   {} tokens", search_eval_sys_tokens);
        println!();

        println!("{:-^70}", " Chat Mode ");
        println!("  Prompt:      {:>6} tokens  (system {} + memory {} + history {} + query {})",
            chat_prompt, chat_system_tokens, memory_tokens, typical_history_tokens, typical_query_tokens);
        println!("  Completion:  {:>6} tokens", chat_completion);
        println!("  Total:       {:>6} tokens", chat_total);
        println!();

        println!("{:-^70}", " Search Mode ");
        println!("  Evaluator prompt:   {:>6} tokens  (system {} + query/metadata {})",
            search_eval_prompt, search_eval_sys_tokens, search_eval_prompt - search_eval_sys_tokens);
        println!("  Evaluator completion: {:>4} tokens", search_eval_completion);
        println!("  Synthesizer prompt: {:>6} tokens  (system {} + query {} + evidence {})",
            search_synth_prompt, 50, typical_query_tokens, search_evidence_tokens);
        println!("  Synthesizer completion: {:>2} tokens", search_synth_completion);
        println!("  Total prompt:       {:>6} tokens", search_total_prompt);
        println!("  Total completion:   {:>6} tokens", search_total_completion);
        println!("  Total:              {:>6} tokens", search_total);
        println!();

        println!("{:-^70}", " RAG Mode (3 iterations) ");
        println!("  Per-iteration planner prompt:   {:>6} tokens", rag_plan_prompt_per_iter);
        println!("  Per-iteration planner completion: {:>4} tokens", rag_plan_completion_per_iter);
        println!("  Per-iteration evaluator prompt: {:>6} tokens", rag_eval_prompt_per_iter);
        println!("  Per-iteration evaluator completion: {:>2} tokens", rag_eval_completion_per_iter);
        println!("  Synthesizer prompt:             {:>6} tokens  (query/history {} + chunks {})",
            rag_synth_prompt, typical_query_tokens + typical_history_tokens, retrieval_tokens);
        println!("  Synthesizer completion:         {:>6} tokens", rag_synth_completion);
        println!("  Total prompt:                   {:>6} tokens", rag_total_prompt);
        println!("  Total completion:               {:>6} tokens", rag_total_completion);
        println!("  Total:                          {:>6} tokens", rag_total);
        println!();

        println!("{:-^70}", " Cost Comparison (relative to Chat) ");
        println!("  Chat:   {:>6} tokens  (1.0x baseline)", chat_total);
        println!("  Search: {:>6} tokens  ({:.1}x)", search_total, search_total as f64 / chat_total as f64);
        println!("  RAG:    {:>6} tokens  ({:.1}x)", rag_total, rag_total as f64 / chat_total as f64);
        println!();

        // Sanity assertions
        assert!(chat_total < 2000, "Chat should be under 2k tokens");
        assert!(search_total < 8000, "Search should be under 8k tokens");
        assert!(rag_total > 10000, "RAG should be over 10k tokens");
        assert!(rag_total > search_total * 2, "RAG should be >2x Search");
    }
}
