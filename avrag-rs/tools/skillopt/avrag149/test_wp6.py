#!/usr/bin/env python3
"""C5/WP6 验证：记忆化扫描器（plain assert，.venv/bin/python avrag149/test_wp6.py 运行）。

覆盖 M4 门：
- 把黄金集参考答案原样写进 skill → 必拒
- 把题面/答案改写为抽象通用规则 → 放行（不误伤）
- 合法规则含短实体（"速冻机"）→ 不误报
- train_avrag149 --scan 自检路径可用
"""
from __future__ import annotations

import sys
import tempfile
from pathlib import Path

_TOOLS = Path(__file__).resolve().parent.parent
AVRAG_RS = _TOOLS.parent.parent
if str(_TOOLS) not in sys.path:
    sys.path.insert(0, str(_TOOLS))

from avrag149.memorization_scanner import MemorizationScanner  # noqa: E402

GOLD = AVRAG_RS / "tests/rag_quality/golden_set_realistic.json"


def _gold_answer() -> str:
    import json
    data = json.loads(GOLD.read_text(encoding="utf-8"))
    for s in data["subsets"]:
        for ex in s.get("examples", []):
            ans = ex.get("expected_answer", "")
            if len(ans) >= 6:
                return ans
    raise AssertionError("no gold answer found")


def test_verbatim_answer_rejected() -> None:
    scanner = MemorizationScanner(AVRAG_RS)
    answer = _gold_answer()
    base = "# 技能基座\n\n通用规则。\n"
    new = base + f"\n## 具体答案\n{answer}\n"
    hits = scanner.scan(new, base)
    assert hits, f"黄金集参考答案必须被拒：{answer!r}"
    print(f"  [OK] 参考答案逐字写入 → 拒绝（{len(hits)} 条）")


def test_abstract_rule_passes() -> None:
    scanner = MemorizationScanner(AVRAG_RS)
    base = "# 技能基座\n\n通用规则。\n"
    # 抽象通用规则：不含具体题目/实体/数字
    rule = (
        "## 检索策略\n"
        "- 单事实题先做窄化检索（doc_ids 收窄后再查），再并行展开；\n"
        "- 多主张题优先 catalog 枚举，避免大范围重复全量扫描。\n"
    )
    new = base + rule
    hits = scanner.scan(new, base)
    assert not hits, f"抽象通用规则被误报：{hits}"
    print("  [OK] 抽象通用规则放行（不误伤）")


def test_short_entity_no_false_positive() -> None:
    scanner = MemorizationScanner(AVRAG_RS)
    base = "# 技能基座\n"
    # 合法规则里自然出现语料主题实体（"速冻机"），不应算记忆化
    rule = (
        "## 事实核查\n"
        "- 涉及速冻机等设备参数时，先从论文原文取数，不在正文心算。\n"
    )
    new = base + rule
    hits = scanner.scan(new, base)
    assert not hits, f"短实体误报：{hits}"
    print("  [OK] 合法规则含语料实体不误报")


def test_scan_cli_runs() -> None:
    import subprocess
    r = subprocess.run(
        [sys.executable, "train_avrag149.py", "--scan",
         "avrag149/skills/initial.md"],
        capture_output=True, text=True, cwd=str(_TOOLS),
    )
    out = r.stdout + r.stderr
    assert r.returncode == 0, out
    assert "记忆化扫描" in out, out
    # 初始 skill 是干净的产品 prompt 拷贝，不应命中
    assert "未命中记忆化" in out, out
    print("  [OK] --scan 自检跑通且初始 skill 干净")


if __name__ == "__main__":
    print("C5/WP6 验证（记忆化扫描器）：")
    test_verbatim_answer_rejected()
    test_abstract_rule_passes()
    test_short_entity_no_false_positive()
    test_scan_cli_runs()
    print("全部通过。")
