# RAG 工具后端与主控 Agent 边界讨论纪要

> 历史讨论纪要。边界结论仍然有效，但“RAG API 是纯工具 backend / 不带任何 agent 能力”的说法已经细化为：RAG API 不是面向用户的自主 agent，但可以运行有边界的模型辅助检索算子，例如三元组抽取、query entity extraction、relation/path rerank、chunk rerank。见 [2026-04-26 当前产品架构](/home/chuan/context-osv6/avrag-rs/docs/superpowers/specs/2026-04-26-current-product-rag-architecture.md)。

## 1. 讨论背景

最近几轮围绕两个问题展开：

1. 当前 RAG 主链路里，planner 受到 session context 污染，导致旧失败状态被重复放大。
2. 后续产品形态希望支持：
   - 前端内置主控 agent
   - 对外接入 OpenClaw 一类 assistant agent

在这个目标下，需要重新明确：

- 什么能力属于主控 agent
- 什么能力属于 RAG backend
- clarify / planning / retrieval / answer 的边界应该如何切分

---

## 2. 对当前实现的判断

当前实现更像：

- 单编排器下的双角色 LLM 流水线
  - planner：决定检索计划
  - answer synthesizer：根据检索结果生成回答
- 中间夹着一串确定性执行节点
  - dense / bm25 / rerank / summary policy / citation validate / output guard

不是严格意义上的多 agent 协作系统，更像：

- `plan -> execute retrieval pipeline -> synthesize answer`

其中 planner 权力很大：

- 若 `clarify_needed=true`，会直接短路后续检索规划
- answer 阶段只会把 `clarify_message` 原样返回

因此，一旦 planner 吃到被污染的 session context，就会把错误判断扩散到整条链路。

---

## 3. 已确认的问题根因

### 3.1 planner 污染的真实来源

planner 的 session context 当前包含两部分：

- `Conversation summary`
- `Recent messages`

这两部分都会被注入 planner。

本次失败案例中，污染 planner 的不是文档 summary，而是：

- session 原始对话中的连续失败回复
- 针对这些失败对话生成的 session summary

二者共同形成了“该文档当前不可检索”的错误先验。

### 3.2 已验证的缓解措施

已做过一次 prompt 级实验性调整：

- 明确要求 planner 只允许用 session history 做指代消解
- 不允许用 session history 参与 clarify 判断
- 若 session history 与 docscope / metadata 冲突，优先信 docscope / metadata

复放旧污染 session 后，这条问题已恢复为正常：

- `clarify_needed=false`
- planner 输出正常 query + bm25
- 检索成功并返回带 citations 的答案

这个结果证明：

- 旧失败历史确实会把 planner 往 clarify / 不检索方向拉偏
- planner 的边界需要重新设计，而不是继续扩大其自主解释空间

---

## 4. 当前讨论已经收敛出的架构方向

### 4.1 RAG API 的定位

已明确：

- RAG API 是检索工具 backend
- 不带用户级自主 agent 能力
- 可以包含 bounded model-assisted retrieval operators
- 不负责 clarify
- 不负责会话决策
- 不负责多轮对话管理

它只负责：

- 接收 plan schema
- 执行检索
- 返回 retrieval bundle

### 4.2 主控 agent 的定位

主控 agent 负责：

- 读用户输入
- 读 planning skill
- 判断是：
  - 直接 clarify
  - 还是输出 plan schema 调用 RAG API
- 接收 retrieval bundle
- 读 answer skill
- 生成最终用户回复

因此：

- 不需要单独再造一个独立 orchestrator 组件
- 主控 agent 自己就是 orchestration 所在的位置

换句话说：

- 不是不要 orchestration
- 而是 orchestration 内嵌进主控 agent

### 4.3 clarify 的处理方式

已明确：

- clarify 不应该传给用户 JSON schema
- clarify 也不需要作为 RAG API 的输入或输出契约

当主控 agent 判断需要澄清时：

- 直接用自然语言回复用户
- 不输出 retrieval plan
- 不调用 RAG API

因此，RAG API 只接受 plan schema，不处理 clarify 分支。

### 4.4 对外 / 对内统一方式

无论是：

- 前端网页中集成的主控 agent
- 外接 OpenClaw 类 assistant agent

都统一采用同一种模式：

- 主控 agent 负责 planning / clarify / answer
- 调用同一个 RAG API，并传入 plan schema
- RAG API 执行后返回 retrieval results / chunks / citations / trace

不做高低两层接口拆分。

---

## 5. 当前更偏向的系统边界

### 5.1 主控 agent 输入输出

主控 agent 输入：

- 用户自然语言
- 自身的 skills
- 自身维护的 session / memory / dialogue state

主控 agent 对 RAG API 的输出：

- 结构化 `plan schema`

主控 agent 对用户的输出：

- clarify 时：自然语言问题
- answer 时：自然语言回答

### 5.2 RAG API 输入输出

RAG API 输入：

- `plan schema`
- `doc_scope`
- 可选的执行预算 / ACL / trace metadata

RAG API 输出：

- retrieval bundle
  - chunks
  - citations
  - optional summary chunks
  - coverage
  - degrade trace
  - backend trace

RAG API 不接收：

- 用户原始问题
- session history
- session summary
- clarify decision
- agent memory

RAG API 不输出：

- 是否要 clarify
- 最终用户回复
- 多轮对话状态判断

---

## 6. 对 session memory 的当前倾向

当前讨论倾向是：

- 不要把 agent 的自由文本回复整体喂给 planning 阶段
- planner 污染的主要风险来自：
  - 旧失败回答
  - 被这些失败回答压缩后的 session summary

方向上更接近：

- session context 只保留对主控 agent 有价值的部分
- 尤其在 planning 阶段，避免把“历史失败状态”当成事实条件

