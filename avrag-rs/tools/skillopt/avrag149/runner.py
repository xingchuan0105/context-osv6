"""avrag149 rollout 执行层：把 skill 文档注入产品评测并解析 v2 报告。

注入机制说明
------------
E2E 测试上下文会强制 ``PROMPT_DIR`` 指向真实的 ``avrag-rs/prompts``
（``test_context/config.rs`` 的 ``e2e_prompt_dir()``），外部 PROMPT_DIR 注入无效。
因此这里采用**临时交换**：评测前备份 ``prompts/<prompt_target>`` 并写入 skill 内容，
评测（含异常）后恢复原文件。产品 prompts 目录只在评测期间被短暂改写，且必被恢复。

纪律
----
- 本模块只负责「把 skill 文档送进评测 + 读出分数」，不读取、不传播 golden answers。
- ``.env`` 的密钥只注入子进程环境，绝不打印、绝不写入任何文件。
"""
from __future__ import annotations

import csv
import json
import os
import re
import shutil
import subprocess
from pathlib import Path

# 评测产物根（相对 avrag_rs_root）
_EVAL_OUTPUT_REL = "crates/app/tests/e2e_output/rag_eval_v2"

# 评测命令模板（与 docs/engineering 交接文档中的全量/定向命令一致）
_CARGO_TEST = (
    "cargo test -p app --test product_e2e realistic_corpus_full_eval "
    "--features product-e2e -- --ignored --test-threads=1 --nocapture"
)


def load_env_file(path: str | os.PathLike) -> dict[str, str]:
    """解析 KEY=VALUE 形式的 env 文件（跳过注释与空行），供子进程复用。

    复用 avrag-rs/.env 的既有配置值（AGENTS.md：.env 复用纪律）。
    """
    env: dict[str, str] = {}
    p = Path(path)
    if not p.is_file():
        return env
    for raw in p.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, _, value = line.partition("=")
        key = key.strip()
        if key:
            env[key] = value.strip().strip('"').strip("'")
    return env


class SwapPromptFile:
    """上下文管理器：评测期间把 prompts 树内单个目标文件替换为 skill 内容。

    - ``__enter__`` 前校验目标文件存在、内容非空；替换前把原内容存到内存与
      ``out_dir/.prompt_backup/<target>``（崩溃恢复用）。
    - ``__exit__`` 无论是否异常都恢复原文件；恢复失败时给出 git 恢复提示。
    """

    def __init__(self, prompts_root: str | os.PathLike, target_rel: str,
                 skill_content: str, backup_dir: str | os.PathLike) -> None:
        self.target = Path(prompts_root) / target_rel
        self.skill_content = skill_content
        self.backup_file = Path(backup_dir) / Path(target_rel).name
        self._original: str | None = None

    def __enter__(self) -> "SwapPromptFile":
        if not self.target.is_file():
            raise FileNotFoundError(
                f"目标 prompt 文件不存在: {self.target}（检查 prompt_target 配置）"
            )
        self._original = self.target.read_text(encoding="utf-8")
        self.backup_file.parent.mkdir(parents=True, exist_ok=True)
        self.backup_file.write_text(self._original, encoding="utf-8")
        self.target.write_text(self.skill_content, encoding="utf-8")
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        try:
            if self._original is not None:
                self.target.write_text(self._original, encoding="utf-8")
        except OSError as restore_err:
            print(
                f"[WARN] 恢复 {self.target} 失败: {restore_err}\n"
                f"       备份在 {self.backup_file}；必要时 git 恢复: git checkout -- prompts/",
                file=__import__("sys").stderr,
            )
        finally:
            # 恢复完成后清掉备份（异常路径也会走到这里）
            try:
                if self.backup_file.exists():
                    self.backup_file.unlink()
            except OSError:
                pass


def _eval_output_dir(avrag_rs_root: Path) -> Path:
    return avrag_rs_root / _EVAL_OUTPUT_REL


