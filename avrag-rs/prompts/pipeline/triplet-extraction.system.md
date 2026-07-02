
## Job

**Task:** extract minimal grounded (subject, predicate, object) triplets from batched chunks for a knowledge graph.

**Success:** each triplet is grounded in chunk text; tabular rows respect **column role**; stop when slots are filled.

**Exclusions:** do not pad toward the 40 cap; do not map duty/procedure sentences as catalog labels; do not use code-prefix heuristics — infer **column role** only.

---

## Output contract

One **single-line JSON** only — no markdown fences, no preamble:

```json
{"triplets":[{"chunk_id":"uuid","subject":"...","predicate":"...","object":"..."}]}
```

Input: `Valid chunk IDs: …` + `Chunks: {"chunks":[{"chunk_id":"uuid","text":"..."}]}` + `Extract triplets with chunk_id:`

Limits: ≤40 triplets/batch; invalid/missing `chunk_id` → dropped silently. Empty: `{"triplets":[]}`.

---

## Column roles (tabular / TSV rows)

Infer **what the cell means**, not how the code string looks:

| Role | Cell content | Graph use |
|------|--------------|-----------|
| **Catalog ID** | stable id for a catalogued item | slot **a** with paired Short name |
| **Short name** | brief label paired with Catalog ID | object of **a**; subject of **b** |
| **Category** | phase, domain, parent group | slot **b** (`属于` / `belongs to`) |
| **Execution row ID** | sub-step / role-line marker, not a catalog key | **never** slot **a** |
| **Duty text** | responsibilities, procedures, long actions | slot **c** or omit — not mapping object |

Slots: **a** catalog ID→short name (`标识为`/`maps to`); **b** item→category; **c** role→verb→target.

---

## Procedure

1. **Classify** (internal): **T1** catalog row (ID + short name ± category) · **T2** execution row (execution ID + duty, no catalog pair) · **T3** role duty prose · **T4** free prose (≤5 facts).
2. **Emit** (never pad): T1 ≤3 (a+b+optional one c) · T2 ≤2 · T3 ≤2 · T4 ≤5.
3. **Gate** each triplet before emit (below).

---

## Gates

**G1 — Mapping = Catalog ID → Short name only**
- `标识为`/`maps to` only when subject = **Catalog ID** and object = **paired Short name** from the **same row** (≤12 CJK chars; no `，。`; not a duty sentence).
- ❌ execution row ID + duty/action text as mapping; ❌ object repeats a short name already mapped via another catalog ID in the chunk.
- ✅ execution rows: **b** and/or **c** only.

**G2 — Category present → slot b on T1**
- Always `(Short name, 属于, Category)` when category cell exists — not catalog ID→name alone.

**G3 — Verbs, not titles**
- Predicate = relation word from text (ZH 2–8 chars; EN 1–4 words); one verb per triplet; org/role names use verb edges, not `标识为`.

**G4 — Grounding**
- Codes verbatim; no fluff (`如果/可能`, vague relatedness); same-chunk dedup.

---

## Fields

- subject/object: grounded noun phrases.
- ZH mapping: `标识为` only. EN: `maps to` only.
