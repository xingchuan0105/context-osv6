"""avrag149 数据加载：把 RAG 质量黄金集（golden_set_realistic.json）展平为 SkillOpt split。

黄金集结构：``{"version": ..., "subsets": [{"name": ..., "examples": [...]}, ...]}``。
产品评测 runner 以「subsets 展平顺序」作为 1-based 题号（``E2E_QUESTIONS`` 索引），
因此这里展平时必须保持文件内顺序，并把 1-based 题号写进 item 的 ``id``。
"""
from __future__ import annotations

import json
import os
import random
from collections import defaultdict

from skillopt.datasets.base import SplitDataLoader, _compute_split_counts, _parse_split_ratio


class Avrag149DataLoader(SplitDataLoader):
    """加载 RAG 黄金集（149 题）为 SkillOpt train/val/test split。

    split_mode="ratio" 时由基类 shuffle 划分（split_seed 固定保证可复现），
    item 的 ``id`` 始终保留原始 1-based 题号，供 rollout 映射回 ``E2E_QUESTIONS``。
    ``include_ids`` 非空时只保留这些题号（精简集:错题 + 各题型代表题，
    2026-08-01）——id 仍是原展平题号,与 E2E_QUESTIONS 一致。

    split_mode="per_family"（2026-08-02，WP1 防记忆化地基）：
    - 在每个 subset 族**内部**按 7:2:1 划分，而不是全量混着切——保证
      留出集与训练集同族分布，随机全局切分会让同风格题跨 split，记忆化
      能骗过 gate（D6-② 结构性 holdout 的前提）。
    - ``holdout_subsets`` 中的整族全部进 test，**永不进训练/反射视野**
      （组合 subset 如 cross_document/ipd_table/rag_search_joint 的答案
      永不进入 optimizer，记忆化无处命中）。
    """

    def __init__(self, include_ids: list[int] | None = None,
                 holdout_subsets: list[str] | None = None, **kwargs):
        super().__init__(**kwargs)
        self.include_ids = set(include_ids) if include_ids else None
        self.holdout_subsets = set(holdout_subsets) if holdout_subsets else set()

    def setup(self, cfg: dict) -> None:
        """split_mode="per_family" 兼容：base 校验只认 ratio/split_dir。

        归一化为 ratio 过校验，用 ``self._per_family`` 标志让物化分支走族内切分。
        """
        mode = str(self.split_mode or cfg.get("split_mode") or "ratio").strip().lower()
        self._per_family = mode == "per_family"
        if self._per_family:
            self.split_mode = "ratio"
        super().setup(cfg)

    def _materialize_ratio_split(self, cfg: dict) -> str:
        if getattr(self, "_per_family", False):
            return self._materialize_per_family_split(cfg)
        return super()._materialize_ratio_split(cfg)

    def _resolve_split_output_dir(self, cfg: dict) -> str:
        if self.split_output_dir:
            return os.path.abspath(self.split_output_dir)
        out_root = os.path.abspath(str(cfg.get("out_root") or os.getcwd()))
        env_name = str(cfg.get("env") or type(self).__name__.replace("DataLoader", "").lower())
        ratio_tag = str(self.split_ratio or "2:1:7").replace(":", "-")
        mode = "perfamily" if getattr(self, "_per_family", False) else "ratio"
        return os.path.join(
            out_root, "_generated_splits",
            f"{env_name}_{ratio_tag}_{mode}_seed{self.split_seed}",
        )

    def _materialize_per_family_split(self, cfg: dict) -> str:
        """per-family split：族内 7:2:1 + 整族留出（holdout_subsets 全进 test）。"""
        data_path = os.path.abspath(str(self.data_path or "").strip())
        if not data_path:
            raise ValueError(f"{type(self).__name__} requires data_path when split_mode=per_family.")
        ratio = _parse_split_ratio(self.split_ratio)
        items = self.load_raw_items(data_path)
        if not items:
            raise ValueError(f"No raw items available for per_family split from {data_path}")

        by_family: dict[str, list[dict]] = defaultdict(list)
        for it in items:
            by_family[str(it.get("subset") or "unknown")].append(it)

        rng = random.Random(self.split_seed)
        train_items: list[dict] = []
        val_items: list[dict] = []
        test_items: list[dict] = []
        for family, fam_items in sorted(by_family.items()):
            if family in self.holdout_subsets:
                # 整族留出（组合 subset）：答案永不进训练/反射视野（D6-②）
                test_items.extend(fam_items)
                continue
            shuffled = list(fam_items)
            rng.shuffle(shuffled)
            tr, va, te = _compute_split_counts(len(shuffled), ratio)
            train_items.extend(shuffled[:tr])
            val_items.extend(shuffled[tr:tr + va])
            test_items.extend(shuffled[tr + va:])

        split_dir = self._resolve_split_output_dir(cfg)
        manifest = {
            "source_data_path": data_path,
            "split_mode": "per_family",
            "split_ratio": self.split_ratio,
            "split_seed": self.split_seed,
            "holdout_subsets": sorted(self.holdout_subsets),
            "counts": {"train": len(train_items), "val": len(val_items), "test": len(test_items)},
        }
        os.makedirs(split_dir, exist_ok=True)
        self.write_split_items(os.path.join(split_dir, "train"), train_items)
        self.write_split_items(os.path.join(split_dir, "val"), val_items)
        self.write_split_items(os.path.join(split_dir, "test"), test_items)
        with open(os.path.join(split_dir, "split_manifest.json"), "w", encoding="utf-8") as f:
            json.dump(manifest, f, ensure_ascii=False, indent=2)
        print(
            f"  [{type(self).__name__}] generated per_family split {self.split_ratio} "
            f"(holdout={sorted(self.holdout_subsets)}) at {split_dir} from {data_path}"
        )
        return split_dir

    def load_raw_items(self, data_path: str) -> list[dict]:
        with open(data_path, encoding="utf-8") as f:
            raw = json.load(f)

        subsets = raw.get("subsets")
        if not isinstance(subsets, list) or not subsets:
            raise ValueError(f"{data_path} 缺少 subsets 数组")

        items: list[dict] = []
        seq = 0  # 完整 golden 的展平序号 = E2E_QUESTIONS 原题号(与 items 长度无关)
        for subset in subsets:
            subset_name = str(subset.get("name") or "unknown")
            for ex in subset.get("examples", []):
                seq += 1
                if self.include_ids is not None and seq not in self.include_ids:
                    continue
                items.append({
                    "id": str(seq),
                    "n": seq,
                    "subset": subset_name,
                    # task_type 供 SkillOpt 按任务类型分组反射；取 subset 名最稳
                    "task_type": subset_name,
                    "query": ex["query"],
                    # 仅用于评分（ground_truth 永不写入 skill 文档/提示词）
                    "ground_truth": ex.get("expected_answer", ""),
                    "mode": ex.get("mode", ""),
                    "capabilities": ex.get("capabilities", []),
                    "expected_should_answer": ex.get("expected_should_answer", True),
                    # non-RAG 题（无 source_chunks）：faithfulness not_applicable，
                    # soft 分只用 correctness（2026-08-01 评分点修正）
                    "no_context": not bool(ex.get("source_chunks")),
                })

        if not items:
            raise ValueError(f"{data_path} 展平后无任何题目")
        return items
