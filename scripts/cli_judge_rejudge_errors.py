#!/usr/bin/env python3
"""Offline rejudge JUDGE_ERROR artifacts via Grok Build CLI (`grok -p`).

Reads run_dir/q*.artifact.json with score_v2.label == JUDGE_ERROR (or all
judge_status=error if --all-error), builds eval_v2 user prompt from
judge_input snapshot, calls:

  grok -p <prompt> --output-format plain --max-turns 1 --disallowed-tools ...

Parses judge JSON, applies label_for (Python port of design §5), rewrites
artifact score_v2 + judge.json, then rewrites summary.json + per_query.tsv
from all artifacts.

Usage:
  python3 scripts/cli_judge_rejudge_errors.py \\
    avrag-rs/crates/app/tests/e2e_output/rag_eval_v2/v2_YYYYMMDD-HHMMSS \\
    [--only q008,q097]
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

SCHEMA_VERSION = "rag_eval_judge_v2"
TAU_C = 0.7
TAU_F = 0.7
PARTIAL_MIN = 0.4

SYSTEM_PROMPT = """\
你是严格的中文 RAG 评测员。只输出合法 JSON，不要 markdown 围栏。

【评分细则】
1. answer_correctness（答案正确性，0–1 分）
- 与 reference_answer 语义等价即高分：允许改写、空格、同义、合理语序。
- reference_answer 是评分 rubric，不是字面模板。
- verdict=partial：核心事实正确但缺关键限定（例如年份对但主体张冠李戴）。
- 当 expected_should_answer=false：若模型正确拒答，verdict=not_applicable 且 score 填 1.0（占位，以 refusal 维度为准，不得给 0 分）；若仍作答，correctness 给低分。

2. faithfulness（忠实度，0–1 分）
- 只根据给定 context 判定。context_source=cited 表示 context 为答案实际引用的段落；context_source=retrieved_fallback 表示答案未引用任何段落、context 为检索兜底段落（证据链已断，判分需相应从严）。
- 答案中每个实质性事实 claim 必须被 context 支持；数字、日期、专名从严。
- 允许同义改写；不允许 context 中不存在的具体数字/实体。把所有无支持的 claim 列入 unsupported_claims。
- 判 unsupported_claims 的唯一标准是「该事实在 context 中是否有支持」：context 明确支持的事实——无论作为直接答案还是作为背景/相关信息呈现——一律不得列入 unsupported_claims。
- 对「呈现方式可能误导」（暗示、framing 问题）的关切写进 answer_correctness 或 answer_relevancy 的 rationale，不得折算进 faithfulness 扣分。
- 正确拒答（实质性声明语料未记载）中带引用的背景事实尤其受本条保护；refuse-then-fabricate（context 没有的具体数字/实体）仍照常判 unsupported。

3. answer_relevancy（答案相关性，0–1 分）
- 是否在回答所问；文不对题即使「事实正确」也给低分。

4. refusal（拒答判定 —— 实质重于形式）
- is_refusal 按实质判定：只要答案的核心信息是「材料/语料中没有所问内容」，即为拒答——包括解释型、委婉型、部分帮助型（明确声明所问事实未记载，同时提供相关背景）。直白拒答与解释型拒答完全等价，措辞不影响判定。
- 边界：答案声明「未记载」之后仍给出 context 中没有的具体信息（数字/名称/日期等）⇒ is_refusal=false（实质行为是编造，由 faithfulness 另行扣分）。
- correct_for_expectation 比较「观察到的实质行为」与 expected_should_answer，任何情况下都必须有确定值，禁止「无意义就填 false」：
  · 未拒答（is_refusal=false）且 expected_should_answer=true ⇒ correct_for_expectation 必须为 true（score 1.0）；
  · 拒答（is_refusal=true）且 expected_should_answer=false ⇒ correct_for_expectation 必须为 true（score 1.0）；
  · 未拒答但 expected_should_answer=false（该拒未拒）⇒ correct_for_expectation 为 false（score 0.0）；
  · 拒答但 expected_should_answer=true（不该拒却拒）⇒ correct_for_expectation 为 false（score 0.0）。