def run_eval(
    avrag_rs_root: str | os.PathLike,
    *,
    questions: list[int] | None = None,
    extra_env: dict[str, str] | None = None,
    timeout_secs: int = 3600,
    verbose: bool = True,
    prompt_dir_override: str | os.PathLike | None = None,
    out_dir_override: str | os.PathLike | None = None,
) -> Path:
    """跑一次产品评测（nightly realistic_corpus_full_eval），返回新增的 v2 产物目录。

    ``questions`` 为 None 时跑全量 149 题；否则传 ``E2E_QUESTIONS``（1-based）。
    环境 = 当前进程环境 + avrag-rs/.env + extra_env 覆盖。
    ``prompt_dir_override``（WP2）：per-worker prompt 树——经 E2E_SKILLOPT_PROMPT_DIR
    豁免通道注入（config.rs），并发安全，不互斥共享 prompts 文件。
    ``out_dir_override``（WP2）：并行 worker 用独立评测输出目录（避免"最新目录"
    检测在并发下竞争）；默认 None → 共享 ``rag_eval_v2`` + 最新目录检测（串行语义不变）。
    """
    root = Path(avrag_rs_root).resolve()
    if not (root / "Cargo.toml").is_file():
        raise FileNotFoundError(f"不是 avrag-rs 根目录: {root}")

    # dotenv fills gaps only — process env (and extra_env) win, so launch
    # scripts can pin OpenCode Go / Ollama dual-channel without editing .env.
    env = load_env_file(root / ".env")
    env.update({k: v for k, v in os.environ.items() if v is not None})
    env["E2E_MODE"] = "nightly"
    if prompt_dir_override is not None:
        env["E2E_SKILLOPT_PROMPT_DIR"] = str(prompt_dir_override)
    if extra_env:
        env.update(extra_env)

    cmd = _CARGO_TEST
    if questions is not None and len(questions) != 149:
        qs = ",".join(str(q) for q in sorted(set(questions)))
        env["E2E_QUESTIONS"] = qs
    elif "E2E_QUESTIONS" in env:
        # 全量时不残留子集限制
        env.pop("E2E_QUESTIONS", None)

    out_dir = Path(out_dir_override) if out_dir_override is not None else _eval_output_dir(root)
    out_dir.mkdir(parents=True, exist_ok=True)
    before = set(out_dir.iterdir())

    if verbose:
        scope = f"E2E_QUESTIONS={env['E2E_QUESTIONS']}" if "E2E_QUESTIONS" in env else "全量 149"
        print(f"  [runner] 评测开始（{scope}，timeout={timeout_secs}s）…")
    proc = subprocess.run(
        cmd, shell=True, cwd=root, env=env,
        capture_output=True, text=True, timeout=timeout_secs,
    )

    after = set(out_dir.iterdir())
    new_dirs = sorted(after - before, key=lambda p: p.stat().st_mtime)
    if proc.returncode != 0:
        tail = (proc.stdout or "")[-3000:] + (proc.stderr or "")[-1000:]
        raise RuntimeError(
            f"评测失败 rc={proc.returncode}\n--- 输出尾部 ---\n{tail}"
        )
    if not new_dirs:
        raise RuntimeError(
            f"评测返回成功但未产生新的 v2 产物目录（{out_dir}）。\n"
            f"--- 输出尾部 ---\n{(proc.stdout or '')[-2000:]}"
        )
    newest = new_dirs[-1]
    if verbose:
        print(f"  [runner] 评测完成 → {newest}")
    return newest


