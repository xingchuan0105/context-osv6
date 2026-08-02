"""avrag149 SkillOpt 环境适配器：把产品 RAG 评测（149 题黄金集）接进 ReflACT 训练循环。

Trainer 调用链：build_train_env/build_eval_env → rollout → reflect（默认实现）→ gate。
rollout 即「skill 文档注入产品 prompts → 跑真实 nightly 评测 → 解析 v2 报告评分」。
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

from skillopt.datasets.base import BatchSpec
from skillopt.envs.base import EnvAdapter

from .dataloader import Avrag149DataLoader
from .rollout import run_batch, run_batches_parallel

# tools/skillopt/avrag149/adapter.py → avrag-rs（仓库根）
_DEFAULT_AVRAG_RS_ROOT = Path(__file__).resolve().parents[3]


def _merge_worker_predictions(out_dir: Path) -> None:
    """把 ``worker_*/predictions/<id>`` 合并回 ``out_dir/predictions/``。

    WP2 并行 rollout 后，SkillOpt reflect 从 ``out_dir/predictions/<id>`` 读取
    轨迹（gradient/reflect.py:141）；各 worker 的 predictions 先落在
    ``out_dir/worker_<i>/predictions/``，这里合并回统一读取点。
    """
    import shutil

    pred_dir = out_dir / "predictions"
    pred_dir.mkdir(parents=True, exist_ok=True)
    for w in sorted(out_dir.glob("worker_*")):
        src = w / "predictions"
        if not src.is_dir():
            continue
        for item in sorted(src.iterdir()):
            dst = pred_dir / item.name
            if not dst.exists():
                if item.is_dir():
                    shutil.copytree(item, dst)
                else:
                    shutil.copy2(item, dst)


class Avrag149Adapter(EnvAdapter):
    """SkillOpt 适配器：env name = ``avrag149``。"""

    def __init__(
        self,
        split_dir: str = "",
        data_path: str = "",
        split_mode: str = "ratio",
        split_ratio: str = "7:2:1",
        split_seed: int = 42,
        split_output_dir: str = "",
        workers: int = 4,
        eval_workers: int = 1,
        analyst_workers: int = 8,
        # WP4/D7：reflect 后端可插拔。llm = skillopt 默认裸 LLM 反射；
        # coding_agent = 走 reflect_agent.py（coding agent 代理，工作区无 ground_truth）。
        reflect_backend: str = "llm",
        failure_only: bool = False,
        minibatch_size: int = 8,
        edit_budget: int = 4,
        seed: int = 42,
        limit: int = 0,
        max_completion_tokens: int = 4096,
        avrag_rs_root: str = "",
        prompt_target: str = "system/agent-base.md",
        eval_timeout_secs: int = 3600,
        # 精简集(2026-08-01):题号白名单文件(错题 ∪ 各题型代表题);
        # 空/缺省 → 全量 149。
        include_ids_file: str = "",
        # per-family split 的整族留出(WP1 防记忆化地基):逗号分隔的 subset 名,
        # 全部进 test,永不进训练/反射视野(D6-② 结构性 holdout)。
        holdout_subsets: str = "",
        **kwargs,
    ) -> None:
        self.workers = int(workers)
        self.eval_workers = max(1, int(eval_workers))
        self.analyst_workers = int(analyst_workers)
        self.reflect_backend = str(reflect_backend or "llm").strip().lower()
        self.failure_only = bool(failure_only)
        self.minibatch_size = int(minibatch_size)
        self.edit_budget = int(edit_budget)
        self.max_completion_tokens = int(max_completion_tokens)
        self.avrag_rs_root = str(avrag_rs_root or _DEFAULT_AVRAG_RS_ROOT)
        self.prompt_target = prompt_target
        self.eval_timeout_secs = int(eval_timeout_secs)

        if not data_path:
            data_path = str(Path(self.avrag_rs_root) / "tests/rag_quality/golden_set_realistic.json")

        # 精简集:include_ids_file 相对 tools/skillopt 根解析
        include_ids: list[int] | None = None
        if include_ids_file:
            p = Path(include_ids_file)
            if not p.is_absolute():
                p = Path(__file__).resolve().parent.parent / p
            include_ids = [int(x) for x in json.loads(p.read_text(encoding="utf-8"))]

        # per-family split 的整族留出（WP1 防记忆化地基，D6-②）
        holdout = [s.strip() for s in str(holdout_subsets).split(",") if s.strip()]

        self.dataloader = Avrag149DataLoader(
            split_dir=split_dir,
            data_path=data_path,
            split_mode=split_mode,
            split_ratio=split_ratio,
            split_seed=split_seed,
            split_output_dir=split_output_dir,
            seed=seed,
            limit=limit,
            include_ids=include_ids,
            holdout_subsets=holdout,
        )

    # ── Lifecycle ────────────────────────────────────────────────────────

    def setup(self, cfg: dict) -> None:
        super().setup(cfg)
        self.dataloader.setup(cfg)

    def get_dataloader(self):
        return self.dataloader

    # ── Env construction ─────────────────────────────────────────────────

    def build_env_from_batch(self, batch: BatchSpec, **kwargs):
        return list(batch.payload or [])

    def build_train_env(self, batch_size: int, seed: int, **kwargs):
        batch = self.dataloader.build_train_batch(batch_size=batch_size, seed=seed, **kwargs)
        return self.build_env_from_batch(batch, **kwargs)

    def build_eval_env(self, env_num: int, split: str, seed: int, **kwargs):
        batch = self.dataloader.build_eval_batch(env_num=env_num, split=split, seed=seed, **kwargs)
        return self.build_env_from_batch(batch, **kwargs)

    # ── Rollout ──────────────────────────────────────────────────────────

    def rollout(self, env_manager, skill_content: str, out_dir: str, **kwargs) -> list[dict]:
        if self.eval_workers <= 1 or len(env_manager) <= 1:
            return run_batch(
                items=env_manager,
                skill_content=skill_content,
                out_root=out_dir,
                avrag_rs_root=self.avrag_rs_root,
                prompt_target=self.prompt_target,
                eval_timeout_secs=self.eval_timeout_secs,
                verbose=kwargs.get("verbose", True),
            )
        # WP2：切分 batch 并行 rollout（每 chunk 独立 prompt 树 + 独立 out 目录）
        chunks = [env_manager[i::self.eval_workers] for i in range(self.eval_workers)]
        jobs = [
            (chunk, skill_content, str(Path(out_dir) / f"worker_{i}"))
            for i, chunk in enumerate(chunks)
            if chunk
        ]
        results_lists = run_batches_parallel(
            jobs,
            avrag_rs_root=self.avrag_rs_root,
            prompt_target=self.prompt_target,
            eval_timeout_secs=self.eval_timeout_secs,
            max_workers=len(jobs),
            verbose=kwargs.get("verbose", True),
        )
        merged = [r for rl in results_lists for r in rl]
        _merge_worker_predictions(Path(out_dir))
        # 合并后的 rollouts.json 写到 out_dir（各 worker 的子目录保留审计）
        (Path(out_dir) / "rollouts.json").write_text(
            json.dumps(merged, ensure_ascii=False, indent=2), encoding="utf-8",
        )
        return merged

    def build_reference_text(self, item: dict) -> str:
        """C2/WP1：黄金集参考答案绝不进入 optimizer 视野（防记忆化，D6-①）。

        skillopt 默认 reflect 把 ``reference_text`` 作为 ``#### Hidden Reference``
        直接喂给 analyst（``gradient/reflect.py:160-164``）——这是记忆化泄漏口。
        评分（hard/soft）已在 rollout 宿主侧完成，optimizer 只需要
        query + answer + score + 检索指标，不需要 gold 文本。返回空串使该段不注入。
        """
        return ""

    # ── Reflect（WP4/D7：后端可插拔）───────────────────────────────────────

    def reflect(self, results, skill_content: str, out_dir: str, **kwargs):
        """评估/改写后端：llm（默认）| coding_agent。

        - ``llm``：skillopt 默认 minibatch reflect（裸 LLM，读 predictions/）。
        - ``coding_agent``：构造无 ground_truth 的工作区 → 调 reflect_agent.py
          （coding agent 代理）→ 解析 RawPatch。D6-① 红线：工作区不含 gold。
        """
        if self.reflect_backend != "coding_agent":
            return self._reflect_llm(results, skill_content, out_dir, **kwargs)
        return self._reflect_coding_agent(results, skill_content, out_dir, **kwargs)

    def _reflect_llm(self, results, skill_content: str, out_dir: str, **kwargs):
        """默认 llm 分支：完全对齐 skillopt 默认实现（run_minibatch_reflect）。"""
        from skillopt.gradient.reflect import run_minibatch_reflect

        return run_minibatch_reflect(
            results=results,
            skill_content=skill_content,
            prediction_dir=kwargs.get("prediction_dir", str(Path(out_dir) / "predictions")),
            patches_dir=kwargs.get("patches_dir", str(Path(out_dir) / "patches")),
            workers=self.analyst_workers,
            failure_only=self.failure_only,
            minibatch_size=self.minibatch_size,
            edit_budget=self.edit_budget,
            random_seed=kwargs.get("random_seed"),
            error_system=self.get_error_minibatch_prompt(),
            success_system=self.get_success_minibatch_prompt(),
            step_buffer_context=kwargs.get("step_buffer_context", ""),
            meta_skill_context=kwargs.get("meta_skill_context", ""),
            update_mode=kwargs.get("skill_update_mode", "patch"),
        )

    def _reflect_coding_agent(self, results, skill_content: str, out_dir: str, **kwargs):
        """coding_agent 分支：工作区（无 ground_truth）→ reflect_agent.py → RawPatch。"""
        import shutil
        import subprocess
        import tempfile

        workspace = Path(tempfile.mkdtemp(prefix="skillopt_reflect_ws_"))
        try:
            # 工作区：skill + 轨迹（conversation + trajectory_attribution，均无 gold）
            (workspace / "skill.md").write_text(skill_content, encoding="utf-8")
            traj_dir = workspace / "trajectories"
            traj_dir.mkdir()
            pred_dir = Path(kwargs.get("prediction_dir", str(Path(out_dir) / "predictions")))
            if pred_dir.is_dir():
                for tid in sorted(pred_dir.iterdir()):
                    if not tid.is_dir():
                        continue
                    dst = traj_dir / tid.name
                    dst.mkdir()
                    for fname in ("conversation.json", "trajectory_attribution.json"):
                        src = tid / fname
                        if src.is_file():
                            shutil.copy2(src, dst / fname)

            out_json = workspace / "patches.json"
            script = Path(__file__).resolve().parent / "reflect_agent.py"
            cmd = [sys.executable, str(script),
                   "--workspace", str(workspace),
                   "--skill", str(workspace / "skill.md"),
                   "--trajectories", str(traj_dir),
                   "--out", str(out_json),
                   "--edit-budget", str(self.edit_budget)]
            try:
                proc = subprocess.run(cmd, check=True, timeout=600,
                                      capture_output=True, text=True)
            except subprocess.CalledProcessError as exc:
                raise RuntimeError(
                    f"reflect_agent 失败 rc={exc.returncode}\n"
                    f"--- stderr ---\n{(exc.stderr or '')[-2000:]}"
                ) from exc
            patches = json.loads(out_json.read_text(encoding="utf-8"))
            return patches.get("patches") or []
        finally:
            shutil.rmtree(workspace, ignore_errors=True)

    # ── Prompts（2026-08-01）────────────────────────────────────────────
    # skillopt 0.2.0（PyPI）打包缺陷:skillopt/prompts/ 目录为空,load_prompt
    # 运行时 FileNotFoundError(首次真实训练暴露:reflect 需要 analyst_error)。
    # 从 GitHub main 拉取全部 prompts 到项目内 avrag149/prompts/ 托管,
    # override 加载路径——不修改 site-packages,版本可控。

    _PROMPTS_DIR = Path(__file__).resolve().parent / "prompts"

    def _load_env_prompt(self, name: str) -> str | None:
        path = self._PROMPTS_DIR / f"{name}.md"
        if not path.is_file():
            return None
        return path.read_text(encoding="utf-8")

    def get_task_types(self) -> list[str]:
        seen: list[str] = []
        for item in (
            self.dataloader.train_items
            + self.dataloader.val_items
            + self.dataloader.test_items
        ):
            tt = str(item.get("task_type") or "avrag149")
            if tt not in seen:
                seen.append(tt)
        return seen or ["avrag149"]
