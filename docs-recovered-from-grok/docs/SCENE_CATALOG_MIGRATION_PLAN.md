# Scene Catalog Migration Plan

## 目标
将 tool skill 的渐进式披露从"工具说明书模式"重构为"场景目录 + 两轮披露模式"。

## 架构变更

### 1. 新增 Skill: `scene-catalog`
- `prompts/skills/scene-catalog/SKILL.md` — Index 层：场景列表 + 触发信号 + Round 1 输出格式
- `prompts/skills/scene-catalog/reference/rag-scenes.md` — R1-R6 Load 层配方
- `prompts/skills/scene-catalog/reference/search-scenes.md` — W1-W5 Load 层配方
- `prompts/skills/scene-catalog/reference/chat-scenes.md` — C1-C3 Load 层配方
- `prompts/skills/scene-catalog/reference/output-schema.md` — Round 1 输出 schema

### 2. 删除旧 Skills
- `prompts/skills/retrieval-planner/` → 删除（功能合并到 scene-catalog）
- `prompts/skills/chat-plan/` → 删除（功能合并到 scene-catalog）
- `prompts/skills/web-search-coverage-eval/` → 保留（Search Evaluate phase 仍需要）

### 3. 修改 Prompt Builder
- `crates/app/src/agents/strategy/prompts.rs`:
  - 删除 `build_plan_system_prompt()`
  - 新增 `build_scene_selection_prompt(strategy: &str)` — Round 1
  - 新增 `build_tool_call_prompt(scenes: &[&str], tools: &[&str])` — Round 2
  - 保留 `build_answer_system_prompt()` 不变

### 4. 修改策略执行流程（3 个 agent）
- `crates/app/src/agents/strategy/chat.rs` — `step_plan` 改为两轮
- `crates/app/src/agents/strategy/rag.rs` — `step_plan` 改为两轮
- `crates/app/src/agents/strategy/search.rs` — `step_decompose` 改为两轮

### 5. 修改 DisclosureUnit
- `crates/app/src/agents/progressive/disclosure_unit.rs`:
  - `DisclosureTier::Index` → 场景目录（场景标题 + 触发信号）
  - `DisclosureTier::Load` → 场景配方（工具组合 + 权重 + gotchas）
  - `DisclosureTier::Runtime` → 工具 schema + examples

### 6. 更新测试
- `progressive::tests` — 更新 disclosure 测试
- `capability::registry::tests` — 更新 tool catalog 测试
- `strategy::tests` — 更新 prompt builder 测试

## 执行顺序

### Phase 1: 文件准备
1. [x] 检查旧 skill 引用（retrieval-planner, chat-plan）
2. [x] 创建 scene-catalog skill 文件
3. [x] 删除旧 skill 目录

### Phase 2: Prompt Builder 重构
4. [x] 重写 `prompts.rs`：新增 build_scene_selection_prompt + build_tool_call_prompt
5. [x] 保留 build_plan_system_prompt 作为 legacy 兼容层

### Phase 3: 策略适配
6. [x] 修改 `chat.rs` step_plan（两轮）
7. [x] 修改 `rag.rs` step_plan（两轮）
8. [x] 修改 `search.rs` step_decompose（两轮）

### Phase 4: 测试与验证
9. [x] 更新 progressive 测试（替换 retrieval-planner/chat-plan 为 scene-catalog）
10. [x] 更新 registry 测试
11. [x] 更新 guardrails 测试（prompt leak 检测文本）
12. [x] 更新 token_budget 引用
13. [x] 修复 code-gen-query depends 依赖
14. [x] 运行 `cargo test -p app --lib` — **439 passed**
15. [x] 运行 `cargo test -p avrag-guardrails --lib` — **44 passed**

### Phase 5: 清理 Legacy 兼容层
16. [x] 删除 `build_plan_system_prompt()` 函数
17. [x] 删除 `PLANNER_SKILL_ID` 常量（chat/rag/search）
18. [x] 删除所有 legacy 测试
19. [x] 运行 `cargo test -p app --lib` — **433 passed**（-6 legacy tests）

## 验证标准
- [ ] `cargo test -p app --lib` 全部通过
- [ ] `cargo test -p app --lib -- progressive::tests` 通过
- [ ] `cargo test -p app --lib -- capability::registry::tests` 通过
- [ ] E2E RAG 测试通过
- [ ] E2E Search 测试通过
- [ ] E2E Chat 测试通过
