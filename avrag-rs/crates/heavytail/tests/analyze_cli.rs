use std::path::PathBuf;
use std::process::Command;

use heavytail::metrics::analyze_sentences;
use heavytail::segment::split_sentences;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn fingerprint_fixture(name: &str) -> heavytail::metrics::FingerprintReport {
    let prose = std::fs::read_to_string(fixture(name)).expect("read fixture");
    let sentences: Vec<(String, usize)> = split_sentences(&prose, false)
        .into_iter()
        .map(|s| (s.text, s.para_idx))
        .collect();
    analyze_sentences(&sentences)
}

#[test]
fn human_like_fixture_has_higher_cv_than_ai_like() {
    let human = fingerprint_fixture("human_like.txt");
    let ai = fingerprint_fixture("ai_like.txt");
    assert!(
        human.cv > ai.cv,
        "expected human CV {} > AI CV {}",
        human.cv,
        ai.cv
    );
    assert!(
        human.sentence_lengths.len() >= 3,
        "human fixture should have multiple sentences"
    );
}

#[test]
fn analyze_cli_runs_on_fixture() {
    let bin = env!("CARGO_BIN_EXE_heavytail-analyze");
    let path = fixture("human_like.txt");
    let output = Command::new(bin)
        .arg(&path)
        .output()
        .expect("spawn heavytail-analyze");
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("CV:"));
    assert!(stdout.contains("Top sensitivity rows"));
}

#[test]
fn analyze_cli_json_mode() {
    let bin = env!("CARGO_BIN_EXE_heavytail-analyze");
    let path = fixture("ai_like.txt");
    let output = Command::new(bin)
        .arg(&path)
        .arg("--json")
        .output()
        .expect("spawn heavytail-analyze");
    assert!(output.status.success());
    let v: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse JSON output");
    assert!(v.get("fingerprint").is_some());
    assert!(v.get("top_sensitivity").is_some());
}

#[test]
fn compare_cli_runs_on_fixtures_dirs() {
    let bin = env!("CARGO_BIN_EXE_heavytail-analyze");
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let human = base.join("_compare_human");
    let ai = base.join("_compare_ai");
    std::fs::create_dir_all(&human).unwrap();
    std::fs::create_dir_all(&ai).unwrap();
    std::fs::copy(fixture("human_like.txt"), human.join("sample.txt")).unwrap();
    std::fs::copy(fixture("ai_like.txt"), ai.join("sample.txt")).unwrap();

    let output = Command::new(bin)
        .arg("compare")
        .arg(&human)
        .arg(&ai)
        .output()
        .expect("spawn compare");
    assert!(
        output.status.success(),
        "compare failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Cohen d"));
    assert!(stdout.contains("GATE 1"));

    std::fs::remove_dir_all(&human).ok();
    std::fs::remove_dir_all(&ai).ok();
}
