use std::collections::BTreeMap;

use super::evaluator::{Evaluator, ExactMatchEvaluator, F1Evaluator};
use super::runner::{run_eval_with_trigger, run_evaluation};
use super::types::{
    EvalCase, EvalDatasetSpec, EvalMetric, EvalRun, EvalScore, EvalSummary, EvalTrigger,
    EvalTriggerConfig,
};

fn dummy_case(answer: &str, ground: Option<&str>) -> EvalCase {
    EvalCase {
        case_id: "c1".to_string(),
        request: crate::runtime::AgentRequest {
            kind: crate::AgentKind::Chat,
            query: "q".to_string(),
            notebook_id: None,
            session_id: None,
            doc_scope: vec![],
            messages: vec![],
            user_preferences: None,
            debug: false,
            stream: false,
            language: None,
            auth: crate::runtime::stub_agent_auth(),
            docscope_metadata: None,
            metadata: BTreeMap::new(),
            cancellation_token: None,
            guard_pipeline: None,
            preferred_tools: vec![],
            format_hint: None,
            max_iterations: None,
        },
        result: {
            let mut r = crate::runtime::AgentRunResult::default();
            r.answer = answer.to_string();
            r
        },
        ground_truth: ground.map(|s| s.to_string()),
        scores: vec![],
    }
}

fn make_run(run_id: &str, cases: Vec<EvalCase>, overall: f64) -> EvalRun {
    let mut metric_sums: BTreeMap<String, (f64, usize)> = BTreeMap::new();
    for c in &cases {
        for s in &c.scores {
            let entry = metric_sums.entry(s.metric.clone()).or_insert((0.0, 0));
            entry.0 += s.score;
            entry.1 += 1;
        }
    }
    let metric_averages: BTreeMap<String, f64> = metric_sums
        .into_iter()
        .map(|(k, (sum, count))| (k, sum / count as f64))
        .collect();

    EvalRun {
        run_id: run_id.to_string(),
        run_name: "test".to_string(),
        strategy: "ChatStrategy".to_string(),
        strategy_version: "v1".to_string(),
        started_at_ms: 0,
        completed_at_ms: Some(0),
        cases,
        summary: Some(EvalSummary {
            total_cases: 2,
            passed_cases: 0,
            failed_cases: 0,
            metric_averages,
            overall_score: overall,
        }),
        trigger: None,
        failures: vec![],
    }
}

fn case_with_score(case_id: &str, metric: &str, score: f64) -> EvalCase {
    let mut case = dummy_case("answer", Some("ground"));
    case.case_id = case_id.to_string();
    case.scores = vec![EvalScore {
        metric: metric.to_string(),
        score,
        explanation: None,
    }];
    case
}

#[tokio::test]
async fn exact_match_perfect() {
    let case = dummy_case("hello world", Some("hello world"));
    let eval = ExactMatchEvaluator;
    let score = eval.evaluate(&case).await.unwrap();
    assert_eq!(score.score, 1.0);
}

#[tokio::test]
async fn exact_match_fails_on_difference() {
    let case = dummy_case("hello world", Some("hello"));
    let eval = ExactMatchEvaluator;
    let score = eval.evaluate(&case).await.unwrap();
    assert_eq!(score.score, 0.0);
}

#[tokio::test]
async fn exact_match_normalizes_case_and_punctuation() {
    let case = dummy_case("Hello, World!", Some("hello world"));
    let eval = ExactMatchEvaluator;
    let score = eval.evaluate(&case).await.unwrap();
    assert_eq!(score.score, 1.0);
}

