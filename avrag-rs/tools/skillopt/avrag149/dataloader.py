"""avrag149 数据加载：把 RAG 质量黄金集（golden_set_realistic.json）展平为 SkillOpt split。

黄金集结构：``{"version": ..., "subsets": [{"name": ..., "examples": [...]}, ...]}``。
产品评测 runner 以「subsets 展平顺序」作为 1-based 题号（``E2E_QUESTIONS`` 索引），
因此这里展平时必须保持文件内顺序，并把 1-based 题号写进 item 的 ``id``。
"""
from __future__ import annotations

import json

from skillopt.datasets.base import SplitDataLoader


class Avrag149DataLoader(SplitDataLoader):
    """加载 RAG 黄金集（149 题）为 SkillOpt train/val/test split。

    split_mode="ratio" 时由基类 shuffle 划分（split_seed 固定保证可复现），
    item 的 ``id`` 始终保留原始 1-based 题号，供 rollout 映射回 ``E2E_QUESTIONS``。
    """

    def load_raw_items(self, data_path: str) -> list[dict]:
        with open(data_path, encoding="utf-8") as f:
            raw = json.load(f)

        subsets = raw.get("subsets")
        if not isinstance(subsets, list) or not subsets:
            raise ValueError(f"{data_path} 缺少 subsets 数组")

        items: list[dict] = []
        for subset in subsets:
            subset_name = str(subset.get("name") or "unknown")
            for ex in subset.get("examples", []):
                n = len(items) + 1  # 1-based，与 E2E_QUESTIONS 索引一致
                items.append({
                    "id": str(n),
                    "n": n,
                    "subset": subset_name,
                    # task_type 供 SkillOpt 按任务类型分组反射；取 subset 名最稳
                    "task_type": subset_name,
                    "query": ex["query"],
                    # 仅用于评分（ground_truth 永不写入 skill 文档/提示词）
                    "ground_truth": ex.get("expected_answer", ""),
                    "mode": ex.get("mode", ""),
                    "capabilities": ex.get("capabilities", []),
                    "expected_should_answer": ex.get("expected_should_answer", True),
                })

        if not items:
            raise ValueError(f"{data_path} 展平后无任何题目")
        return items
