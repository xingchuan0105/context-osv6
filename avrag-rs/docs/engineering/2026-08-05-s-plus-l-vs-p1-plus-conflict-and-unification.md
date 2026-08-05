# S+L 邻并 × 多轮 P1+：冲突分析与统一方案

| 项目 | 内容 |
|------|------|
| 日期 | 2026-08-05 |
| 状态 | **U-P0～U-P3 + working-set char 裁剪 + P1″ claim_notes 已实现**（member 闭包 reseen、visibility、history stub、`WORKING_SET_CHAR_BUDGET` 近轮 demote、`[claim_notes]` 板） |
| 相关 | `2026-08-05-retrieval-adjacent-shortlist-merge-design.md`；`2026-08-05-multi-round-retrieval-context-management-design.md`；现码 `merge::adjacent_merge_*`、`bridge` reseen |
| 验收锚 | q017（邻并增益）；q141（多轮噪声 / 假拒） |

---

## 0. 一句话

**S+L 解决「单次检索可见包里的语义断裂」；P1+ 解决「多轮可见包体积与注意力」。二者操作同一批 model-visible 证据，必须共用身份模型与固定流水线顺序，否则会出现重复灌正文、snippet 砍掉拉近的尾段、working-set 按条数误判等冲突。**

---

## 1. 两机制各自在做什么

### 1.1 S+L 邻并（已实现，默认开）

```text
retrieve → pool C
L := C 前 L_max（cut 前）
S := final_cut(C)
E := adjacent_merge(S, L)   // 同 doc + |Δcursor|≤r
→ tool result chunks = E
```

| 动作 | 对 model-visible 的影响 |
|------|-------------------------|
| S 内邻块拼接 | 条数 ↓，字数 ≈ 不变 |
| **L\S 拉近** | **新增** 原本 cut 掉的邻块全文（q017 的价值） |
| 输出形态 | 一条 `ScoredChunk`：`chunk_id = run[0]`，`content = join(run)` |

### 1.2 多轮 P0～P1+（P0 已实现；P1+ 未齐）

| 档 | 意图 |
|----|------|
| **P0** | 同 `chunk_id` 再命中 → `reseen` + body 空 |
| **P0′** | 饱和（new_ratio 低）→ 本轮收紧回传形态 |
| **P1** | 默认可视 = **卡片**（alias/doc/score/snippet）；全文仅 expanded 集合 |
| **P1′** | 历史 observation **clearing** + working-set 顶 K 全文 |
| **P1″** | durable **notes/claim 板** 作主记忆 |

管道位：

```text
tool 返回 → bridge 别名/reseen → 写入 messages
… 多轮 …
LLM 边界：transformContext（clearing / working-set）→ 模型
```

---

## 2. 冲突点（机制级）

### C1. 身份分裂：合并 run vs 原子 chunk_id

| 现象 | 后果 |
|------|------|
| 邻并把 A+B 拼成一条，**只保留 A 的 `chunk_id`** | durable / reseen 只登记 A |
| 下轮检索再命中 **B** | B 不在 `seen_chunk_aliases` → **再灌 B 全文**，而历史 `#n` 已含 B 正文 |
| SELECTED / cite | 只能点到 A；B 无独立 alias |

**根因：** 邻并把「语义 run」压成单 id，P0 reseen 仍按 **原子 chunk_id** 记账。

### C2. 体积目标相反（同层叠加）

| 机制 | 体积目标 |
|------|----------|
| S+L 拉近 | 故意 **增加** 可见字数（换语义完整） |
| P1 / P1′ | 故意 **减少** 可见字数 |

不是逻辑矛盾，而是 **未约定预算账户**：拉近花的字数应从哪扣？working-set K 按「条」还是「token」？

### C3. 卡片/snippet 与「拉近」目的相克

- 拉近是为了让模型看到 **(1)(2)(3)(4) 整段**。  
- 若 P1 对合并条只留 **前 200 字 snippet**，第四点（往往在 run 尾部）被砍掉 → **邻并白做**。  
- 若合并条一律 full expand，又与「默认卡片」冲突。

### C4. 流水线顺序未定义

当前实现近似：

```text
dense/lexical: cut → S+L merge → JSON
bridge: alias + reseen(omit body)
→ 整包进 observation / messages（历史不清）
```

P1′ 若在 **LLM 边界** 再 clear/snip，可能：

- 先 merge 拉全文 → 再被历史 clear 掉（OK）  
- 或本轮 delta 被错误当成「旧」snip（坏）  
- 饱和收紧若在 merge **前** 砍 S，L 邻居对不上锚；在 merge **后** 砍 E，又可能砍掉刚拉近的邻块

### C5. Working-set「顶 K 条」与 mega-chunk

合并后 1 条 = 2～4 个原子 chunk 的字数。  
若 K 按 **条数**：一条 mega 挤掉其他文档。  
若 K 按 **token**：合理，但必须在 **merge 之后** 计量。

### C6. Durable pool / notes 记什么

