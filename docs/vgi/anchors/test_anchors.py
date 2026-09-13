"""Hand-computed anchors for docs/vgi. Stdlib only.

Values were calculated independently of any vgi-rs implementation.
"""

from __future__ import annotations

import hashlib
import unittest
from collections import defaultdict


def sha256_utf8(s: str) -> str:
    return hashlib.sha256(s.encode("utf-8")).hexdigest()


def corpus_fp(revision: str, docs: list[dict]) -> str:
    rows = []
    for d in docs:
        body_sha = sha256_utf8(d["body"])
        row = (
            d["doc_id"]
            + "\0"
            + body_sha
            + "\0"
            + d.get("heading", "")
            + "\0"
            + d.get("url", "")
        )
        rows.append((d["doc_id"].encode("utf-8"), row))
    rows.sort(key=lambda x: x[0])
    payload = revision + "\n" + "\n".join(r for _, r in rows)
    return sha256_utf8(payload)


LIMIT = 7372
OVERLAP = 256
STRIDE = LIMIT - OVERLAP  # 7116
CAP = 64


def token_windows(n: int) -> tuple[list[tuple[int, int]], bool]:
    if n == 0:
        return [(0, 0)], False
    if n <= LIMIT:
        return [(0, n)], False
    out: list[tuple[int, int]] = []
    i = 0
    while True:
        st = i * STRIDE
        en = st + LIMIT
        out.append((st, en))
        if en >= n or len(out) == CAP:
            break
        i += 1
    if out[-1][1] >= n:
        st, _ = out[-1]
        return out[:-1] + [(st, n)], False
    return out, True


def rrf_merge(lists: list[list[str]], k: int = 60) -> list[str]:
    score: dict[str, float] = defaultdict(float)
    hits: dict[str, int] = defaultdict(int)
    vec_rank: dict[str, int] = {}
    for li, ranked in enumerate(lists):
        for rank, doc in enumerate(ranked, start=1):
            score[doc] += 1.0 / (k + rank)
            hits[doc] += 1
            if li == 0:
                vec_rank[doc] = rank
    docs = set(score)
    return sorted(
        docs,
        key=lambda d: (-score[d], -hits[d], vec_rank.get(d, 10**9), d),
    )


def recall_at_k(evidence: set[str], hits: list[str], k: int) -> float:
    if not evidence:
        return 0.0
    return len(set(hits[:k]) & evidence) / len(evidence)


class CorpusFp(unittest.TestCase):
    def test_corpus_fp_two_doc(self):
        docs = [
            {"doc_id": "a", "body": "hello", "heading": "", "url": ""},
            {"doc_id": "b", "body": "world", "heading": "W", "url": "http://x"},
        ]
        self.assertEqual(
            corpus_fp("rev0", docs),
            "5f76e86df681122f018c02203b95a682d692f37e5c88de93ba3643c786d500b2",
        )
        shuffled = [docs[1], docs[0]]
        self.assertEqual(corpus_fp("rev0", shuffled), corpus_fp("rev0", docs))
        self.assertNotEqual(
            corpus_fp("rev0", docs),
            "eab63d9148e8702392dc1176a1db6c957c06e4a6a7b813518c9fd1a2297aeb2e",
        )


class TokenWindows(unittest.TestCase):
    def test_token_windows(self):
        cases = {
            0: ([(0, 0)], False),
            100: ([(0, 100)], False),
            7372: ([(0, 7372)], False),
            7373: ([(0, 7372), (7116, 7373)], False),
            8000: ([(0, 7372), (7116, 8000)], False),
        }
        for n, (want, trunc) in cases.items():
            got, t = token_windows(n)
            self.assertEqual(got, want, n)
            self.assertEqual(t, trunc, n)
        w, t = token_windows(455680)
        self.assertEqual(len(w), 64)
        self.assertEqual(w[-1], (448308, 455680))
        self.assertFalse(t)
        w, t = token_windows(455681)
        self.assertEqual(len(w), 64)
        self.assertEqual(w[-1], (448308, 455680))
        self.assertTrue(t)


class Rrf(unittest.TestCase):
    def test_rrf_four_docs(self):
        order = rrf_merge([["A", "B", "C"], ["B", "A", "D"]], k=60)
        self.assertEqual(order, ["A", "B", "C", "D"])
        sA = 1 / 61 + 1 / 62
        sC = 1 / 63
        self.assertAlmostEqual(sA, 0.032522474881015, places=12)
        self.assertAlmostEqual(sC, 0.015873015873016, places=12)
        self.assertGreater(sA, sC)


class Recall(unittest.TestCase):
    def test_recall_at_k(self):
        evidence = {"D1", "D2", "D3"}
        hits = ["D9", "D1", "D4", "D2"]
        self.assertEqual(recall_at_k(evidence, hits, 100), 2 / 3)


if __name__ == "__main__":
    unittest.main()