def parse_report(
    v2_dir: str | os.PathLike,
    ids: list[int] | None = None,
) -> tuple[dict[int, dict], dict]:
    """解析 v2 产物目录，返回 (每题评分行, 汇总元信息)。

    per_query.tsv 列：n, subset, label, correctness, faithfulness, relevancy, recall, recall_at_k, query
    label 取值：PASS / PARTIAL / RETRIEVAL_MISS / UNGROUNDED / REFUSAL_WRONG（可能含 JUDGE_ERROR）。

    ``ids``：本次评测的 E2E 题号列表（与 ``run_eval(questions=...)`` 一致）。
    tsv 的 ``n`` 列是评测内枚举序号（``report.rs`` 用 ``i+1``）——全量时恰好等于
    题号，**子集时是 1..len(ids) 连续序号而非原题号**（2026-08-01 修：子集评测
    baseline 错位，30 题被误映射成连续号，PASS 29/30 显示为 7/30）。
    传入 ``ids`` 时按行序映射回原题号（tsv 行序 == sorted(ids) 序）。
    """
    vdir = Path(v2_dir)
    tsv = vdir / "per_query.tsv"
    if not tsv.is_file():
        raise FileNotFoundError(f"缺少 per_query.tsv: {tsv}")

    rows: dict[int, dict] = {}
    with tsv.open(encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f, delimiter="\t")
        for i, r in enumerate(reader):
            if ids is None:
                try:
                    n = int(r["n"])
                except (KeyError, ValueError):
                    continue
            else:
                # 子集评测:tsv 行 i 对应 sorted(ids)[i]
                if i >= len(ids):
                    continue
                n = ids[i]
            rows[n] = {
                "subset": r.get("subset", ""),
                "label": r.get("label", ""),
                "correctness": _to_float(r.get("correctness")),
                "faithfulness": _to_float(r.get("faithfulness")),
                "relevancy": _to_float(r.get("relevancy")),
                "recall": _to_float(r.get("recall")),
                "recall_at_k": _to_float(r.get("recall_at_k")),
                "query": r.get("query", ""),
            }

    meta: dict = {}
    summary = vdir / "summary.json"
    if summary.is_file():
        try:
            s = json.loads(summary.read_text(encoding="utf-8"))
            meta = {
                "judge_model": s.get("judge_model", ""),
                "judge_ok": (s.get("summary") or {}).get("judge_ok"),
                "judge_error": (s.get("summary") or {}).get("judge_error"),
                "total": (s.get("summary") or {}).get("total"),
                "label_counts": (s.get("summary") or {}).get("label_counts", {}),
            }
        except (json.JSONDecodeError, OSError):
            meta = {}
    return rows, meta


def score_row(row: dict, no_context: bool = False) -> tuple[int, float, bool]:
    """评分规则（与 nightly 判定口径对齐）。

    hard = 1 iff label == "PASS"；soft = mean(correctness, faithfulness) ∈ [0,1]。
    - ``no_context``（non-RAG 题，golden 无 source_chunks）：faithfulness 为
      not_applicable 占位 1.0、无区分度，soft 只用 correctness（2026-08-01 评分点修正）。
    - 返回 ``(hard, soft, skip)``：``skip=True`` 表示评测故障（JUDGE_ERROR，
      judge API 失败）——不是 skill 质量问题，调用方应从聚合/训练中排除，
      不得按 0 分惩罚（2026-08-01 评分点修正）。
    """
    label = row.get("label", "")
    if label in ("JUDGE_ERROR", "INFRA_ERROR"):
        # 评测故障(judge API 失败 / 答案未生成)不是 skill 质量问题:调用方应
        # 从聚合/训练中排除,不得按 0 分惩罚(2026-08-01 评分点修正)。
        return 0, 0.0, True
    hard = 1 if label == "PASS" else 0
    c = row.get("correctness", 0.0)
    f = row.get("faithfulness", 0.0)
    soft = c if no_context else (c + f) / 2.0
    return hard, soft, False


# ── 代码一次通过率（L1.5 code_pass）────────────────────────────────────
# 主取证：mode_debug.general.loop_rounds.exit_reasons 中的 code_gen / code_gen_error
# （终答通常已无 ```python 块；sandbox 源码不进 qNNN.json）。
# 辅取证：sandbox_error activity、tool_trace 执行错误、终答/代码块脏 API。

_CODE_EXEC_FAIL_RE = re.compile(
    r"Execution failed|AttributeError|NameError|TypeError|SyntaxError|"
    r"ModuleNotFoundError|ImportError|Traceback \(most recent call last\)",
    re.I,
)
_BAD_API_RE = re.compile(
    r"\b(dense_search|lexical_search|graph_search|read_lines)\s*\(|\btop_k\s*=",
    re.I,
)
_BAD_IMPORT_RE = re.compile(
    r"(?:^|\n)\s*(?:import\s+(?:os|subprocess)\b|from\s+(?:os|subprocess)\b)",
)
_CODE_FENCE_RE = re.compile(r"```(?:python)?\s*\n(.*?)```", re.I | re.S)
_CODEGEN_OK = "code_gen"
_CODEGEN_ERR = "code_gen_error"


def extract_code_blocks(text: str) -> list[str]:
    return [m.group(1) for m in _CODE_FENCE_RE.finditer(text or "")]


