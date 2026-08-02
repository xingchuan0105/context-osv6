"""D6-③ 记忆化扫描器：训练候选 skill 的编辑增量 vs 黄金集答案/题面。

动机（2026-08-02）：optimizer LLM（尤其 coding agent）比裸 prompt 更能读库、
更能抄答案——候选 skill 可能把黄金集的参考答案或题面直接写进 prompt，短期
骗过训练 gate、出厂后丧失泛化能力（且泄漏 golden 私密语料）。

用法
----
    scanner = MemorizationScanner(avrag_rs_root)
    hits = scanner.scan(new_skill, base_skill)   # 空 = 干净

    # 训练时挂在 apply_patch_with_report 前：命中 → 返回 no-op 候选
    # （candidate == current，gate 不通过，记忆化编辑进拒绝路径）。

精度权衡（D6-③ 阈值可调，误伤可人工放行）：
- 整句命中：编辑片段与某条 gold 答案/题面长度相近且 n-gram Jaccard ≥ 阈值
  （阈值默认 0.6，防"规则里自然出现短事实/实体名"误报）；
- 太短的 gold（< 8 归一化字符）不参与匹配，避免"2019年"这类合法事实误报。
"""
from __future__ import annotations

import difflib
import json
import re
from pathlib import Path

_WS = re.compile(r"[\s，。、；：？！,.!?;:'\"“”‘’()（）\[\]【】\-—]+")


def _norm(text: str) -> str:
    return _WS.sub("", str(text)).lower()


def _ngrams(text: str, n: int = 4) -> set[str]:
    return {text[i:i + n] for i in range(max(0, len(text) - n + 1))}


def _jaccard(a: str, b: str) -> float:
    sa, sb = _ngrams(a), _ngrams(b)
    if not sa or not sb:
        return 0.0
    return len(sa & sb) / len(sa | sb)


def load_gold_fragments(avrag_rs_root: str | Path) -> list[str]:
    """黄金集 expected_answer / query / source_chunks → 归一化片段。

    只收长度 ≥ 6 的片段（太短的实体/数字不做整句判定）。
    """
    path = Path(avrag_rs_root) / "tests/rag_quality/golden_set_realistic.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    frags: list[str] = []
    for subset in data.get("subsets", []):
        for ex in subset.get("examples", []):
            for field in ("expected_answer", "query"):
                v = ex.get(field, "")
                if v:
                    frags.append(_norm(v))
            for c in ex.get("source_chunks", []):
                t = c.get("text", "") if isinstance(c, dict) else str(c)
                if t:
                    frags.append(_norm(t))
    return [f for f in frags if len(f) >= 6]


def extract_added_text(new_skill: str, base_skill: str) -> str:
    """行级 diff 的新增行（编辑增量，相对已知干净的基座）。"""
    added: list[str] = []
    for line in difflib.unified_diff(
        base_skill.splitlines(), new_skill.splitlines(), lineterm=""
    ):
        if line.startswith("+") and not line.startswith("+++"):
            added.append(line[1:])
    return "\n".join(added)


class MemorizationScanner:
    """扫描 skill 编辑增量对黄金集的记忆化命中。"""

    def __init__(
        self,
        avrag_rs_root: str | Path,
        sim_threshold: float = 0.6,
        min_frag_len: int = 8,
    ) -> None:
        self.gold = load_gold_fragments(avrag_rs_root)
        self.sim_threshold = sim_threshold
        self.min_frag_len = min_frag_len

    def scan(self, new_skill: str, base_skill: str) -> list[str]:
        """返回命中的编辑片段（空 = 干净）。base_skill 应为已知干净的基座
        （skill_init，产品 prompt 拷贝），使命中 = 训练注入的 gold。"""
        added = extract_added_text(new_skill, base_skill)
        hits: list[str] = []
        for frag in self._split_fragments(added):
            if self._is_memorized(frag):
                hits.append(frag)
        return hits

    # ── internals ────────────────────────────────────────────────────────

    def _split_fragments(self, added: str) -> list[str]:
        for line in added.splitlines():
            line = line.strip().strip("-#*|>")
            if len(line) >= self.min_frag_len:
                yield line

    def _is_memorized(self, frag: str) -> bool:
        nf = _norm(frag)
        if len(nf) < 8:
            return False
        for g in self.gold:
            if len(g) < 8:
                continue
            if nf == g:
                return True  # 整行逐字命中（含短答案）
            # 近似整句命中需双方都足够长（防短实体/事实自然出现误报）
            if len(g) >= 12 and len(nf) >= 12 and _jaccard(nf, g) >= self.sim_threshold:
                return True
        return False
