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
) -> Path:
    """跑一次产品评测（nightly realistic_corpus_full_eval），返回新增的 v2 产物目录。

    ``questions`` 为 None 时跑全量 149 题；否则传 ``E2E_QUESTIONS``（1-based）。
    环境 = 当前进程环境 + avrag-rs/.env + extra_env 覆盖。
    """
    root = Path(avrag_rs_root).resolve()
    if not (root / "Cargo.toml").is_file():
        raise FileNotFoundError(f"不是 avrag-rs 根目录: {root}")

    env = dict(os.environ)
    env.update(load_env_file(root / ".env"))
    env["E2E_MODE"] = "nightly"
    if extra_env:
        env.update(extra_env)

    cmd = _CARGO_TEST
    if questions is not None and len(questions) != 149:
        qs = ",".join(str(q) for q in sorted(set(questions)))
        env["E2E_QUESTIONS"] = qs
    elif "E2E_QUESTIONS" in env:
        # 全量时不残留子集限制
        env.pop("E2E_QUESTIONS", None)

    out_dir = _eval_output_dir(root)
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
    if label == "JUDGE_ERROR":
        return 0, 0.0, True
    hard = 1 if label == "PASS" else 0
    c = row.get("correctness", 0.0)
    f = row.get("faithfulness", 0.0)
    soft = c if no_context else (c + f) / 2.0
    return hard, soft, False


def load_artifact_answer(v2_dir: str | os.PathLike, n: int) -> str:
    """从题级 artifact 里取模型答案（供 reflect 轨迹用），取不到返回空串。"""
    path = Path(v2_dir) / f"q{n}.artifact.json"
    try:
        a = json.loads(path.read_text(encoding="utf-8"))
        sv = a.get("score_v2") or {}
        return str(sv.get("model_answer") or "").strip()
    except (OSError, json.JSONDecodeError):
        return ""


def _to_float(v) -> float:
    try:
        return float(v)
    except (TypeError, ValueError):
        return 0.0


def copy_prompt_tree(prompts_root: str | os.PathLike, dst: str | os.PathLike) -> None:
    """拷贝 prompts 整树到 dst（用于镜像对比/审计，非注入路径）。"""
    shutil.copytree(prompts_root, dst, dirs_exist_ok=True)