def score_code_pass(answer: str, attr: dict | None = None) -> tuple[int, float, dict]:
    """代码一次通过率评分。

    **主信号**（产品 loop 已写入 mode_debug）：
    - ``exit_reasons`` 含 ``code_gen_error`` → 未一次通过
      - 若首次 codegen 相关 reason 就是 error → hard=0 soft=0
      - 若先 error 后 code_gen（修好） → hard=0 soft=0.35（非一次通过）
    - 仅有 ``code_gen``、无 error → hard=1 soft=1
    - ``activity_counts.sandbox_error`` > 0 → hard=0

    **辅信号**：tool_errors / 终答 Execution failed / 终答代码块脏 API。

    **无 codegen 尝试**：hard=1 soft=1，reason=no_codegen_attempt（不惩罚 native 路径）。
    """
    attr = attr or {}
    detail: dict = {
        "code_pass": True,
        "reasons": [],
        "codegen_ok_rounds": 0,
        "codegen_err_rounds": 0,
        "first_codegen": None,
    }
    ans = answer or ""

    exit_reasons = [str(x) for x in (attr.get("exit_reasons") or []) if x]
    final_er = str(attr.get("exit_reason") or "").strip()
    if final_er and final_er not in exit_reasons:
        exit_reasons = exit_reasons + [final_er]

    n_ok = sum(1 for r in exit_reasons if r == _CODEGEN_OK)
    n_err = sum(1 for r in exit_reasons if r == _CODEGEN_ERR)
    detail["codegen_ok_rounds"] = n_ok
    detail["codegen_err_rounds"] = n_err
    first_codegen = next(
        (r for r in exit_reasons if r in (_CODEGEN_OK, _CODEGEN_ERR)), None
    )
    detail["first_codegen"] = first_codegen
    sandbox_n = int(attr.get("sandbox_error_count") or 0)

    # ── 主路径：loop exit_reasons ─────────────────────────────────────
    if n_err > 0 or sandbox_n > 0:
        detail["code_pass"] = False
        if sandbox_n > 0:
            detail["reasons"].append(f"sandbox_error_activity={sandbox_n}")
        if first_codegen == _CODEGEN_ERR:
            detail["reasons"].append("first_codegen_failed")
            # 若后续仍有成功 code_gen，给 soft 部分分；hard 仍 0（非一次通过）
            if n_ok > 0:
                detail["reasons"].append(f"later_codegen_ok rounds={n_ok}")
                return 0, 0.35, detail
            return 0, 0.0, detail
        # 首次 code_gen 成功、后续才 error（少见）
        detail["reasons"].append(
            f"codegen_error_after_ok ok={n_ok} err={n_err}"
        )
        return 0, 0.5, detail

    if n_ok > 0:
        detail["reasons"].append(f"codegen_all_ok rounds={n_ok}")
        # 辅：tool_trace 级失败仍算通过失败（TypeError 等）
        if attr.get("code_error") or attr.get("tool_errors"):
            # 过滤 embedding/infra 类噪声：只认明确代码契约错误
            te = [str(x) for x in (attr.get("tool_errors") or [])]
            codeish = [
                t
                for t in te
                if any(
                    k in t
                    for k in (
                        "TypeError",
                        "AttributeError",
                        "NameError",
                        "SyntaxError",
                        "answer_exec_fail",
                        "stderr:",
                    )
                )
            ]
            if codeish:
                detail["code_pass"] = False
                detail["reasons"].append(f"tool_code_errors={codeish}")
                return 0, 0.2, detail
        return 1, 1.0, detail

    # ── 无 codegen 轮次：回退到终答/代码块（通常无块）────────────────
    if attr.get("code_error") or attr.get("tool_errors"):
        te = [str(x) for x in (attr.get("tool_errors") or [])]
        codeish = [
            t
            for t in te
            if any(
                k in t
                for k in (
                    "TypeError",
                    "AttributeError",
                    "NameError",
                    "SyntaxError",
                    "answer_exec_fail",
                    "stderr:",
                )
            )
        ]
        if codeish:
            detail["code_pass"] = False
            detail["reasons"].append(f"tool_code_errors={codeish}")
            return 0, 0.0, detail

    if _CODE_EXEC_FAIL_RE.search(ans):
        detail["code_pass"] = False
        detail["reasons"].append("exec_fail_in_answer")
        return 0, 0.0, detail

    blocks = extract_code_blocks(ans)
    first = blocks[0] if blocks else ""
    if not first.strip():
        detail["reasons"].append("no_codegen_attempt")
        return 1, 1.0, detail

    if _BAD_API_RE.search(first) or _BAD_IMPORT_RE.search(first):
        detail["code_pass"] = False
        detail["reasons"].append("dirty_api_or_import_in_first_block")
        return 0, 0.25, detail

    if "client." in first and "await" not in first:
        detail["code_pass"] = False
        detail["reasons"].append("client_without_await_in_first_block")
        return 0, 0.5, detail

    detail["reasons"].append("first_block_clean")
    return 1, 1.0, detail


