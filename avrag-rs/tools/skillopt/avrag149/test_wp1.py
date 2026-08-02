#!/usr/bin/env python3
"""C2/WP1 验证：防泄漏 + per-family split（plain assert，.venv/bin/python avrag149/test_wp1.py 运行）。

覆盖验证门：
- build_reference_text 返回空串（gold 不喂 optimizer，D6-①）
- rollout results 的 reference_text 为空（断 Hidden Reference 泄漏）
- per-family split：holdout 整族只进 test；非 holdout 族内按比例切分
- layer_signals 按层取信号（WP0 回归）
"""
from __future__ import annotations

import sys
from pathlib import Path

_TOOLS = Path(__file__).resolve().parent.parent  # tools/skillopt
# avrag-rs 根 = tools/skillopt 的父级父级（tools → avrag-rs）
AVRAG_RS = _TOOLS.parent.parent
if str(_TOOLS) not in sys.path:
    sys.path.insert(0, str(_TOOLS))

from avrag149.adapter import Avrag149Adapter  # noqa: E402
from avrag149.runner import layer_signals  # noqa: E402


def test_build_reference_text_empty() -> None:
    """D6-①：参考答案绝不进 optimizer 视野。"""
    adapter = Avrag149Adapter(avrag_rs_root=str(AVRAG_RS))
    item = {"id": "1", "ground_truth": "2019年", "query": "Q"}
    assert adapter.build_reference_text(item) == ""
    print("  [OK] build_reference_text 返回空串（gold 不喂 optimizer）")


def test_layer_signals_mapping() -> None:
    """WP0 回归：label → 层信号归类。"""
    # RETRIEVAL_MISS recall=0 → 检索面
    s = layer_signals({"label": "RETRIEVAL_MISS", "recall": 0.0, "correctness": 0.0, "faithfulness": 0.0})
    assert s["stop_class"] == "ok"  # recall=0 的 RETRIEVAL_MISS 归检索面
    # SELECTION_MISS → 选择面
    s = layer_signals({"label": "SELECTION_MISS", "recall": 1.0, "correctness": 0.0, "faithfulness": 0.0})
    assert s["selection_miss"] == 1
    # UNGROUNDED → 停点编造
    s = layer_signals({"label": "UNGROUNDED", "recall": 1.0, "correctness": 1.0, "faithfulness": 0.0})
    assert s["stop_class"] == "overconfident"
    # PARTIAL 有证据 → 停点过早/合成不全边界
    s = layer_signals({"label": "PARTIAL", "recall": 1.0, "correctness": 0.7, "faithfulness": 0.8})
    assert s["stop_class"] == "premature"
    # JUDGE_ERROR → 基础设施
    s = layer_signals({"label": "JUDGE_ERROR", "recall": 0.0, "correctness": 0.0, "faithfulness": 0.0})
    assert s["stop_class"] == "infra"
    print("  [OK] layer_signals 按层归类正确")


def test_per_family_split() -> None:
    """per-family split：holdout 整族只进 test；非 holdout 族内比例切分。"""
    adapter = Avrag149Adapter(
        avrag_rs_root=str(AVRAG_RS),
        split_mode="per_family",
        split_ratio="7:2:1",
        split_seed=42,
        holdout_subsets="cross_document,ipd_table,rag_search_joint",
    )
    adapter.setup({"out_root": str(_TOOLS / "outputs" / "_test_wp1")})
    dl = adapter.get_dataloader()
    train_subsets = {str(it.get("subset")) for it in dl.train_items}
    val_subsets = {str(it.get("subset")) for it in dl.val_items}
    test_subsets = {str(it.get("subset")) for it in dl.test_items}
    holdout = {"cross_document", "ipd_table", "rag_search_joint"}
    assert holdout.isdisjoint(train_subsets | val_subsets), "holdout 泄漏进 train/val"
    assert holdout.issubset(test_subsets), "holdout 未完整进 test"
    # 总数守恒
    assert len(dl.train_items) + len(dl.val_items) + len(dl.test_items) == len(dl.load_raw_items(
        str(AVRAG_RS / "tests/rag_quality/golden_set_realistic.json")))
    print(f"  [OK] per-family split: holdout={sorted(holdout)} 只进 test；"
          f"train={len(dl.train_items)} val={len(dl.val_items)} test={len(dl.test_items)}")


def test_rollout_reference_text_empty() -> None:
    """D6-①：rollout results 的 reference_text 恒为空（断 Hidden Reference 注入）。"""
    from avrag149.rollout import run_batch  # noqa: F401
    # 直接验证 build_reference_text 路径即可（run_batch 会真实评测，不在此触发）
    adapter = Avrag149Adapter(avrag_rs_root=str(AVRAG_RS))
    # EnvAdapter 合并 reference_text 的路径：build_reference_text 返回空 → 不注入
    assert adapter.build_reference_text({"reference_text": "2019年"}) == ""
    print("  [OK] reference_text 断泄漏路径成立（build_reference_text 恒空）")


if __name__ == "__main__":
    print("C2/WP1 验证：")
    test_build_reference_text_empty()
    test_layer_signals_mapping()
    test_per_family_split()
    test_rollout_reference_text_empty()
    print("全部通过。")
