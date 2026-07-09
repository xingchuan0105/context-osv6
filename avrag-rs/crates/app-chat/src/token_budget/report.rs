//! Report formatting.
use super::types::SimulationResult;

pub fn print_report(results: &[SimulationResult]) {
    println!("\n{:=^80}", " Token Budget Simulation Report ");
    println!();

    for r in results {
        println!("Scenario: {:<30} | Mode: {:<8}", r.scenario_name, r.mode);
        println!("  Total prompt:     {:>6} tokens", r.total_prompt_tokens);
        println!(
            "  Total completion: {:>6} tokens",
            r.total_completion_tokens
        );
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
    println!(
        "{:<25} {:>10} {:>10} {:>10}",
        "Scenario", "Prompt", "Completion", "Total"
    );
    println!("{}", "-".repeat(60));
    for r in results {
        println!(
            "{:<25} {:>10} {:>10} {:>10}",
            r.scenario_name, r.total_prompt_tokens, r.total_completion_tokens, r.total_tokens
        );
    }
    println!("\n");
}

