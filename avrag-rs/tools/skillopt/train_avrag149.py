#!/usr/bin/env python3
"""avrag149 SkillOpt 训练入口（落地期：--check 静态自检，不触发评测/训练）。

用法
----
    静态自检（落地验证，不跑评测、不调 LLM）：
        .venv/bin/python train_avrag149.py --config configs/avrag149/default.yaml --check

    正式训练（等开发全部落地后再执行；会跑真实 nightly 评测 + LLM 反射）：
        .venv/bin/python train_avrag149.py --config configs/avrag149/default.yaml

说明
----
- env 注册为 ``avrag149``（本仓库内实现，不走 skillopt 包内 registry）。
- target 模型由产品评测链路承担（AGENT_LLM_* / E2E_LLM_*），本入口只配置
  optimizer（reflect/rewrite 阶段的 LLM）。
"""
from __future__ import annotations

import argparse
import datetime
import os
import shutil
import sys
from pathlib import Path

_TOOLS_ROOT = Path(__file__).resolve().parent
if str(_TOOLS_ROOT) not in sys.path:
    sys.path.insert(0, str(_TOOLS_ROOT))

from skillopt.model.common import normalize_backend_name  # noqa: E402

from avrag149.adapter import Avrag149Adapter  # noqa: E402

_ENV_REGISTRY: dict[str, type] = {}


def _register_envs() -> None:
    _ENV_REGISTRY["avrag149"] = Avrag149Adapter


def get_adapter(cfg: dict) -> Avrag149Adapter:
    _register_envs()
    env_name = str(cfg.get("env") or "avrag149")
    if env_name not in _ENV_REGISTRY:
        raise ValueError(f"Unknown environment '{env_name}'. Available: {list(_ENV_REGISTRY)}")
    adapter_cls = _ENV_REGISTRY[env_name]
    import inspect
    accepted = set(inspect.signature(adapter_cls.__init__).parameters) - {"self"}
    kwargs = {k: cfg[k] for k in accepted if k in cfg}
    return adapter_cls(**kwargs)


def load_config(config_path: str, overrides: list[str]) -> dict:
    """加载结构化 YAML（支持 _base_ 继承）+ 覆盖，展平为 trainer 用的 flat dict。"""
    from skillopt.config import apply_overrides, flatten_config, load_config as _load

    cfg = _load(config_path, overrides=overrides)
    flat = flatten_config(cfg)

    # backend 归一化（与 skillopt scripts/train.py 一致：优化器/目标默认同后端）
    backend = normalize_backend_name(
        flat.get("model_backend") or flat.get("target_backend") or "qwen_chat"
    )
    if backend in {"qwen", "qwen_chat"}:
        flat.setdefault("optimizer_backend", "qwen_chat")
        flat.setdefault("target_backend", "qwen_chat")
    elif backend in {"claude", "claude_chat"}:
        flat.setdefault("optimizer_backend", "claude_chat")
        flat.setdefault("target_backend", "claude_chat")
    else:
        flat.setdefault("optimizer_backend", backend)
        flat.setdefault("target_backend", backend)

    # skill_init 归一化为绝对路径（相对 tools/skillopt 根），任何 cwd 都能跑
    if flat.get("skill_init"):
        p = Path(flat["skill_init"])
        flat["skill_init"] = str(p if p.is_absolute() else _TOOLS_ROOT / p)

    if not flat.get("out_root"):
        ts = datetime.datetime.now().strftime("%Y%m%d_%H%M%S")
        flat["out_root"] = str(_TOOLS_ROOT / "outputs" / f"skillopt_avrag149_{ts}")
    flat["out_root"] = os.path.abspath(flat["out_root"])
    return flat


def sync_optimizer_env(flat: dict, avrag_rs_root: str | os.PathLike) -> None:
    """复用 avrag-rs/.env 的 AGENT_LLM_*（DeepSeek）作为 optimizer 的 qwen_chat 凭据。

    仅在 QWEN_CHAT_* 未设置时回填；不覆盖用户显式配置，不打印密钥。
    """
    if flat.get("optimizer_backend") not in {"qwen", "qwen_chat"}:
        return
    if os.environ.get("QWEN_CHAT_API_KEY"):
        return
    from avrag149.runner import load_env_file
    env = load_env_file(Path(avrag_rs_root) / ".env")
    mapping = {
        "QWEN_CHAT_BASE_URL": env.get("AGENT_LLM_BASE_URL"),
        "QWEN_CHAT_API_KEY": env.get("AGENT_LLM_API_KEY"),
        "QWEN_CHAT_MODEL": env.get("AGENT_LLM_MODEL"),
    }
    for k, v in mapping.items():
        if v and not os.environ.get(k):
            os.environ[k] = v
            print(f"  [env] {k} ← avrag-rs/.env (AGENT_LLM_*)")