| 记法 | 问题 |
|------|------|
| 只记锚 chunk_id | 丢邻块 id，与 C1 相同 |
| 只记 alias | 跨轮 reseen 可对齐，但需 run 级成员表 |
| 笔记引用「#3 含 (1)–(4)」 | 需要 host 暴露 `member_chunk_ids` |

### C7. 验收题张力（产品）

| 题 | 需要 |
|----|------|
| **q017** | 可见包内 **连续语义**（拉近 + 全文可读） |
| **q141** | 可见包 **别被同文噪声淹没**（少 body、清历史） |

同一邻文：q017 要邻块进来；q141 要少灌邻主题噪声。  
**不能**「全局多拉」或「全局少贴」单开关解决，要 **身份 + 预算 + 形态** 分层。

---

## 3. 统一原则

| # | 原则 |
|---|------|
| U1 | **原子 chunk 永不丢身份**：merge 只改 **展示 run**，成员 id 全进 durable map |
| U2 | **先语义整形，再可见裁剪**：S+L → 入 pool → 再决定 card/full/clear |
| U3 | **拉近消耗 working-set 预算**：L\S pull 与「expanded 全文」共用 token 账户 |
| U4 | **列表连续 run 默认 expanded**；普通 hit 默认 card（保护 q017，约束 q141） |
| U5 | **跨轮 reseen 对成员闭包生效**：run 内任一 member 见过 → 再命中只 reseen |
| U6 | Host 只报告形态；是否再 dig 仍 model+skill |

---

## 4. 统一身份：EvidenceRun

### 4.1 结构（逻辑）

```text
EvidenceRun {
  run_id:         stable  // e.g. hash(doc_id, min_cursor, max_cursor) or first alias
  member_ids:     [chunk_id…]  // cursor 升序
  anchor_id:      chunk_id     // score 最高的原 S 成员（cite 默认）
  doc_id, cursor_lo, cursor_hi
  full_text:      join(members)  // durable only
  score:          max(S members)
  flags:          { adjacent: bool, pulled_n: u8 }
}
```

- **Bridge alias `#n` 绑在 run 上**（不是绑在裸 chunk 上）。  
- Durable：`chunk_id → run_id`、`run_id → EvidenceRun`。  
- 原子命中且无邻并时：`member_ids = [self]` 的退化 run。

### 4.2 现码迁移

| 现状 | 目标 |
|------|------|
| merge 输出单 `chunk_id=run[0]` | 增加 `member_chunk_ids: Vec<Uuid>`（JSON 下发） |
| `seen_chunk_aliases: cid → #n` | 改为 **成员闭包**：任一 member 命中 → 同一 `#n` + omit |
| SELECTED `#n` | 仍解析到 **anchor_id**（或 primary cite id）；评测 gold 可对 **任一 member 正文** 匹配 |

---

## 5. 统一流水线（权威顺序）

单次 `client.dense` / `lexical` 调用：

```text
① retrieve → C
② L := pool cap；S := final_cut
③ S+L adjacent_merge → runs[]     // 语义整形；只动结构，不决策 card
④ durable_pool.upsert(runs)       // 全文 + member 映射；不进模型
⑤ visibility_plan(runs):
     - 标记 adjacent|pulled → prefer_expand
     - 新 run 且非饱和 → 可 expand
     - 饱和 / 超 budget → card only
     - 已在 pool 且本轮非 force → reseen / body omit
⑥ bridge: 赋 #alias（run 级）；按 plan 填 text 或 snippet+body_omitted
⑦ observation 进 messages（本轮 delta）
```

多轮 LLM 边界（每轮 assemble 前，≈ pi `transformContext`）：

```text
⑧ 历史 messages 中 retrieval observation：
     - 近 R 轮 delta 保持 plan 时形态
     - 更早：stub（保留 alias 列表 + reseen 指针）
⑨ working_set：按 **token** 预算保留 expanded 全文
     - 优先：本轮 expanded、SELECTED、adjacent runs
     - 其次：高 score / 近轮
⑩ 注入 [evidence_index] + notes 板（P1″，可渐进）
→ convert_to_llm → 模型
```

**禁止的顺序：**

- 先 card/snippet 再 S+L（尾段丢失）  
- 先 clear 本轮 delta 再给模型  
- reseen 只认锚 id 不认 members  

---

## 6. 可见形态与预算（解决 C2/C3/C5）

### 6.1 三态（每个 run）

| 态 | 模型看到 | 何时 |
|----|----------|------|
| **expanded** | 全文（或 merge 全文） | 本轮新 adjacent run；或 SELECTED/hydrate；或 working-set 命中 |
| **card** | alias, doc, score, snippet（头尾或头 N 字） | 默认新普通 hit；饱和时 |
| **stub** | `reseen:#n` / `body_omitted` | 跨轮已见 member；历史 clear 后 |

### 6.2 预算账户（建议默认）

| 账户 | 建议 | 说明 |
|------|------|------|
| `pull_budget` | 8 **原子** chunk 或 4 **run** | S+L 从 L\S 拉近上限（已有思想） |
| `expand_token_budget` | 如 6k～12k / 轮 | 本轮 expanded 全文总和 |
| `working_set_token_budget` | 如 8k～16k | 跨轮常驻全文 |
| `history_full_rounds` | 1～2 | 更早 observation → stub |

