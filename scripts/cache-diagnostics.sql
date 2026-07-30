-- cache-diagnostics.sql — Prompt-cache 健康度 & execute 轮数分布
--
-- 用途：为"是否投入 prompt-cache 优化"提供数据基础。
--   查询 A：cache 命中率（按 feature / provider / model）——看跨租户驱逐多严重
--   查询 B：execute 轮数分布（按 feature）——验证 BUG-2(Anthropic 末尾断点) 的 ROI
--           （execute 平均 ≥3 轮 → 写一次 cache 读多次，净赚）
--   查询 C：按租户的 cache 命中率——看是否需要 key 池 + 租户亲和路由
--
-- ⚠️ provider 语义差异（影响命中率分母）：
--   DeepSeek/OpenAI : prompt_tokens 已含 cache hit → cache_hit_pct = cached / prompt
--   Anthropic       : prompt_tokens(input_tokens) 不含 cache → 分母用 prompt + cached
--                     （cache_creation 另算，BUG-1 修复后可在查询 D 体现）
--
-- 运行：
--   PGPASSWORD=avrag psql -h 127.0.0.1 -U avrag -d avrag_rs -f scripts/cache-diagnostics.sql
--   （可在生产库跑；只读 SELECT，无副作用）


-- ════════════════════════════════════════════════════════════════════════
-- 查询 A：cache 命中率 by feature / provider / model（近 7 天，billable）
-- ════════════════════════════════════════════════════════════════════════
\echo '== A: cache hit rate by feature/provider/model (7d, billable) =='

SELECT
    feature,
    provider,
    model,
    COUNT(*)                                          AS calls,
    SUM(prompt_tokens)                                AS prompt_tok,
    SUM(cached_tokens)                                AS cached_tok,
    -- DeepSeek/OpenAI: cached/prompt；Anthropic: cached/(prompt+cached)
    CASE
      WHEN provider = 'anthropic'
        THEN ROUND(100.0 * SUM(cached_tokens)
                   / NULLIF(SUM(prompt_tokens) + SUM(cached_tokens), 0), 2)
      ELSE ROUND(100.0 * SUM(cached_tokens)
                 / NULLIF(SUM(prompt_tokens), 0), 2)
    END                                               AS cache_hit_pct,
    SUM(total_tokens)                                 AS total_tok,
    SUM(usage_units)                                  AS units
FROM llm_usage_events
WHERE created_at > NOW() - INTERVAL '7 days'
  AND billable = true
GROUP BY feature, provider, model
ORDER BY prompt_tok DESC;


-- ════════════════════════════════════════════════════════════════════════
-- 查询 B：execute 轮数分布 by feature（近 7 天）
-- 同一 request_id 下的 llm_usage_events 行数 ≈ 该 execute 的 LLM 调用轮数
-- ════════════════════════════════════════════════════════════════════════
\echo '== B: execute rounds distribution by feature (7d) =='

WITH exec_rounds AS (
    SELECT
        request_id,
        feature,
        MIN(provider) AS provider,   -- execute 内 provider 应一致
        COUNT(*)       AS rounds
    FROM llm_usage_events
    WHERE created_at > NOW() - INTERVAL '7 days'
      AND billable = true
      AND request_id IS NOT NULL
      AND request_id <> ''
    GROUP BY request_id, feature
)
SELECT
    feature,
    provider,
    COUNT(*)                                         AS executes,
    ROUND(AVG(rounds), 2)                            AS avg_rounds,
    PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY rounds) AS p50,
    PERCENTILE_CONT(0.9) WITHIN GROUP (ORDER BY rounds) AS p90,
    MAX(rounds)                                      AS max_rounds,
    COUNT(*) FILTER (WHERE rounds >= 3)              AS exec_ge3,
    ROUND(100.0 * COUNT(*) FILTER (WHERE rounds >= 3) / NULLIF(COUNT(*), 0), 1)
                                                     AS pct_ge3_rounds
FROM exec_rounds
GROUP BY feature, provider
ORDER BY executes DESC;


-- ════════════════════════════════════════════════════════════════════════
-- 查询 C：按租户 cache 命中率（近 7 天，agent_loop 为主）
-- 命中率方差大 → 跨租户 cache 驱逐严重 → 考虑 key 池 + 租户亲和路由
-- ════════════════════════════════════════════════════════════════════════
\echo '== C: per-tenant cache hit rate (7d, agent feature) =='

SELECT
    user_id,
    COUNT(*)                                          AS calls,
    SUM(prompt_tokens)                                AS prompt_tok,
    SUM(cached_tokens)                                AS cached_tok,
    ROUND(100.0 * SUM(cached_tokens)
          / NULLIF(SUM(prompt_tokens), 0), 2)         AS cache_hit_pct
FROM llm_usage_events
WHERE created_at > NOW() - INTERVAL '7 days'
  AND billable = true
  AND feature IN ('agent_loop', 'chat', 'rag', 'search')
  AND provider <> 'anthropic'   -- 见查询 A 语义说明；Anthropic 单独看
GROUP BY user_id
HAVING SUM(prompt_tokens) > 1000    -- 过滤噪声：只看有实际用量的租户
ORDER BY prompt_tok DESC
LIMIT 50;
