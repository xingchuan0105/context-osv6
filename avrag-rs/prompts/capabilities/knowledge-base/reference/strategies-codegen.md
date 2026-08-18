# 沙箱 codegen 噪声（knowledge-base/strategies-codegen）

按需加载：`{"skill_request": ["knowledge-base/strategies-codegen"]}`。  
方法签名以 **knowledge-base** skill 为准；失败形态见本轮 `[sandbox_error]`。

本文件只谈 **沙箱写码形态**：一次可执行、少噪声。检索策略（entity-first / 表 / grounding）在对应 spoke。

## 写码形态（观察）

- 独立子查询可用 `asyncio.gather`；有前后依赖的链（先 catalog 再 query）串行更稳。
- 回传消费侧：打印 **短摘要 / id / 计数** 比整段 chunk 正文更不易挤爆窗口；大段 dump 时后续轮上下文常被噪声占满。

下列 few-shot 使用**与评测语料无关的虚构域**，只说明形态。

## Few-shot

### FS-C1 — 一次可执行 dense（无旧名、无 top_k）

**情境**：要核验虚构实体「北麓茶庄」的注册地字段。  
**观察**：代码块内为 `hits = await client.dense("北麓茶庄 注册地")`，再 `print` 命中条数与前几条 id/短摘；无 `import os`、无 `top_k=`、无 `dense_search`。  
**读出**：stderr 空且 tool_trace 出现 `dense_retrieval` Ok 时，说明本轮至少完成了契约内检索。

### FS-C2 — 依赖链不硬 gather

**情境**：要查虚构表「季度出货」里某 SKU 行数。  
**观察**：先 `await client.struct_catalog()`（或等价 catalog 面）得到表 id，再 `await client.struct_query(...)`；两步串行。  
**读出**：catalog 未回 Ok 前并行 query，常得到空/错参；依赖边满足后再 query，total_hits 才有计数语义。

## Gotchas

| 现象 | 回传实际含义 | 常见误读 / 噪声来源 |
|------|--------------|-------------------|
| 有依赖却 `asyncio.gather` 并行 | 依赖边未满足 | 把「独立可并行」套到有依赖链 |
| `import os` / `subprocess` 等 | 沙箱禁止 → 执行失败 | 想绕过 client |
| 只 `print` 大段 chunk 正文 | 回传窗口被占满 | 多 print 当证据更全 |
| 首轮贴多段试错式重复 client 调用 / Ok 后仍追加调用 | 同 seed 重复占轮次，且追加调用可能 err | 把「多写几遍」当召回策略 |

## Ok 后收口

- 一轮内已有足以作答的 Ok 回传时，轨迹里更稳的读法是：立刻消费（短摘要 / id / 计数）并收口。
- 为「更全」继续追加检索或大段打印时，成功侧主要增加噪声，失败侧直接产生 err；两者都不自动提高证据强度。
