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


def uid(doc_id: str, batch: int) -> str:
    if batch < 0:
        raise ValueError("batch")
    return f"{doc_id}#{batch}"


def parse_uid(s: str) -> tuple[str, int]:
    doc_id, sep, rest = s.rpartition("#")
    if not sep or not doc_id or not rest or not rest.isdigit():
        raise ValueError(s)
    return doc_id, int(rest)


def max_pool(hits: list[tuple[str, float]]) -> list[str]:
    best: dict[str, float] = {}
    for u, score in hits:
        doc_id, _ = parse_uid(u)
        prev = best.get(doc_id)
        if prev is None or score > prev:
            best[doc_id] = score
    return sorted(best, key=lambda d: (-best[d], d))


def read_char_window(body: str, offset: int, limit: int) -> str:
    chars = list(body)
    if offset < 0 or limit < 0:
        raise ValueError("offset/limit")
    return "".join(chars[offset : offset + limit])


def rg_scan(
    docs: list[tuple[str, str]], pattern: str, doc_ids: list[str], byte_budget: int
) -> list[dict]:
    if not doc_ids:
        raise ValueError("doc_ids required")
    import re

    rx = re.compile(pattern)
    allowed = set(doc_ids)
    hits = []
    used = 0
    by_id = {i: b for i, b in docs}
    for did in doc_ids:
        if did not in allowed:
            continue
        body = by_id[did]
        pos = 0
        for line in body.split("\n"):
            for m in rx.finditer(line):
                hits.append(
                    {
                        "doc_id": did,
                        "char_offset": pos + m.start(),
                        "line": line,
                    }
                )
                used += len(line.encode("utf-8"))
                if used >= byte_budget:
                    return hits
            pos += len(line) + 1
    return hits


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


def trim_keep_chars(chars: int, max_tokens: int, got_tokens: int) -> int:
    """docs/vgi/embed.md: keep = max(1, floor(chars * 0.97 * max_tokens / got_tokens))."""
    return max(1, int(chars * (max_tokens * 0.97) / got_tokens))


class EmbedContextOverflow(unittest.TestCase):
    def test_trim_keep_chars(self):
        # 96933#0 measured by the API: 300889 chars counted as 140166 tokens, limit 131072.
        #   300889 * 131072 * 0.97 / 140166 = 272926.2395… -> 272926
        self.assertEqual(trim_keep_chars(300_889, 131_072, 140_166), 272_926)
        #   10 * 131072 * 0.97 / 140166 = 9.071… -> 9
        self.assertEqual(trim_keep_chars(10, 131_072, 140_166), 9)
        # never zero
        self.assertEqual(trim_keep_chars(1, 131_072, 140_166), 1)


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


class Uid(unittest.TestCase):
    def test_uid(self):
        self.assertEqual(uid("a", 0), "a#0")
        self.assertEqual(parse_uid("79680#12"), ("79680", 12))
        for bad in ("a", "a#", "#0", "a#x"):
            with self.assertRaises(ValueError):
                parse_uid(bad)


class MaxPool(unittest.TestCase):
    def test_max_pool_three_docs(self):
        order = max_pool([("A#0", 0.90), ("A#1", 0.40), ("B#0", 0.80), ("C#0", 0.80)])
        self.assertEqual(order, ["A", "B", "C"])


class ReadRg(unittest.TestCase):
    def test_read_char_window(self):
        self.assertEqual(read_char_window("hello", 0, 5), "hello")
        self.assertEqual(read_char_window("hello", 1, 2), "el")
        self.assertEqual(read_char_window("hello", 10, 5), "")

    def test_rg_requires_doc_ids(self):
        with self.assertRaises(ValueError):
            rg_scan([("a", "hello")], "ell", [], 1000)

    def test_rg_ell_in_hello(self):
        hits = rg_scan([("a", "hello"), ("b", "world")], "ell", ["a", "b"], 1000)
        self.assertEqual(hits, [{"doc_id": "a", "char_offset": 1, "line": "hello"}])


class PassageWindows(unittest.TestCase):
    """retrieval.md 段落窗：raw token 流 W=512、stride=448、重叠 64；
    起点 0,448,… 持续至 start < max(n-64, 1)，窗尾截到 n。"""

    @staticmethod
    def passage_windows(n: int) -> list[tuple[int, int]]:
        if n == 0:
            return [(0, 0)]
        out = []
        s = 0
        while s < max(n - 64, 1):
            out.append((s, min(s + 512, n)))
            s += 448
        return out

    def test_passage_windows(self):
        self.assertEqual(
            self.passage_windows(600), [(0, 512), (448, 600)]
        )
        self.assertEqual(self.passage_windows(512), [(0, 512)])
        self.assertEqual(
            self.passage_windows(513), [(0, 512), (448, 513)]
        )


class Bm25Lucene(unittest.TestCase):
    """retrieval.md BM25 锚：Lucene 公式 k1=1.2 b=0.75。
    A='machine learning is fun' B='deep learning uses neural networks'
    C='vector databases store embeddings'，查询 learning。"""

    @staticmethod
    def bm25_scores(docs: list[list[str]], query: list[str], k1=1.2, b=0.75):
        import math

        n = len(docs)
        avgdl = sum(len(d) for d in docs) / n
        df = {t: sum(1 for d in docs if t in d) for t in set(query)}
        idf = {t: math.log(1 + (n - df[t] + 0.5) / (df[t] + 0.5)) for t in df}
        out = []
        for d in docs:
            dl = len(d)
            s = 0.0
            for t in query:
                if t not in d:
                    continue
                tf = d.count(t)
                s += idf[t] * tf * (k1 + 1) / (tf + k1 * (1 - b + b * dl / avgdl))
            out.append(s)
        return out

    def test_bm25_lucene_three_docs(self):
        docs = [
            "machine learning is fun".split(),
            "deep learning uses neural networks".split(),
            "vector databases store embeddings".split(),
        ]
        sa, sb, sc = self.bm25_scores(docs, ["learning"])
        self.assertAlmostEqual(sa, 0.48527451, places=6)
        self.assertAlmostEqual(sb, 0.44217447, places=6)
        self.assertEqual(sc, 0.0)
        self.assertGreater(sa, sb)


if __name__ == "__main__":
    unittest.main()
