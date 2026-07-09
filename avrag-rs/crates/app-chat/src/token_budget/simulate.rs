//! Mode simulators.
use avrag_llm::count_tokens;
use super::scenarios::default_scenarios;
use super::types::{Scenario, SimulationResult, StageEstimate};

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

    let iterations: u8 = if scenario.search_results.len() > 3 {
        2
    } else {
        1
    };

    for iter in 0..iterations {
        // Evaluator prompt: system + query + sub_queries + result metadata
        let eval_system = include_str!("../../../../prompts/synthesis/search-answer.md");
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
    let synth_prompt = format!(
        "Question:\n{}\n\nBrave LLM Context evidence:\n{}",
        scenario.query, evidence
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
        let plan_system = include_str!("../../../../prompts/orchestrators/rag-system.md");
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
        let eval_system = include_str!("../../../../prompts/synthesis/rag-answer.md");
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

pub(crate) fn estimate_completion_for_query(query: &str) -> usize {
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