# ── 分步代理信号（WP0：按层取信号，黄金集综合 label 不直接当训练梯度）──
#
# 层 → 信号映射（与 docs/plans/2026-08-02-skillopt-layered-training-impl.md D4 一致）：
#   L1.5 代码层 → sandbox_error / no_output（需轨迹，见 WP3；此处占位）
#   L2  检索面   → recall（per_query 列）
#   L2.5 停点    → label 归类：UNGROUNDED=overconfident / PARTIAL=premature / REFUSAL_WRONG=degrade
#   L3  合成面   → correctness / faithfulness
#   L3b 选择     → SELECTION_MISS（recall>0 但 cited_gold=0）

def layer_signals(row: dict, no_context: bool = False) -> dict:
    """按层取代理信号（评分点 2026-08-01 修正保持：JUDGE/INFRA 不算质量缺陷）。

    返回字段：
    - recall         检索面（L2）：召回率 0..1
    - correctness    合成面（L3）：答案正确性
    - faithfulness   合成面（L3）：答案忠实度（no_context 题不适用）
    - stop_class     L2.5 停点归类：overconfident / premature / degrade / ok / infra
    - selection_miss 选择面（L3b）：1 iff label==SELECTION_MISS（检索成功但未引用 gold）
    """
    label = row.get("label", "")
    recall = _to_float(row.get("recall"))
    c = _to_float(row.get("correctness"))
    f = _to_float(row.get("faithfulness"))
    if label in ("JUDGE_ERROR", "INFRA_ERROR"):
        return {"recall": recall, "correctness": c, "faithfulness": f,
                "stop_class": "infra", "selection_miss": 0}
    if label == "UNGROUNDED":
        stop = "overconfident"
    elif label in ("PARTIAL", "RETRIEVAL_MISS") and recall > 0:
        # 有证据但答不全/答错 → 过早停或合成不全（L2.5/L3 边界，需轨迹二刀，WP3）
        stop = "premature"
    elif label == "REFUSAL_WRONG":
        stop = "degrade"
    else:
        stop = "ok"
    return {"recall": recall, "correctness": c, "faithfulness": f,
            "stop_class": stop, "selection_miss": 1 if label == "SELECTION_MISS" else 0}


def aggregate_layer_signals(rows: dict[int, dict]) -> dict:
    """从 per_query 行聚合分层失败分布（WP0 自检 / 训练信号可视化用）。

    返回：{layer: 失败数} + 总体（pass/total/avg_recall/avg_correctness/avg_faithfulness）。
    """
    agg: dict = {"pass": 0, "retrieval_miss": 0, "selection_miss": 0,
                 "overconfident": 0, "premature": 0, "degrade": 0,
                 "infra": 0, "total": 0, "avg_recall": 0.0,
                 "avg_correctness": 0.0, "avg_faithfulness": 0.0}
    recall_sum = corr_sum = faith_sum = 0.0
    for r in rows.values():
        agg["total"] += 1
        label = r.get("label", "")
        if label == "PASS":
            agg["pass"] += 1
        elif label in ("JUDGE_ERROR", "INFRA_ERROR"):
            agg["infra"] += 1
        elif label == "RETRIEVAL_MISS":
            agg["retrieval_miss"] += 1
        elif label == "SELECTION_MISS":
            agg["selection_miss"] += 1
        elif label == "UNGROUNDED":
            agg["overconfident"] += 1
        elif label == "REFUSAL_WRONG":
            agg["degrade"] += 1
        elif label == "PARTIAL":
            # 有证据但答不全 → 过早停或合成不全（L2.5/L3 边界，需轨迹二刀，WP3）
            agg["premature"] += 1
        s = layer_signals(r, no_context=False)
        recall_sum += s["recall"]
        corr_sum += s["correctness"]
        faith_sum += s["faithfulness"]
    n = agg["total"]
    agg["avg_recall"] = recall_sum / n if n else 0.0
    agg["avg_correctness"] = corr_sum / n if n else 0.0
    agg["avg_faithfulness"] = faith_sum / n if n else 0.0
    return agg