#[tokio::test]
async fn exact_match_requires_ground_truth() {
    let case = dummy_case("hello", None);
    let eval = ExactMatchEvaluator;
    let result = eval.evaluate(&case).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn f1_perfect_match() {
    let case = dummy_case("a b c", Some("a b c"));
    let eval = F1Evaluator;
    let score = eval.evaluate(&case).await.unwrap();
    assert!(score.score > 0.99);
}

#[tokio::test]
async fn f1_partial_match() {
    let case = dummy_case("a b c", Some("a b d"));
    let eval = F1Evaluator;
    let score = eval.evaluate(&case).await.unwrap();
    assert!(score.score > 0.5 && score.score < 1.0);
}

#[tokio::test]
async fn f1_no_overlap() {
    let case = dummy_case("x y z", Some("a b c"));
    let eval = F1Evaluator;
    let score = eval.evaluate(&case).await.unwrap();
    assert_eq!(score.score, 0.0);
}

#[tokio::test]
async fn run_evaluation_computes_summary() {
    let cases = vec![
        dummy_case("hello world", Some("hello world")),
        dummy_case("foo bar", Some("foo baz")),
    ];
    let evaluators: Vec<Box<dyn Evaluator>> =
        vec![Box::new(ExactMatchEvaluator), Box::new(F1Evaluator)];
    let run = run_evaluation("test", "ChatStrategy", "v1", cases, evaluators)
        .await
        .unwrap();
    assert!(run.summary.is_some());
    let summary = run.summary.unwrap();
    assert_eq!(summary.total_cases, 2);
    assert!(summary.overall_score > 0.0);
    assert!(summary.metric_averages.contains_key("exact_match"));
    assert!(summary.metric_averages.contains_key("f1"));
}

#[test]
fn eval_metric_names() {
    assert_eq!(EvalMetric::ExactMatch.name(), "exact_match");
    assert_eq!(EvalMetric::F1.name(), "f1");
    assert_eq!(EvalMetric::LlmAsJudge.name(), "llm_as_judge");
}

#[tokio::test]
async fn run_eval_with_trigger_collects_failures() {
    let cases = vec![
        dummy_case("hello world", Some("hello world")),
        dummy_case("foo bar", Some("foo baz")),
    ];
    let evaluators: Vec<Box<dyn Evaluator>> =
        vec![Box::new(ExactMatchEvaluator), Box::new(F1Evaluator)];
    let config = EvalTriggerConfig {
        trigger: EvalTrigger::PreMerge {
            dataset: "test".to_string(),
            sample_size: 2,
        },
        dataset: EvalDatasetSpec {
            dataset_id: "test".to_string(),
            sample_size: 2,
            filter: None,
        },
        pass_threshold: 0.75,
        metric_thresholds: BTreeMap::new(),
    };
    let (run, result) =
        run_eval_with_trigger("t", "ChatStrategy", "v1", cases, evaluators, config, None)
            .await
            .unwrap();
    assert_eq!(
        run.trigger,
        Some(EvalTrigger::PreMerge {
            dataset: "test".to_string(),
            sample_size: 2
        })
    );
    assert!(
        !run.failures.is_empty(),
        "expected some failures below 0.75 threshold"
    );
    assert!(run.failures.iter().any(|f| f.metric == "exact_match"));
    assert!(run.failures.iter().any(|f| f.metric == "overall"));
    assert_eq!(
        result.trigger,
        EvalTrigger::PreMerge {
            dataset: "test".to_string(),
            sample_size: 2
        }
    );
    assert!(result.pass_rate >= 0.0 && result.pass_rate <= 1.0);
}

#[tokio::test]
async fn run_eval_with_trigger_uses_metric_thresholds() {
    let cases = vec![dummy_case("hello world", Some("hello world"))];
    let evaluators: Vec<Box<dyn Evaluator>> = vec![Box::new(ExactMatchEvaluator)];
    let mut metric_thresholds = BTreeMap::new();
    metric_thresholds.insert("exact_match".to_string(), 1.01);
    let config = EvalTriggerConfig {
        trigger: EvalTrigger::NightlyRegression {
            dataset: "test".to_string(),
        },
        dataset: EvalDatasetSpec {
            dataset_id: "test".to_string(),
            sample_size: 1,
            filter: None,
        },
        pass_threshold: 0.5,
        metric_thresholds,
    };
    let (run, _result) =
        run_eval_with_trigger("t", "ChatStrategy", "v1", cases, evaluators, config, None)
            .await
            .unwrap();
    assert_eq!(
        run.trigger,
        Some(EvalTrigger::NightlyRegression {
            dataset: "test".to_string()
        })
    );
    assert!(
        run.failures
            .iter()
            .any(|f| f.metric == "exact_match" && f.threshold == 1.01)
    );
}

#[tokio::test]
async fn run_eval_with_trigger_no_failures_when_all_pass() {
    let cases = vec![dummy_case("hello world", Some("hello world"))];
    let evaluators: Vec<Box<dyn Evaluator>> = vec![Box::new(ExactMatchEvaluator)];
    let config = EvalTriggerConfig {
        trigger: EvalTrigger::RedTeam {
            attack_vectors: vec![],
        },
        dataset: EvalDatasetSpec {
            dataset_id: "test".to_string(),
            sample_size: 1,
            filter: None,
        },
        pass_threshold: 0.0,
        metric_thresholds: BTreeMap::new(),
    };
    let (run, result) =
        run_eval_with_trigger("t", "ChatStrategy", "v1", cases, evaluators, config, None)
            .await
            .unwrap();
    assert!(
        run.failures.is_empty(),
        "expected no failures with threshold 0.0"
    );
    assert_eq!(result.pass_rate, 1.0);
}

#[tokio::test]
async fn run_eval_with_trigger_comparison_with_baseline() {
    let cases = vec![dummy_case("hello world", Some("hello world"))];
    let evaluators: Vec<Box<dyn Evaluator>> = vec![Box::new(ExactMatchEvaluator)];
    let config = EvalTriggerConfig {
        trigger: EvalTrigger::PreMerge {
            dataset: "test".to_string(),
            sample_size: 1,
        },
        dataset: EvalDatasetSpec {
            dataset_id: "test".to_string(),
            sample_size: 1,
            filter: None,
        },
        pass_threshold: 0.0,
        metric_thresholds: BTreeMap::new(),
    };

    let baseline = make_run("base", vec![case_with_score("c1", "exact_match", 0.5)], 0.5);

    let (_run, result) = run_eval_with_trigger(
        "t",
        "ChatStrategy",
        "v1",
        cases,
        evaluators,
        config,
        Some(&baseline),
    )
    .await
    .unwrap();

    assert!(
        result.comparison.is_some(),
        "expected comparison when baseline is provided"
    );
    let comp = result.comparison.unwrap();
    assert_eq!(comp.baseline_run_id, "base");
    assert!(
        comp.overall_delta > 0.0,
        "expected positive delta since candidate improved"
    );
    assert!(
        comp.regressions.is_empty(),
        "expected no regressions since candidate improved"
    );
}
