use super::*;
use super::simulate::estimate_completion_for_query;
use avrag_llm::count_tokens;



#[test]
fn simulate_chat_simple() {
    let scenarios = default_scenarios();
    let chat = scenarios
        .iter()
        .find(|s| s.name == "chat_simple_cn")
        .unwrap();
    let result = simulate_scenario(chat);
    assert_eq!(result.mode, "chat");
    assert!(result.total_prompt_tokens > 0);
    assert!(result.total_completion_tokens > 0);
    assert_eq!(result.stages.len(), 1);
}

#[test]
fn simulate_search_with_memory() {
    let scenarios = default_scenarios();
    let search = scenarios
        .iter()
        .find(|s| s.name == "search_complex")
        .unwrap();
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
        assert!(
            r.total_tokens > 0,
            "{} should have >0 tokens",
            r.scenario_name
        );
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
    let rag_complex = results
        .iter()
        .find(|r| r.scenario_name == "rag_complex")
        .unwrap();
    let chat_simple = results
        .iter()
        .find(|r| r.scenario_name == "chat_simple_cn")
        .unwrap();
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
    let rag_plan_sys = include_str!("../../../../prompts/orchestrators/rag-system.md");
    let rag_eval_sys = include_str!("../../../../prompts/synthesis/rag-answer.md");
    let search_eval_sys = include_str!("../../../../prompts/synthesis/search-answer.md");

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
    let memory_tokens =
        count_tokens(typical_summary) + count_tokens(&typical_prefs.to_string());

    // RAG chunks: 8 chunks, ~300 tokens each
    let chunks_count = 8;
    let chunk_tokens = 300;
    let retrieval_tokens = chunks_count * chunk_tokens;

    // Search results: 4 results, ~150 tokens each (title+url+snippet)
    let search_results_count = 4;
    let search_result_tokens = 150;
    let search_evidence_tokens = search_results_count * search_result_tokens;

    // --- Chat estimate ---
    let chat_prompt =
        chat_system_tokens + memory_tokens + typical_history_tokens + typical_query_tokens;
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
    let rag_plan_prompt_per_iter =
        rag_plan_sys_tokens + typical_query_tokens + typical_history_tokens + 50; // iteration annotation
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
    let rag_total_prompt = rag_iterations
        * (rag_plan_prompt_per_iter + rag_eval_prompt_per_iter)
        + rag_synth_prompt;
    let rag_total_completion = rag_iterations
        * (rag_plan_completion_per_iter + rag_eval_completion_per_iter)
        + rag_synth_completion;
    let rag_total = rag_total_prompt + rag_total_completion;

    // --- Print report ---
    println!("\n{:=^70}", " Typical Single-Session Token Estimate ");
    println!();
    println!("Assumptions:");
    println!(
        "  - Query: \"{}\" ({} tokens)",
        typical_query, typical_query_tokens
    );
    println!(
        "  - History: {} turns (~{} tokens each)",
        history_turns, history_tokens_per_turn
    );
    println!(
        "  - Memory: summary + preferences = {} tokens",
        memory_tokens
    );
    println!(
        "  - RAG chunks: {} chunks @ {} tokens each",
        chunks_count, chunk_tokens
    );
    println!(
        "  - Search results: {} results @ {} tokens each",
        search_results_count, search_result_tokens
    );
    println!("  - RAG ReAct iterations: {}", rag_iterations);
    println!();
    println!("System prompt sizes (measured with tiktoken):");
    println!("  - Chat system:        {} tokens", chat_system_tokens);
    println!("  - RAG planner system: {} tokens", rag_plan_sys_tokens);
    println!("  - RAG evaluator sys:  {} tokens", rag_eval_sys_tokens);
    println!("  - Search evaluator:   {} tokens", search_eval_sys_tokens);
    println!();

    println!("{:-^70}", " Chat Mode ");
    println!(
        "  Prompt:      {:>6} tokens  (system {} + memory {} + history {} + query {})",
        chat_prompt,
        chat_system_tokens,
        memory_tokens,
        typical_history_tokens,
        typical_query_tokens
    );
    println!("  Completion:  {:>6} tokens", chat_completion);
    println!("  Total:       {:>6} tokens", chat_total);
    println!();

    println!("{:-^70}", " Search Mode ");
    println!(
        "  Evaluator prompt:   {:>6} tokens  (system {} + query/metadata {})",
        search_eval_prompt,
        search_eval_sys_tokens,
        search_eval_prompt - search_eval_sys_tokens
    );
    println!(
        "  Evaluator completion: {:>4} tokens",
        search_eval_completion
    );
    println!(
        "  Synthesizer prompt: {:>6} tokens  (system {} + query {} + evidence {})",
        search_synth_prompt, 50, typical_query_tokens, search_evidence_tokens
    );
    println!(
        "  Synthesizer completion: {:>2} tokens",
        search_synth_completion
    );
    println!("  Total prompt:       {:>6} tokens", search_total_prompt);
    println!(
        "  Total completion:   {:>6} tokens",
        search_total_completion
    );
    println!("  Total:              {:>6} tokens", search_total);
    println!();

    println!("{:-^70}", " RAG Mode (3 iterations) ");
    println!(
        "  Per-iteration planner prompt:   {:>6} tokens",
        rag_plan_prompt_per_iter
    );
    println!(
        "  Per-iteration planner completion: {:>4} tokens",
        rag_plan_completion_per_iter
    );
    println!(
        "  Per-iteration evaluator prompt: {:>6} tokens",
        rag_eval_prompt_per_iter
    );
    println!(
        "  Per-iteration evaluator completion: {:>2} tokens",
        rag_eval_completion_per_iter
    );
    println!(
        "  Synthesizer prompt:             {:>6} tokens  (query/history {} + chunks {})",
        rag_synth_prompt,
        typical_query_tokens + typical_history_tokens,
        retrieval_tokens
    );
    println!(
        "  Synthesizer completion:         {:>6} tokens",
        rag_synth_completion
    );
    println!(
        "  Total prompt:                   {:>6} tokens",
        rag_total_prompt
    );
    println!(
        "  Total completion:               {:>6} tokens",
        rag_total_completion
    );
    println!("  Total:                          {:>6} tokens", rag_total);
    println!();

    println!("{:-^70}", " Cost Comparison (relative to Chat) ");
    println!("  Chat:   {:>6} tokens  (1.0x baseline)", chat_total);
    println!(
        "  Search: {:>6} tokens  ({:.1}x)",
        search_total,
        search_total as f64 / chat_total as f64
    );
    println!(
        "  RAG:    {:>6} tokens  ({:.1}x)",
        rag_total,
        rag_total as f64 / chat_total as f64
    );
    println!();

    // Sanity assertions
    assert!(chat_total < 2000, "Chat should be under 2k tokens");
    assert!(search_total < 8000, "Search should be under 8k tokens");
    assert!(rag_total > 10000, "RAG should be over 10k tokens");
    assert!(rag_total > search_total * 2, "RAG should be >2x Search");
}
