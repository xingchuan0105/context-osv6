# GitNexus CLI 使用指南 & 代码审查技能总结

> 生成时间: 2026-04-26
> 项目: context-osv6

---

## GitNexus CLI 已学会

### 安装状态
- ✅ `gitnexus` 已安装在 `~/.nvm/versions/node/v24.13.0/bin/gitnexus`
- ✅ context-osv6 已索引: **12,071 nodes | 25,953 edges | 579 clusters | 300 flows**

### 核心命令

```bash
# 1. 分析/索引仓库
gitnexus analyze [path]              # 索引当前仓库
gitnexus analyze --force             # 强制重新索引
gitnexus analyze --embeddings        # 启用语义搜索

# 2. 查询知识图谱
gitnexus query "search term" -r context-osv6 -l 10 --content
#   -r: 指定仓库
#   -l: 限制结果数
#   --content: 包含完整源码

# 3. 符号360度视图
gitnexus context "SymbolName" -r context-osv6
#   显示: callers, callees, processes

# 4. 影响分析（改动爆炸半径）
gitnexus impact "SymbolName" -r context-osv6
#   显示: 改动会影响哪些模块/流程

# 5. 原始 Cypher 查询
gitnexus cypher "MATCH (n) RETURN n LIMIT 10" -r context-osv6

# 6. 状态检查
gitnexus status                      # 当前仓库索引状态
gitnexus list                        # 所有索引的仓库
```

### 已验证的查询示例

```bash
# 查询 entity extraction 相关代码
gitnexus query "entity extraction" -r context-osv6 -l 10 --content

# 查询 graph retrieval 实现
gitnexus query "graph retrieval" -r context-osv6 -l 10 --content

# 查看 RetrievalPlanner 实现上下文
gitnexus context "Impl:avrag-rs/crates/llm/src/planner.rs:RetrievalPlanner" -r context-osv6

# 分析 RAG_PLAN_SYSTEM_PROMPT 的影响范围
gitnexus impact "RAG_PLAN_SYSTEM_PROMPT" -r context-osv6
```

---

## OpenClaw 代码审查相关 Skills

### 从 ClawHub 找到的相关技能

| 技能名 | 用途 | 来源 |
|--------|------|------|
| `audit-code` | 安全聚焦的代码审查 | awesome-openclaw-skills |
| `agent-skills-audit` | 多学科代码审计（安全+性能+UX+DX） | awesome-openclaw-skills |
| `arc-security-audit` | 全面安全审计 | awesome-openclaw-skills |
| `aegis-audit` | 深度行为安全审计 | awesome-openclaw-skills |
| `subagent-code-reviewer` | 派生子代理审查代码 | lobehub |
| `gitnexus-cli` | GitNexus CLI 集成 | lobehub |

### 安装方法

```bash
# 通过 LobeHub 安装
npx -y @lobehub/market-cli skills install audit-code --agent open-claw
npx -y @lobehub/market-cli skills install agent-skills-audit --agent open-claw
npx -y @lobehub/market-cli skills install subagent-code-reviewer --agent open-claw
```

**注意**: 需要注册 `lhm register` 或使用 `MARKET_CLIENT_ID`/`MARKET_CLIENT_SECRET`

---

## 深度代码审查方法论（基于 gitnexus + 手动分析）

### 1. 架构级审查（用 gitnexus）

```bash
# 查看模块依赖关系
gitnexus query "module dependency" -r context-osv6 -l 20

# 查看数据流
gitnexus query "data flow" -r context-osv6 -l 20

# 查看关键接口的实现者
gitnexus context "RetrievalDataPlane" -r context-osv6
```

### 2. 影响分析（改动前必做）

```bash
# 分析修改某处代码的影响范围
gitnexus impact "FunctionName" -r context-osv6
```

### 3. 手动深度审查清单

**A. 接口契约检查**
- [ ] 检查 trait 定义和实现是否一致
- [ ] 检查 error 处理是否完整
- [ ] 检查 async/await 边界

**B. 数据流检查**
- [ ] 跟踪关键数据从输入到输出的完整路径
- [ ] 检查中间转换是否丢失信息
- [ ] 检查并发安全性

**C. 架构一致性检查**
- [ ] 对比 PRD 和实际实现
- [ ] 检查模块边界是否清晰
- [ ] 检查是否有循环依赖

**D. 性能和安全检查**
- [ ] 检查是否有阻塞操作
- [ ] 检查资源泄漏
- [ ] 检查输入验证

---

## 下一步建议

### 立即可做
1. **安装 audit-code skill** — 获得结构化代码审查模板
2. **用 gitnexus 做影响分析** — 针对你关心的改动点

### 深度 Review 流程
1. 用 `gitnexus query` 找到相关代码区域
2. 用 `gitnexus context` 理解符号关系
3. 用 `gitnexus impact` 分析改动风险
4. 手动检查接口契约和数据流
5. 对比 PRD 验证架构一致性

---

*由 大虾 🦐 整理*