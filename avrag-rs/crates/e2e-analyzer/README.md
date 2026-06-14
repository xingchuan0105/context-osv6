# e2e-analyzer

Cross-run E2E test analysis framework.

## Commands

```bash
# Diff current run against baseline
cargo run -p e2e-analyzer -- diff --baseline crates/app/tests/e2e_output/e2e_20260528-XXXXXX --current crates/app/tests/e2e_output/e2e_20260528-YYYYYY

# Diagnose failures with attribution
cargo run -p e2e-analyzer -- diagnose --run crates/app/tests/e2e_output/e2e_20260528-XXXXXX

# Coverage matrix (P1)
cargo run -p e2e-analyzer -- coverage --runs crates/app/tests/e2e_output

# Stability trends (P2)
cargo run -p e2e-analyzer -- trends --history crates/app/tests/e2e_output --limit 20

# Combined report
cargo run -p e2e-analyzer -- report --run crates/app/tests/e2e_output/e2e_20260528-XXXXXX --output report.md --format markdown

# llm_real runs (bucket layout: llm_real/{run_id}/{test_name}/metadata.json)
cargo run -p e2e-analyzer -- llm-real list --output crates/app/tests/e2e_output
cargo run -p e2e-analyzer -- llm-real summary --run crates/app/tests/e2e_output/llm_real/e2e_20260528-XXXXXX
cargo run -p e2e-analyzer -- llm-real trends --output crates/app/tests/e2e_output --limit 20

# Promote newest llm_real run as regression baseline
./scripts/promote-llm-real-baseline.sh
cargo run -p e2e-analyzer -- llm-real summary --run crates/app/tests/e2e_output/llm_real/e2e_20260528-XXXXXX --output llm_real_summary.md

# Legacy flat layout still supported for diff/diagnose/report; trends/coverage also scan buckets

# Baseline management
cargo run -p e2e-analyzer -- baseline promote --run crates/app/tests/e2e_output/e2e_20260528-XXXXXX
cargo run -p e2e-analyzer -- baseline show
```

## Exit Codes

- `0`: Pass or soft drift only
- `1`: Critical regression detected (CI gate blocked)
