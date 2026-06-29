# Agent Harness 三项升级设计 (2026-05-12)

> 状态：**已回撤** (2026-05-12)
> 原因：产品定位调整，核心聚焦知识库检索+网络检索，Agent 协作由用户自选的 Claude Code/Hermes 提供。
> 历史：本文记录 2026-05-12 对照 `shareAI-lab/learn-claude-code` 的调研结论，曾作为实施依据，现已废弃。
> 补充：L2 `session_summary` 已于 2026-06 移除，见 `avrag-rs/docs/adr/0007-react-phased-context-disclosure.md`。

---

## 回撤说明

原设计的三项升级（tool-use 循环、滑动窗口、Skill 按需加载）在调研后发现与 COS6 产品定位不符：

1. **产品核心**：知识库检索 + 网络检索，不是 Agent 框架
2. **页面形态**：3 个 Agent 提供人机协作范式（检索→分析→生成，流水线串行）
3. **本地安装**：用户用 Claude Code / Hermes 接入，COS6 提供 MCP Server 暴露检索能力
4. **不做**：比 Claude Code / Hermes 更厉害的 Agent Harness

原代码已实现 Phase A-D，于 2026-05-12 全部回撤（commit `f8407c1`）。

---

## 保留的结论（对当前架构仍有参考）

### 1. Skill 加载机制（两层设计）

```
Layer 1 (启动): 扫描 skills/ 目录，把 name + description 注入 system prompt
Layer 2 (按需): 模型调用 load_skill("pdf") 时返回完整内容
```

**价值**：避免一次性塞入所有 skill 内容导致 token 爆炸。

**COS6 当前**：未实现，skill 内容直接编译进 system prompt。如需优化可参考此两层设计。

### 2. 对话压缩策略

```
Layer 1 (Hot):  最近 N 轮完整保留
Layer 2 (Warm): 上一段窗口摘要
Layer 3 (Cold): 更早的只保留关键事实
```

**价值**：长会话 token 控制。

**COS6 当前**：`session_summary` 单点摘要，可优化为三层滑动。

### 3. 缓存策略

```
共享 context 块（session + prefs + memory）→ 两次调用完全复用 → 缓存前缀命中
```

**价值**：Plan+Answer 模式下减少 token 重复。

---

## 废弃的内容

- AgentLoop 共享循环驱动
- AgentToolRegistry 工具注册表
- 多 Agent 协作（队友、消息总线、任务认领）
- 后台任务管理（BackgroundManager）
- 关闭协议 / 计划审批

---

## 关联文档状态

| 文档 | 状态 |
|------|------|
| `2026-05-12-agent-harness-upgrade-implementation.md` | 已废弃 |
| `2026-05-12-architecture-baseline.md` | 需更新引用 |
