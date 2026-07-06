#!/usr/bin/env python3
"""Generate synthetic Chinese corpora for HeavyTail GATE 1 (MVP)."""

from __future__ import annotations

import random
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "tests/corpus"
HUMAN_DIR = ROOT / "human"
AI_DIR = ROOT / "ai"

TOPICS = [
    "城市更新",
    "人工智能",
    "教育改革",
    "气候变化",
    "医疗健康",
    "文化传承",
    "交通出行",
    "乡村振兴",
    "数字经济",
    "青年就业",
    "食品安全",
    "航天探索",
]

HUMAN_SHORT = ["风很大。", "他停了停。", "雨来了。", "没人说话。", "灯还亮着。", "路很长。"]

HUMAN_LONG = [
    "当我们站在历史与现实的交汇点上回望，会发现那些看似微小的日常选择往往在多年以后才显露出真正的重量。",
    "街角的旧书摊还在，只是老板换了一位年轻人，他一边整理泛黄的封面，一边用耳机听着播客。",
    "政策文本读起来枯燥，但真正落到社区里，会变成晾衣绳上的风、晚高峰地铁里交换的一个眼神。",
]

# Small closed vocabulary reused heavily → low hapax for AI-like text.
AI_VOCAB = [
    "发展",
    "建设",
    "推进",
    "完善",
    "提升",
    "机制",
    "体系",
    "能力",
    "水平",
    "工作",
    "任务",
    "目标",
    "措施",
    "方案",
    "管理",
    "服务",
    "保障",
    "落实",
    "加强",
    "协调",
]


def hapax_clause(seed: int, para: int, sent: int) -> str:
    """Single-use content phrase to lift hapax ratio on human side."""
    return f"独见词{seed:02d}{para}{sent}号。"


def write_human(path: Path, topic: str, seed: int) -> None:
    rng = random.Random(seed)
    paras: list[str] = []
    for p in range(4):
        parts: list[str] = []
        # Cluster lengths for positive lag-1 autocorr: run of shorts, then run of longs.
        for s in range(rng.randint(3, 4)):
            parts.append(rng.choice(HUMAN_SHORT))
        for s in range(rng.randint(2, 3)):
            parts.append(rng.choice(HUMAN_LONG))
        parts.append(f"关于{topic}的局部细节{hapax_clause(seed, p, 9)}")
        parts.append(hapax_clause(seed, p, 10))
        paras.append("".join(parts))
    path.write_text("\n\n".join(paras) + "\n", encoding="utf-8")


def write_ai(path: Path, topic: str, seed: int) -> None:
    rng = random.Random(seed + 1000)
    sentences: list[str] = []
    # Uniform ~20-char sentences built only from AI_VOCAB + topic.
    for i in range(14):
        w1 = AI_VOCAB[i % len(AI_VOCAB)]
        w2 = AI_VOCAB[(i + 5) % len(AI_VOCAB)]
        w3 = AI_VOCAB[(i + 11) % len(AI_VOCAB)]
        # Fixed template length ≈ 20 non-whitespace chars.
        sentences.append(f"通过{w1}{topic}{w2}并{w3}相关安排。")
    rng.shuffle(sentences)
    path.write_text("".join(sentences) + "\n", encoding="utf-8")


def main() -> None:
    HUMAN_DIR.mkdir(parents=True, exist_ok=True)
    AI_DIR.mkdir(parents=True, exist_ok=True)
    for i, topic in enumerate(TOPICS, start=1):
        write_human(HUMAN_DIR / f"human_{i:02d}_{topic}.txt", topic, seed=i)
        write_ai(AI_DIR / f"ai_{i:02d}_{topic}.txt", topic, seed=i)
    print(f"Wrote {len(TOPICS)} human + {len(TOPICS)} AI files under {ROOT}")


if __name__ == "__main__":
    main()