- 当 expected_should_answer=false 且模型实质拒答：answer_correctness.verdict 必须为 not_applicable，score 填 1.0（占位，不评分），不得给 0 分——正确拒答不存在「答错」。

5. 禁止
- 不要因「答案未出现某个精确字符串」扣 correctness。
- 不要用训练知识补全；context 没有的事实就判 ungrounded / insufficient。
"""

OUTPUT_SCHEMA_SHAPE = r"""{
  "schema_version": "rag_eval_judge_v2",
  "refusal": {"is_refusal": true, "correct_for_expectation": true, "score": 0.0, "rationale": "…"},
  "answer_correctness": {"score": 0.0, "verdict": "correct|partial|incorrect|not_applicable", "rationale": "…", "key_points_hit": ["…"], "key_points_missed": ["…"]},
  "faithfulness": {"score": 0.0, "verdict": "grounded|mixed|ungrounded|not_applicable", "unsupported_claims": ["…"], "rationale": "…"},
  "answer_relevancy": {"score": 0.0, "rationale": "…"},
  "context_sufficiency": {"score": 0.0, "verdict": "sufficient|partial|insufficient|unknown", "rationale": "…"}
}"""

DISALLOWED = (
    "web_search,web_fetch,run_terminal_command,read_file,search_replace,write,"
    "list_dir,grep,open_page,image_gen,image_edit,spawn_subagent"
)


def build_user_prompt(ji: dict) -> str:
    q = ji.get("question") or ""
    ref = ji.get("reference_answer") or ""
    esa = bool(ji.get("expected_should_answer", True))
    ans = ji.get("model_answer") or ""
    cs = ji.get("context_source") or "cited"
    cited = ji.get("cited_context") or []
    notes = ji.get("rubric_notes")
    enr = bool(ji.get("expect_no_retrieval", False))

    p = []
    p.append("【问题】\n" + q)
    p.append("\n\n【参考答案（评分 rubric，非字面模板）】\n" + ref)
    p.append("\n\n【expected_should_answer】\n" + ("true" if esa else "false"))
    p.append("\n\n【模型答案】\n" + ans)
    p.append(f"\n\n【评测 context（context_source={cs}）】\n")
    if cs == "no_context":
        p.append("（无 —— 本题不是 RAG 检索题）\n")
        p.append(
            "\n【重要】本题是纯聊天/工具题，没有也不应有检索 context。因此：\n"
            "- faithfulness.verdict 必须返回 \"not_applicable\"（score 填 1.0 占位，不评分）；\n"
            "- 不得因缺少 context 而编造 unsupported_claims；\n"
            "- context_sufficiency.verdict 必须返回 \"unknown\"；\n"
            "- answer_correctness / answer_relevancy / refusal 正常评分。\n"
        )
    elif cs == "tool_outputs":
        p.append("（内建工具输出，见下）\n")
        for i, chunk in enumerate(cited, 1):
            p.append(f"[{i}] {chunk}\n")
        p.append(
            "\n【重要】上方 context 是**内建工具的真实输出**（如 weather_query / calculator / docwiki），是本题答案的权威依据：\n"
            "- faithfulness 按答案是否被这些工具输出支持来评分（不是 not_applicable）；\n"
            "- answer_correctness 可以也应该验证——答案与工具输出一致即高分；\n"
            "- 仅当确实没有任何工具输出时才考虑 not_applicable。\n"
        )
    elif not cited:
        p.append("（空）\n")
    else:
        for i, chunk in enumerate(cited, 1):
            p.append(f"[{i}] {chunk}\n")
    if notes:
        p.append("\n【补充评分约定 rubric_notes】\n" + str(notes) + "\n")
    if enr:
        p.append(
            "\n【重要】本题是多轮对话/记忆题，答案可合法依赖对话历史（prior turns）而非检索 context。因此：\n"
            "- faithfulness.verdict 必须返回 \"not_applicable\"（score 填 1.0 占位，不评分）；\n"
            "- 不得因缺少检索 context 而编造 unsupported_claims；\n"
            "- context_sufficiency.verdict 必须返回 \"unknown\"；\n"
            "- answer_correctness / answer_relevancy / refusal 正常评分。\n"
        )
    p.append(
        "\n【输出要求】\n只输出一个合法 JSON 对象，不要 markdown 围栏，不要输出解释文字。\n"
        f'schema_version 固定为 "{SCHEMA_VERSION}"。结构：\n'
        + OUTPUT_SCHEMA_SHAPE
        + "\n\n【禁止】\n- 不要因「答案未出现某个精确字符串」扣 correctness。\n"
        "- 不要用训练知识补全；context 没有的事实就判 ungrounded / insufficient。\n"
    )
    return "".join(p)


def extract_json(text: str) -> dict:
    text = text.strip()
    # strip markdown fence if any
    m = re.search(r"```(?:json)?\s*([\s\S]*?)```", text)
    if m:
        text = m.group(1).strip()
    # first object
    start = text.find("{")
    if start < 0:
        raise ValueError("no JSON object in judge output")
    # brace match
    depth = 0
    for i, ch in enumerate(text[start:], start):
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return json.loads(text[start : i + 1])
    return json.loads(text[start:])


def derived_refusal_correct(is_refusal: bool, expected_should_answer: bool) -> bool:
    return is_refusal != expected_should_answer


def label_for(
    *,
    judge: dict,
    gold_exists: bool,
    no_context: bool,
    expect_no_retrieval: bool,
    expected_should_answer: bool,
    retrieval_recall: float,
    cited_gold_hits: int,
    eval_gate: str,
) -> str:
    corr = judge["answer_correctness"]
    faith = judge["faithfulness"]
    refusal = judge["refusal"]
    c_score = float(corr.get("score", 0))
    c_verdict = (corr.get("verdict") or "").lower()
    f_score = float(faith.get("score", 0))
    f_verdict = (faith.get("verdict") or "").lower()
    unsupported = faith.get("unsupported_claims") or []
    is_refusal = bool(refusal.get("is_refusal"))

    correctness_na = c_verdict == "not_applicable"
    answer_quality_ok = correctness_na or (
        c_score >= TAU_C and c_verdict not in ("partial", "incorrect")
    )

    if (eval_gate or "").lower() == "retrieval_primary":
        if not derived_refusal_correct(is_refusal, expected_should_answer):
            return "REFUSAL_WRONG"
        if gold_exists and not expect_no_retrieval:
            if retrieval_recall <= 0.0:
                return "RETRIEVAL_MISS"
            if retrieval_recall + 1e-12 < 1.0:
                return "PARTIAL"
        return "PASS"

    if gold_exists and not expect_no_retrieval and retrieval_recall == 0.0 and not answer_quality_ok:
        return "RETRIEVAL_MISS"
    if (
        gold_exists
        and retrieval_recall > 0.0
        and cited_gold_hits == 0
        and not correctness_na
        and c_score < TAU_C
    ):
        return "SELECTION_MISS"
    if not derived_refusal_correct(is_refusal, expected_should_answer):
        return "REFUSAL_WRONG"
    faith_applicable = (
        not no_context
        and not expect_no_retrieval
        and f_verdict != "not_applicable"
    )
    if faith_applicable and f_score < TAU_F and unsupported:
        if answer_quality_ok and retrieval_recall == 0.0 and not expect_no_retrieval:
            return "CORRECT_UNGROUNDED"
        return "UNGROUNDED"
    if not correctness_na and c_score < PARTIAL_MIN:
        return "INCORRECT"
    if not correctness_na and (c_score < TAU_C or c_verdict == "partial"):
        return "PARTIAL"
    return "PASS"


def call_grok(prompt: str, timeout: int = 180) -> str:
    cmd = [
        "grok",
        "-p",
        prompt,
        "--output-format",
        "plain",
        "--max-turns",
        "1",
        "--disallowed-tools",
        DISALLOWED,
        "--always-approve",
    ]
    r = subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        timeout=timeout,
    )
    out = (r.stdout or "").strip()
    if r.returncode != 0 and not out:
        err = (r.stderr or "").strip()
        raise RuntimeError(f"grok exit={r.returncode}: {err[:500]}")
    return out


def qid_from_path(p: Path) -> str:
    name = p.name
    return name.replace(".artifact.json", "")


def reaggregate(run_dir: Path) -> dict:
    scores = []
    for p in sorted(run_dir.glob("q*.artifact.json")):
        a = json.loads(p.read_text())
        s = a.get("score_v2") or {}
        if s:
            scores.append(s)
    label_counts = Counter(s.get("label") or "UNKNOWN" for s in scores)
    judge_ok = sum(1 for s in scores if (s.get("judge_status") or "").lower() in ("ok",))
    # also accept nested
    for s in scores:
        js = s.get("judge_status")
        if isinstance(js, dict):
            pass
    # judge_status may be string "ok"/"error" or missing when only label set
    def is_judge_ok(s):
        st = s.get("judge_status")
        if st is None:
            return s.get("label") not in ("JUDGE_ERROR", "INFRA_ERROR")
        if isinstance(st, str):
            return st.lower() == "ok"
        return True

    judge_ok = sum(1 for s in scores if is_judge_ok(s))
    judge_error = len(scores) - judge_ok

    def mean(vals):
        vals = [v for v in vals if v is not None]
        return sum(vals) / len(vals) if vals else None

    corr_vals = []
    faith_vals = []
    rel_vals = []
    recall_vals = []
    recall_at_k_vals = []
    faith_n = 0
    ret_n = 0
    subsets: dict[str, dict] = defaultdict(lambda: {
        "total": 0,
        "judge_ok": 0,
        "label_counts": Counter(),
        "corr": [],
        "faith": [],
        "rel": [],
        "recall": [],
        "recall_at_k": [],
        "faith_n": 0,
        "ret_n": 0,
    })

    for s in scores:
        sub = s.get("subset") or "unknown"
        lab = s.get("label") or "UNKNOWN"
        subsets[sub]["total"] += 1
        subsets[sub]["label_counts"][lab] += 1
        jok = is_judge_ok(s)
        if jok:
            subsets[sub]["judge_ok"] += 1
        j = s.get("judge") or {}
        ac = (j.get("answer_correctness") or {}).get("score")
        fa = (j.get("faithfulness") or {}).get("score")
        ar = (j.get("answer_relevancy") or {}).get("score")
        ret = s.get("retrieval") or {}
        rec = ret.get("recall")
        rak = ret.get("recall_at_k")
        enr = bool(s.get("expect_no_retrieval"))
        cs = s.get("context_source") or ""
        f_ver = ((j.get("faithfulness") or {}).get("verdict") or "").lower()
        if jok and ac is not None:
            corr_vals.append(float(ac))
            subsets[sub]["corr"].append(float(ac))
        if jok and ar is not None:
            rel_vals.append(float(ar))
            subsets[sub]["rel"].append(float(ar))
        faith_ok = jok and not enr and cs != "no_context" and f_ver != "not_applicable" and fa is not None
        if faith_ok:
            faith_vals.append(float(fa))
            faith_n += 1
            subsets[sub]["faith"].append(float(fa))
            subsets[sub]["faith_n"] += 1
        if not enr and rec is not None:
            recall_vals.append(float(rec))
            ret_n += 1
            subsets[sub]["recall"].append(float(rec))
            subsets[sub]["ret_n"] += 1
            if rak is not None:
                recall_at_k_vals.append(float(rak))
                subsets[sub]["recall_at_k"].append(float(rak))

    subset_out = {}
    for sub, d in sorted(subsets.items()):
        subset_out[sub] = {
            "total": d["total"],
            "judge_ok": d["judge_ok"],
            "faithfulness_applicable": d["faith_n"],
            "retrieval_applicable": d["ret_n"],
            "label_counts": dict(d["label_counts"]),
            "mean_answer_correctness": mean(d["corr"]),
            "mean_answer_relevancy": mean(d["rel"]),
            "mean_faithfulness": mean(d["faith"]),
            "mean_retrieval_recall": mean(d["recall"]),
            "mean_retrieval_recall_at_k": mean(d["recall_at_k"]),
        }

    summary = {
        "total": len(scores),
        "judge_ok": judge_ok,
        "judge_error": judge_error,
        "label_counts": dict(label_counts),
        "faithfulness_applicable": faith_n,
        "retrieval_applicable": ret_n,
        "mean_answer_correctness": mean(corr_vals),
        "mean_answer_relevancy": mean(rel_vals),
        "mean_faithfulness": mean(faith_vals),
        "mean_retrieval_recall": mean(recall_vals),
        "mean_retrieval_recall_at_k": mean(recall_at_k_vals),
        "subsets": subset_out,
    }

    # summary.json envelope
    old = {}
    sp = run_dir / "summary.json"
    if sp.exists():
        try:
            old = json.loads(sp.read_text())
        except Exception:
            old = {}
    envelope = {
        "judge_model": old.get("judge_model", "grok_build_cli"),
        "schema_version": old.get("schema_version", SCHEMA_VERSION),
        "summary": summary,
        "thresholds": old.get(
            "thresholds",
            {"tau_correctness": TAU_C, "tau_faithfulness": TAU_F, "partial_min": PARTIAL_MIN},
        ),
    }
    sp.write_text(json.dumps(envelope, ensure_ascii=False, indent=2) + "\n")

    # per_query.tsv (row n is sequential, not qnum — match prior harness)
    lines = [
        "n\tsubset\tlabel\teval_gate\tcorrectness\tfaithfulness\trelevancy\trecall\trecall_at_k\tquery"
    ]
    for i, s in enumerate(scores, 1):
        j = s.get("judge") or {}
        ac = (j.get("answer_correctness") or {}).get("score", "")
        fa = (j.get("faithfulness") or {}).get("score", "")
        ar = (j.get("answer_relevancy") or {}).get("score", "")
        ret = s.get("retrieval") or {}
        rec = ret.get("recall", "")
        rak = ret.get("recall_at_k", "")
        gate = s.get("eval_gate") or "full"
        q = (s.get("query") or "").replace("\t", " ").replace("\n", " ")
        lines.append(
            f"{i}\t{s.get('subset') or ''}\t{s.get('label') or ''}\t{gate}\t"
            f"{ac}\t{fa}\t{ar}\t{rec}\t{rak}\t{q}"
        )
    (run_dir / "per_query.tsv").write_text("\n".join(lines) + "\n")
    return summary


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("run_dir", type=Path)
    ap.add_argument("--only", default="", help="comma-separated qids e.g. q008,q097")
    ap.add_argument("--all-error", action="store_true", help="all judge_status=error")
    ap.add_argument("--timeout", type=int, default=180)
    args = ap.parse_args()
    run_dir: Path = args.run_dir
    if not run_dir.is_dir():
        print(f"run_dir not found: {run_dir}", file=sys.stderr)
        return 2

    only = {x.strip() for x in args.only.split(",") if x.strip()}
    targets = []
    for p in sorted(run_dir.glob("q*.artifact.json")):
        qid = qid_from_path(p)
        if only and qid not in only:
            continue
        a = json.loads(p.read_text())
        s = a.get("score_v2") or {}
        lab = s.get("label")
        jst = s.get("judge_status")
        jst_s = jst.lower() if isinstance(jst, str) else ""
        if only or lab == "JUDGE_ERROR" or (args.all_error and jst_s == "error"):
            if not a.get("judge_input"):
                print(f"{qid}: skip (no judge_input)")
                continue
            targets.append((qid, p, a))

    print(f"[cli-judge] run={run_dir}")
    print(f"[cli-judge] targets={[t[0] for t in targets]}")
    rejudged = 0
    failed = 0
    for qid, path, artifact in targets:
        ji = artifact["judge_input"]
        user = build_user_prompt(ji)
        prompt = SYSTEM_PROMPT + "\n\n" + user
        print(f"[cli-judge] {qid} calling grok …")
        try:
            raw = call_grok(prompt, timeout=args.timeout)
            parsed = extract_json(raw)
        except Exception as e:
            print(f"[cli-judge] {qid} FAIL: {e}")
            failed += 1
            continue

        old = artifact.get("score_v2") or {}
        ret = old.get("retrieval") or {}
        sel = old.get("selection") or {}
        gold_exists = int(sel.get("golden_count") or 0) > 0
        cited_hits = int(sel.get("golden_matched_in_cited") or 0)
        recall = float(ret.get("recall") or 0.0)
        cs = (ji.get("context_source") or old.get("context_source") or "cited").lower()
        enr = bool(ji.get("expect_no_retrieval") or old.get("expect_no_retrieval"))
        esa = bool(ji.get("expected_should_answer", True))
        # calculation card → expect_no_retrieval-ish for label? rejudge uses calculation_card
        qc = artifact.get("query_card") or {}
        if str(qc.get("question_type") or "").lower() == "calculation":
            enr = True
        gate = old.get("eval_gate") or "full"
        if isinstance(gate, dict):
            gate = gate.get("name") or "full"

        label = label_for(
            judge=parsed,
            gold_exists=gold_exists,
            no_context=(cs == "no_context"),
            expect_no_retrieval=enr,
            expected_should_answer=esa,
            retrieval_recall=recall,
            cited_gold_hits=cited_hits,
            eval_gate=str(gate),
        )
        old_label = old.get("label")
        new_score = dict(old)
        new_score["judge"] = parsed
        new_score["judge_status"] = "ok"
        new_score["label"] = label
        new_score["context_source"] = ji.get("context_source") or old.get("context_source")
        new_score["expect_no_retrieval"] = enr
        artifact["score_v2"] = new_score
        artifact["judge_status"] = "ok"
        artifact["judge_label"] = label
        path.write_text(json.dumps(artifact, ensure_ascii=False, indent=2) + "\n")

        jp = path.with_name(f"{qid}.judge.json")
        if jp.exists():
            try:
                jj = json.loads(jp.read_text())
            except Exception:
                jj = {}
            jj["judge_status"] = "ok"
            jj["note"] = "cli_grok_rejudge"
            jj["parsed"] = parsed
            jp.write_text(json.dumps(jj, ensure_ascii=False, indent=2) + "\n")
        else:
            jp.write_text(
                json.dumps(
                    {
                        "judge_status": "ok",
                        "note": "cli_grok_rejudge",
                        "parsed": parsed,
                    },
                    ensure_ascii=False,
                    indent=2,
                )
                + "\n"
            )

        print(
            f"[cli-judge] {qid}: {old_label} -> {label} "
            f"(corr={parsed.get('answer_correctness', {}).get('score')} "
            f"faith={parsed.get('faithfulness', {}).get('score')})"
        )
        rejudged += 1

    summary = reaggregate(run_dir)
    print(f"[cli-judge] rejudged={rejudged} failed={failed}")
    print(f"[cli-judge] label_counts={summary.get('label_counts')}")
    print(f"[cli-judge] PASS={summary.get('label_counts', {}).get('PASS')} / {summary.get('total')}")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