**计量单位：token（或 utf-8 字符代理），不要只数条数。**  
Adjacent mega-run 按 `len(full_text)` 记账，一条可吃掉多份「原子额度」。

### 6.3 保护 q017

```text
if run.flags.adjacent || run.pulled_n > 0:
    prefer_expand = true   // 进入 expand 队列头部
    snippet 不得替代 expand，除非超 expand_token_budget 才降级为
    「头尾双 snippet + 明示 truncated_adjacent」
```

超预算时：**先缩非 adjacent 的 expand**，最后才截 adjacent（并 observation 报告截断）。

### 6.4 约束 q141

```text
饱和 new_ratio 低 → prefer_expand 仅限 adjacent|SELECTED
普通 dense 噪声 → card only
历史 clear → 舆论块不在近 R 轮则 stub，不占 working-set
```

邻并仍可能拉同文邻块：若邻块是舆论段，**应用 cursor 邻接，不是主题过滤**（产品不在检索层做语义禁运）。减噪靠 **预算 + 历史 clear + 少 dig**，不靠关邻并。

---

## 7. Reseen 闭包（解决 C1）

```text
on first emit(run):
  for id in run.member_ids:
    chunk_to_run[id] = run_id
  run_to_alias[run_id] = #n

on later hit(chunk_id=X):
  if let Some(run_id) = chunk_to_run[X]:
    emit reseen: alias=run_to_alias[run_id], body_omitted=true
    // 可选：若 X 曾不在 member 展示中，仅 index 更新，不扩文
```

同轮 dense 先 merge 出 A+B，再登记 A、B → 下轮只出 B 也不会二次全文。

---

## 8. 与现码差距（实现清单）

| 组件 | 现码 | 统一后 |
|------|------|--------|
| `adjacent_merge_*` | 丢 member id，单 content | 输出 run + `member_chunk_ids` |
| `seen_chunk_aliases` | 单 cid→alias | 闭包登记全体 members |
| bridge 正文 | 首见全文 / reseen 空 | 按 visibility_plan：expand/card/stub |
| `assemble_retrieve` | 原样堆 messages | ⑧⑨ transformContext |
| tool_trace | 可选 | `adjacent_*` + `expand_tokens` + `cleared_n` |
| 评测 | first-seen 正文 | gold 可对 member 任一正文；cite 仍 anchor |

**P1″ notes：** 可与 ⑧⑨ 同波或紧随：host 每轮追加「本轮新 expand 的 1 行事实摘录」到 `state.evidence_notes`（第三称、有上限），**不**要求模型写笔记工具。

---

## 9. 决策记录

| # | 议题 | 决定 |
|---|------|------|
| D1 | 是否关邻并给 P1+ 让路 | **否**；邻并保留默认开，与 P1+ 共预算 |
| D2 | merge 是否继续单 chunk_id | **否**；演进为 EvidenceRun + members |
| D3 | 卡片是否默认砍 adjacent | **否**；adjacent 优先 expand |
| D4 | working-set 按条还是 token | **token** |
| D5 | 流水线顺序 | **§5 固定**：merge → pool → plan → bridge → history transform |
| D6 | q141 噪声 | 预算 + clear + 饱和；不关 S+L |

---

## 10. 落地阶段（实现时）

```text
U-P0  member_chunk_ids 输出 + reseen 闭包登记          // 修 C1 双灌  ✓
U-P1  visibility_plan + expand_token_budget
      adjacent 优先 expand；普通 card                  // 修 C3  ✓
U-P2  assemble 前 history stub + working_set char      // 修 C2/C5/q141  ✓
      （HISTORY_FULL_RETRIEVAL_ROUNDS + WORKING_SET_CHAR_BUDGET demote）
U-P3  evidence_index + claim_notes 板（P1″） + 遥测字段  ✓
U-P4  可选：饱和时 pull_budget 降档
```

验收：

| 题 | 期望 |
|----|------|
| q017 | L 邻点进 expand 全文；终答能覆盖断裂点 |
| q141 | 多轮后 visible token 不线性涨；假拒率下降 |
| 单测 | merge 后 B 再命中 → reseen 无 body；adjacent run 超预算才截断尾部 |

---

## 11. 非目标

- 用主题分类拒绝「舆论邻块」拉近（检索层不做内容审查）  
- Host 禁止再 dig  
- 取消 S+L 或取消 P1+ 任一端  

---

## 12. 摘要

| 冲突 | 解法 |
|------|------|
| 身份 | EvidenceRun + member 闭包 reseen |
| 体积 | 共用 expand/working-set **token** 预算 |
| 卡片砍尾 | adjacent **优先 expand** |
| 顺序 | merge → pool → plan → bridge → history transform |
| 产品题 | q017 靠 expand 邻并；q141 靠预算与历史 clear |

**下一实现入口：U-P0（member 闭包）——小 diff、立刻消掉「合并后再灌邻块全文」。**