def load_artifact_answer(v2_dir: str | os.PathLike, n: int) -> str:
    """从题级 artifact 里取模型答案（供 reflect 轨迹用），取不到返回空串。"""
    path = Path(v2_dir) / f"q{n}.artifact.json"
    try:
        a = json.loads(path.read_text(encoding="utf-8"))
        sv = a.get("score_v2") or {}
        return str(sv.get("model_answer") or "").strip()
    except (OSError, json.JSONDecodeError):
        return ""


def load_attribution(v2_dir: str | os.PathLike, n: int) -> dict:
    """WP3 轨迹归因：读题级 artifact 的 per-question 分步信号。

    两个来源：
    - ``<v2_dir>/q{n}.artifact.json`` → ``score_v2``（retrieval/selection/judge）
    - ``e2e_output/realistic_corpus_full_eval/q{n}.json`` → ``mode_debug``/``tool_trace``
      （与 v2_dir 同级：v2_dir.parent.parent = e2e_output）

    归因字段（供 L1.5/L2、L2.5/L3 分离）：
    - stop_recall / graded_recall：停点证据覆盖度（recall of golden source_chunks）
    - selection_cited_gold：引用的 gold 命中数（SELECTION_MISS 判定）
    - unsupported_claims：编造/无据主张数（UNGROUNDED 判定）
    - code_error / no_output / tool_errors：代码层（L1.5）信号
    - activities / capabilities / answer_model：行为指纹
    """
    attr: dict = {
        "retrieval_recall": 0.0, "graded_recall": 0.0, "hit": False,
        "stop_recall": 0.0, "selection_cited": 0, "selection_cited_gold": 0,
        "selection_recall": 0.0, "unsupported_claims": 0, "code_error": False,
        "no_output": False, "tool_errors": [], "activities": [],
        "capabilities": [], "answer_model": "",
        "exit_reason": "", "exit_reasons": [], "sandbox_error_count": 0,
    }
    vdir = Path(v2_dir)

    # 1. score_v2（检索/选择/判断分步信号）
    art = vdir / f"q{n}.artifact.json"
    if art.is_file():
        try:
            sv = (json.loads(art.read_text(encoding="utf-8")).get("score_v2") or {})
            ret = sv.get("retrieval") or {}
            sel = sv.get("selection") or {}
            judge = sv.get("judge") or {}
            attr["retrieval_recall"] = _to_float(ret.get("recall"))
            attr["graded_recall"] = _to_float(ret.get("graded_recall"))
            attr["hit"] = bool(ret.get("hit"))
            attr["stop_recall"] = _to_float(ret.get("recall"))  # 停点证据覆盖度
            attr["selection_cited"] = int(sel.get("cited_count") or 0)
            attr["selection_cited_gold"] = int(sel.get("golden_matched_in_cited") or 0)
            attr["selection_recall"] = _to_float(sel.get("recall"))
            attr["unsupported_claims"] = len(
                (judge.get("faithfulness") or {}).get("unsupported_claims") or []
            )
        except (json.JSONDecodeError, OSError):
            pass

    # 2. mode_debug + tool_trace（行为指纹 + 代码层）
    # code_pass 主信号：loop_rounds.exit_reasons 的 code_gen / code_gen_error
    mode_dir = vdir.parent.parent / "realistic_corpus_full_eval"
    qf = mode_dir / f"q{n}.json"
    if qf.is_file():
        try:
            d = json.loads(qf.read_text(encoding="utf-8"))
            gen = ((d.get("mode_debug") or {}).get("general") or {})
            acts = gen.get("activity_counts") or {}
            if isinstance(acts, dict):
                attr["activities"] = list(acts.keys())
                attr["sandbox_error_count"] = int(acts.get("sandbox_error") or 0)
            else:
                attr["activities"] = list(acts or [])
                attr["sandbox_error_count"] = 0
            attr["answer_model"] = str(gen.get("answer_model") or "")
            attr["capabilities"] = list(gen.get("capabilities") or [])
            attr["exit_reason"] = str(gen.get("exit_reason") or "")
            lr = gen.get("loop_rounds") or {}
            if isinstance(lr, dict):
                attr["exit_reasons"] = [
                    str(x) for x in (lr.get("exit_reasons") or []) if x
                ]
            else:
                attr["exit_reasons"] = []
            tt = d.get("tool_trace") or []
            for t in tt:
                if not isinstance(t, dict):
                    continue
                if t.get("status") not in (None, "Ok", "ok"):
                    attr["tool_errors"].append(str(t.get("tool") or t.get("status")))
                se = str(t.get("stderr") or "")
                if se and _CODE_EXEC_FAIL_RE.search(se):
                    attr["tool_errors"].append(f"stderr:{t.get('tool')}")
                err = str(t.get("error") or "")
                if err and _CODE_EXEC_FAIL_RE.search(err):
                    attr["tool_errors"].append(f"error:{t.get('tool')}:{err[:80]}")
            ans = str(d.get("answer") or "")
            if _CODE_EXEC_FAIL_RE.search(ans):
                attr["tool_errors"].append("answer_exec_fail")
            attr["code_error"] = bool(attr["tool_errors"])
            # 无工具调用且零召回 → 代码层无产出（零检索直答类，行为报告 A 类）
            if not tt and attr["retrieval_recall"] == 0.0:
                attr["no_output"] = True
        except (json.JSONDecodeError, OSError):
            pass
    return attr


