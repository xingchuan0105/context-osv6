"""avrag149 SkillOpt 环境适配器：把产品 RAG 评测（149 题黄金集）接进 ReflACT 训练循环。

Trainer 调用链：build_train_env/build_eval_env → rollout → reflect（默认实现）→ gate。
rollout 即「skill 文档注入产品 prompts → 跑真实 nightly 评测 → 解析 v2 报告评分」。
"""
from __future__ import annotations

from pathlib import Path

from skillopt.datasets.base import BatchSpec
from skillopt.envs.base import EnvAdapter

from .dataloader import Avrag149DataLoader
from .rollout import run_batch

# tools/skillopt/avrag149/adapter.py → avrag-rs（仓库根）
_DEFAULT_AVRAG_RS_ROOT = Path(__file__).resolve().parents[3]


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
        analyst_workers: int = 8,
        failure_only: bool = False,
        minibatch_size: int = 8,
        edit_budget: int = 4,
        seed: int = 42,
        limit: int = 0,
        max_completion_tokens: int = 4096,
        avrag_rs_root: str = "",
        prompt_target: str = "system/agent-base.md",
        eval_timeout_secs: int = 3600,
        **kwargs,
    ) -> None:
        self.workers = int(workers)
        self.analyst_workers = int(analyst_workers)
        self.failure_only = bool(failure_only)
        self.minibatch_size = int(minibatch_size)
        self.edit_budget = int(edit_budget)
        self.max_completion_tokens = int(max_completion_tokens)
        self.avrag_rs_root = str(avrag_rs_root or _DEFAULT_AVRAG_RS_ROOT)
        self.prompt_target = prompt_target
        self.eval_timeout_secs = int(eval_timeout_secs)

        if not data_path:
            data_path = str(Path(self.avrag_rs_root) / "tests/rag_quality/golden_set_realistic.json")

        self.dataloader = Avrag149DataLoader(
            split_dir=split_dir,
            data_path=data_path,
            split_mode=split_mode,
            split_ratio=split_ratio,
            split_seed=split_seed,
            split_output_dir=split_output_dir,
            seed=seed,
            limit=limit,
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
        return run_batch(
            items=env_manager,
            skill_content=skill_content,
            out_root=out_dir,
            avrag_rs_root=self.avrag_rs_root,
            prompt_target=self.prompt_target,
            eval_timeout_secs=self.eval_timeout_secs,
            verbose=kwargs.get("verbose", True),
        )

    # reflect() 继承默认实现（读 out_dir/predictions/<id>/conversation.json）

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
