# 约定建议稿骨架（subtex.init / subtex.convention_draft）

合并说明（本段不写入 `AGENTS.md`）：工具返回的目录事实已填入下方骨架。`AGENTS.md` 中标记之外的内容保持原样、零覆盖；Subtex 管理段落只存在于 `<!-- subtex:begin -->` 与 `<!-- subtex:end -->` 标记之间；若标记已存在，仅替换标记之间的内容。Agent 依据目录事实填充 `{…}` 占位符后写出或合并本段。

---

<!-- subtex:begin -->

## Subtex 目录约定

**根目录**：{root_name}

**目录现状**（来自本轮扫描）：
{directory_facts}

**成品与原料位置**：
{output_locations}

**禁写区**（这些位置不由 Agent 放置新文件）：
{no_write_zones}

**检索顺序**（未就绪的层级如实跳过）：
1. 文件名 / grep 直接定位
2. `subtex.outline` 全局大纲定位
3. `subtex.search` 混合检索（返回带 `matched_via` 与出处）

本段由 Subtex 维护；标记之外的内容不属于管理范围。

<!-- subtex:end -->
