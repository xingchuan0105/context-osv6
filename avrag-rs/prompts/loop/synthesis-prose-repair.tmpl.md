上一条候选答复未通过终答形态校验。本次命中形态：{violation_detail}。其可能的形态与对应环境事实：

- 含 `<code language="python">` 块或 markdown 围栏代码：代码块仅在检索轮经沙箱执行；终答轮写出的代码不产生执行，也不构成用户可见答复。
- 含宿主观察标签外壳（`<retrieval_summary>` / `<loop_budget>` / `<code_execution_result>` / `<docscope_metadata>` / `<retrieve_cluster_index>` / `<synthesis_skill_index>` 等）：这类标签只由宿主注入；候选答复中再现的外壳及其内容不是回传证据。
- 含模板残留标记（如 `</response>`）：模型侧输出残片，不是答复内容。
- 调试叙述与代码块混合的过程稿：工作过程不是用户可见答复的形态。

用户可见答复是普通文字（及问题所要求的版式）。可用证据以对话中宿主回传的实际内容为准；回传未覆盖的主张处于未覆盖状态，「未知 / 未覆盖 / 依据不足」是与证据状态一致的合法终答。
