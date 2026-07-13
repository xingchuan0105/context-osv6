# DR2 status report

| Field | Value |
|-------|-------|
| generated | 2026-07-13T12:28:07+08:00 |
| overall | **OK** |
| tier | DR2-full |
| REQUIRE_L3 | 1 |
| SKIP_L3 | 0 |
| SKIP_L2_CORE | 0 |

## Layers

| Layer | Status |
|-------|--------|
| L1 (DR0) | ok |
| L2-core (DR1) | ok |
| L2-patho (DR2) | ok |
| L3-thin-ui (PW smoke) | ok |
| L3-thin-llm (4 modes) | ok |

## Next (if red)

See `[PYRAMID] next=` lines in the console for the failing layer.
Triage: docs/engineering/ACCEPTANCE_PYRAMID_STABILIZATION_PLAN_2026-07-10.md §4

## Re-run

```bash
bash scripts/test-dr2.sh
REQUIRE_L3=1 bash scripts/test-dr2.sh
SKIP_L3=1 bash scripts/test-dr2.sh
```