def summarize_attribution(label: str, attr: dict) -> str:
    """把归因压缩成 reflect 能读的 ``fail_reason`` 一行（WP3 第二刀）。

    层 → 信号：
    - RETRIEVAL_MISS → 检索面（recall=0）；code_error/no_output → L1.5 代码层
    - UNGROUNDED     → L2.5 停点编造（unsupported_claims）
    - SELECTION_MISS → L3b 选择（cited_gold=0）
    - PARTIAL        → L2.5/L3 边界（stop_recall vs unsupported_claims）
    """
    parts = [f"label={label}"]
    if label == "RETRIEVAL_MISS":
        if attr["code_error"]:
            parts.append(f"code_error={attr['tool_errors']}")   # L1.5
        elif attr["no_output"]:
            parts.append("no_output")                            # L1.5 零检索直答
        else:
            parts.append(f"query_recall={attr['retrieval_recall']:.2f}")  # L2
    elif label == "UNGROUNDED":
        parts.append(f"unsupported_claims={attr['unsupported_claims']}")
        parts.append(f"stop_recall={attr['stop_recall']:.2f}")
    elif label == "SELECTION_MISS":
        parts.append(f"cited={attr['selection_cited']} cited_gold={attr['selection_cited_gold']}")
    elif label in ("PARTIAL", "REFUSAL_WRONG"):
        parts.append(f"stop_recall={attr['stop_recall']:.2f}")
        if attr["unsupported_claims"]:
            parts.append(f"unsupported={attr['unsupported_claims']}")
        if attr["code_error"]:
            parts.append(f"code_error={attr['tool_errors']}")
    if attr["activities"]:
        parts.append(f"acts={len(attr['activities'])}")
    return " ".join(parts)


def _to_float(v) -> float:
    try:
        return float(v)
    except (TypeError, ValueError):
        return 0.0


def copy_prompt_tree(prompts_root: str | os.PathLike, dst: str | os.PathLike) -> None:
    """拷贝 prompts 整树到 dst（用于镜像对比/审计，非注入路径）。"""
    shutil.copytree(prompts_root, dst, dirs_exist_ok=True)


def build_worker_prompt_tree(
    prompts_root: str | os.PathLike,
    target_rel: str,
    skill_content: str,
) -> Path:
    """WP2：构造 per-worker prompt 树（拷贝整树 + 注入 skill 到 target）。

    并发安全：每个 worker 用自己的独立树，E2E 经 E2E_SKILLOPT_PROMPT_DIR 豁免
    通道使用它——不再互斥共享 prompts 文件（原 SwapPromptFile 串行瓶颈）。
    调用方负责清理返回的临时目录（``shutil.rmtree``）。
    """
    import tempfile

    src = Path(prompts_root)
    tmp = Path(tempfile.mkdtemp(prefix="skillopt_prompt_tree_"))
    try:
        shutil.copytree(src, tmp, dirs_exist_ok=True)
    except OSError as err:
        shutil.rmtree(tmp, ignore_errors=True)
        raise OSError(f"构造 per-worker prompt 树失败: {err}") from err
    target = tmp / target_rel
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(skill_content, encoding="utf-8")
    return tmp
