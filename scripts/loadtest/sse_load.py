#!/usr/bin/env python3
"""SSE load generator for avrag-rs (asyncio + httpx, no extra deps beyond httpx).

Simulates the product's dominant load shape: many concurrent long-lived SSE
chat streams. Reports per-stage P50/P95/P99, time-to-first-byte, error rate.

Usage:
  python3 sse_load.py --base http://127.0.0.1:18081 --owner <uuid> \
      --stages 10:60,25:60,50:120,100:120   # vus:seconds pairs
      [--query "hi"] [--think-ms 500] [--out result.json]

Auth: x-owner-user-id header (shadow-stack/dev auth path).
"""
import argparse
import asyncio
import json
import statistics
import time

import httpx


def pct(values, p):
    if not values:
        return 0.0
    values = sorted(values)
    k = max(0, min(len(values) - 1, int(round((p / 100.0) * (len(values) - 1)))))
    return values[k]


async def one_turn(client, base, owner, query, results, think_ms):
    headers = {"x-owner-user-id": owner, "Content-Type": "application/json"}
    payload = {
        "query": query,
        "agent_type": "chat",
        "stream": True,
    }
    t0 = time.monotonic()
    status = None
    ttfb = None
    try:
        async with client.stream(
            "POST", f"{base}/api/v1/chat", headers=headers, json=payload
        ) as resp:
            status = resp.status_code
            async for _line in resp.aiter_lines():
                if ttfb is None:
                    ttfb = time.monotonic() - t0
            ok = status == 200
    except Exception as e:  # noqa: BLE001 - record and move on
        results.append({"ok": False, "status": status, "error": str(e)[:120]})
        return
    dur = time.monotonic() - t0
    results.append({"ok": ok, "status": status, "dur": dur, "ttfb": ttfb or dur})
    if think_ms:
        await asyncio.sleep(think_ms / 1000.0)


async def run_stage(base, owner, query, vus, seconds, think_ms, stage_results):
    limits = httpx.Limits(max_connections=vus * 2, max_keepalive_connections=vus * 2)
    timeout = httpx.Timeout(None, connect=10.0)
    async with httpx.AsyncClient(limits=limits, timeout=timeout) as client:
        deadline = time.monotonic() + seconds

        async def vu_loop():
            while time.monotonic() < deadline:
                await one_turn(client, base, owner, query, stage_results, think_ms)

        await asyncio.gather(*(vu_loop() for _ in range(vus)))


def report(stage_name, vus, seconds, results):
    ok = [r for r in results if r.get("ok")]
    errs = [r for r in results if not r.get("ok")]
    durs = [r["dur"] for r in ok]
    ttfbs = [r["ttfb"] for r in ok]
    print(
        f"[{stage_name}] vus={vus} window={seconds}s "
        f"done={len(results)} ok={len(ok)} err={len(errs)} ({100.0*len(errs)/max(1,len(results)):.1f}%)"
    )
    if durs:
        print(
            f"  dur  P50={pct(durs,50):.2f}s P95={pct(durs,95):.2f}s P99={pct(durs,99):.2f}s "
            f"| ttfb P50={pct(ttfbs,50):.2f}s P95={pct(ttfbs,95):.2f}s "
            f"| throughput={len(ok)/seconds:.1f} req/s"
        )
    if errs:
        from collections import Counter

        kinds = Counter(str(e.get("error") or f"status={e.get('status')}")[:60] for e in errs)
        for k, n in kinds.most_common(5):
            print(f"  err[{n}] {k}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--base", required=True)
    ap.add_argument("--owner", required=True)
    ap.add_argument("--stages", default="10:60,25:60,50:120,100:120")
    ap.add_argument("--query", default="用一句话解释什么是复利？")
    ap.add_argument("--think-ms", type=int, default=500)
    ap.add_argument("--out", default=None)
    args = ap.parse_args()

    all_stage_results = {}
    for spec in args.stages.split(","):
        vus, seconds = (int(x) for x in spec.split(":"))
        results = []
        print(f"--- stage vus={vus} for {seconds}s ---", flush=True)
        t0 = time.monotonic()
        asyncio.run(run_stage(args.base, args.owner, args.query, vus, seconds, args.think_ms, results))
        wall = time.monotonic() - t0
        report(f"stage-{vus}", vus, wall, results)
        all_stage_results[str(vus)] = results

    if args.out:
        with open(args.out, "w") as f:
            json.dump(all_stage_results, f)
        print(f"saved {args.out}")


if __name__ == "__main__":
    main()