这一块还没有形成最终定案，但已经有两个稳定结论：

1. 文档 summary 不是本次 planner 污染源。
2. planner / planning skill 不应该因为“之前没检索到”就再次决定不检索。

---

## 7. 当前已形成的核心原则

1. 主控 agent 决定“问还是查”，RAG backend 不做这个决定。
2. RAG backend 是工具，不是 agent。
3. clarify 是自然语言回合，不是给用户暴露的 schema。
4. 所有集成方都统一向 RAG API 传 plan schema。
5. session history 不能把历史失败状态持续放大到 planning 决策里。

---

## 8. 尚未最终定案的点

以下事项仍需继续讨论后再行动：

1. 主控 agent 的 planning skill 输出是否需要最小 envelope
   - 例如内部区分“可执行 plan”与“直接 clarify”
   - 还是由主控 agent 内部完全自行消化，不向外暴露

2. 主控 agent 在 planning 阶段读取哪类 memory
   - 只读用户消息
   - 还是读结构化 dialogue state
   - 还是保留一部分 session summary

3. RAG API 的正式 `plan schema` 契约
   - item 结构
   - summary_mode 表达方式
   - trace / coverage 返回结构

4. answer skill 是否完全外置到主控 agent
   - 还是保留 backend 内部可选的 answer synthesize 能力用于兼容旧链路

---

## 9. workspace 记忆层的极简设计方向

围绕 workspace memory，当前讨论已经从“多层细分状态机”收敛到更轻的版本。

核心判断是：

- 指代消解首先是结构性问题
- 在 workspace 场景下，前端勾选的文档本身已经是最强的指代消解信号
- `docscope + doc metadata` 应优先于 memory 成为 planning 的解释依据

因此，workspace memory 当前不再追求复杂分层，而是只保留：

1. 短期记忆
2. 长期记忆

### 9.1 指代消解优先级

当前更偏向的优先顺序是：

1. 当前前端选中的 `docscope`
2. `docscope` 对应的 `doc metadata`
3. 当前用户问题中的显式实体或文件名
4. workspace 短期记忆
5. workspace 长期记忆

这意味着：

- memory 只是补充信号
- 不是 planning 的第一信号
- 只有当 `docscope + metadata` 仍不足以完成指代消解时，才继续读取 memory

### 9.2 短期记忆

短期记忆的设计目标是：

- 最大化保留最近真实交互语境
- 最小化 assistant 重复失败回复对 planning 的污染

当前更偏向的设计：

- 只保留最近 `3-4` 轮原始对话
- 使用原文，不做摘要
- 在进入 planning 前做轻量去重

当前建议的最小去重机制：

- 若相邻问答中，assistant 回复与上一条 assistant 回复字符相似度超过 `80%`
- 则只保留最新一条

设计意图是直接压掉这种污染模式：

- “系统无法检索到……”
- “系统当前无法检索到……”
- “请确认文档是否已上传……”

这类高重复失败模板不应连续进入 planning 上下文。

### 9.3 长期记忆

长期记忆不采用开放式长摘要，而是采用更结构化的：

- `narrative + objects`

期望形态类似：

```json
{
  "narrative": "用户持续围绕中国核电 MD 项目纪要提问，重点关注主要内容、行动项和项目边界。",
  "objects": [
    {
      "type": "document",
      "id": "941a2035-6d3f-46e8-86f3-fdc6e51ff8a6",
      "name": "中国核电MD项目交流会议纪要0408.docx"
    },
    {
      "type": "topic",
      "name": "行动项"
    },
    {
      "type": "topic",
      "name": "项目边界"
    }
  ]
}
```

该结构的设计意图是：

- `narrative` 提供连续性和人类可读性
- `objects` 提供稳定锚点，便于后续做引用、匹配和轻量指代消解

### 9.4 长期记忆触发机制

长期记忆的更新更偏向：

- 以轮次触发
- 而不是每轮都总结

当前更偏向的简单策略：

- 每 `6-8` 轮触发一次长期记忆更新
- 或在会话阶段性结束时触发一次

这个策略的目标是：

- 避免每轮都总结带来的抖动和 token 浪费
- 也避免长期记忆更新过慢导致连续性丢失

### 9.5 planning 阶段对 memory 的使用原则

当前讨论已经明显倾向于：

- planning 优先读取 `docscope + metadata`
- 短期记忆只用于补充最近几轮会话语境
- 长期记忆不应默认强注入 planning

更合适的原则是：

- 先看 `docscope`
- 再看最近 `3-4` 轮短期记忆
- 只有这些仍不能完成解释时，才查长期记忆

这样可以减少长期记忆慢慢演化成新的污染源。

### 9.6 与 memvid 的关系

在当前方向下，`memvid` 更适合作为：

- 全局长期记忆层
- workspace 长期记忆层

不太适合作为：

- planning 执行态临时状态存储
- 高频短期对话窗口

因此当前更适合的组合是：

- `memvid`：持久化长期记忆
- 业务层轻状态：短期对话窗口和 planning 输入缓存

---

## 10. 当前建议的下一步

在继续编码前，优先完成以下设计确认：

1. 确认主控 agent 的 planning skill 最终输出契约
2. 确认 RAG API 的 `execute-plan` 请求 / 响应 schema
3. 确认 workspace memory 的最小实现
   - `docscope + metadata` 作为主指代消解器
   - 最近 `3-4` 轮短期记忆 + 去重
   - `narrative + objects` 长期记忆格式
4. 再决定是否把现有 backend 的 planner / answer 逻辑拆出

当前状态应视为：

- 已完成问题诊断
- 已完成 prompt 级验证
- 已完成工具边界方向收敛
- 已完成 workspace memory 的极简方向收敛
- 尚未进入正式架构改造实施