# ── --check 静态自检 ─────────────────────────────────────────────────────────

def run_check(flat: dict) -> None:
    print("=" * 64)
    print("  avrag149 SkillOpt 落地静态自检（不触发评测 / 不调 LLM）")
    print("=" * 64)

    import skillopt
    print(f"  skillopt: {getattr(skillopt, '__version__', 'unknown')}")

    # 1. 关键路径存在性（avrag_rs_root 空 → 复用 adapter 的默认推断）
    from avrag149.adapter import _DEFAULT_AVRAG_RS_ROOT
    raw_root = flat.get("avrag_rs_root") or ""
    root = Path(raw_root) if raw_root else _DEFAULT_AVRAG_RS_ROOT
    if not root.is_absolute():
        root = _TOOLS_ROOT / root
    checks = {
        "avrag_rs_root": root,
        "prompts 树": root / "prompts",
        "目标 prompt 文件": root / "prompts" / str(flat.get("prompt_target", "system/agent-base.md")),
        "黄金集": root / "tests/rag_quality/golden_set_realistic.json",
        "skill_init": Path(flat.get("skill_init") or ""),
    }
    for name, p in checks.items():
        ok = p.is_file() if p.suffix else p.is_dir()
        print(f"  [{'OK' if ok else 'FAIL'}] {name}: {p}")
        if not ok:
            print(f"    → 修复后重跑；退出。")
            sys.exit(1)

    # 2. cargo 可用性（评测命令依赖）
    cargo = shutil.which("cargo")
    print(f"  [{'OK' if cargo else 'FAIL'}] cargo: {cargo or '不在 PATH'}")
    if not cargo:
        sys.exit(1)

    # 3. adapter 实例化 + split 加载（只读 JSON，不跑评测）
    print("  [..] 实例化 adapter + 加载 splits …")
    adapter = get_adapter(flat)
    adapter.setup(flat)
    dl = adapter.get_dataloader()
    print(f"      train={len(dl.train_items)} val={len(dl.val_items)} test={len(dl.test_items)}")
    if len(dl.train_items) + len(dl.val_items) + len(dl.test_items) != 149:
        print(f"      [FAIL] 题数合计 ≠ 149，检查 golden 集与 split_ratio")
        sys.exit(1)
    if not dl.train_items:
        print("      [FAIL] train 为空")
        sys.exit(1)
    task_types = adapter.get_task_types()
    print(f"      task_types: {len(task_types)} 个（前 5: {task_types[:5]}）")

    # 4. 训练计划估算（不执行）
    bs = int(flat.get("batch_size") or 0)
    epochs = int(flat.get("num_epochs") or 0)
    sel = int(flat.get("sel_env_num") or 0)
    print(f"  [OK] 配置解析：env={flat.get('env')} optimizer_backend={flat.get('optimizer_backend')}")
    print(f"       epochs={epochs} batch_size={bs} edit_budget={flat.get('edit_budget')}")
    print(f"       lr_scheduler={flat.get('lr_scheduler')} gate={flat.get('use_gate')} sel_env_num={sel}")
    if bs and len(dl.train_items):
        print(f"       ≈ train {len(dl.train_items)} 题 / batch {bs} → 每 epoch ~{max(1, len(dl.train_items) // bs)} 步")
    print(f"       out_root={flat.get('out_root')}")

    print()
    print("  落地验证通过。训练/评测尚未执行——等开发全部落地后运行：")
    print(f"    .venv/bin/python train_avrag149.py --config configs/avrag149/default.yaml")
    print("=" * 64)


def main() -> None:
    p = argparse.ArgumentParser(description="avrag149 SkillOpt 训练入口")
    p.add_argument("--config", required=True, help="YAML 配置路径")
    p.add_argument("--check", action="store_true", help="静态自检，不触发评测/训练")
    p.add_argument("--cfg-options", nargs="+", default=[], help="覆盖: section.key=value")
    p.add_argument("--out_root", type=str, help="产物目录覆盖")
    args = p.parse_args()

    flat = load_config(args.config, args.cfg_options)
    if args.out_root:
        flat["out_root"] = os.path.abspath(args.out_root)
    sync_optimizer_env(flat, flat.get("avrag_rs_root") or ".")

    if args.check:
        run_check(flat)
        return

    print(f"  env={flat.get('env')} optimizer_backend={flat.get('optimizer_backend')} "
          f"optimizer_model={flat.get('optimizer_model')}")
    print(f"  out_root={flat['out_root']}")

    adapter = get_adapter(flat)
    from skillopt.engine.trainer import ReflACTTrainer
    trainer = ReflACTTrainer(flat, adapter)
    summary = trainer.train()
    if summary.get("test_hard") is not None:
        print(f"  Final test: {summary['test_hard']:.4f}")


if __name__ == "__main__":
    main()
