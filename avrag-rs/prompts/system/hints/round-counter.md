## 轮次计数

- ReAct 轮次：第 {round} / {max_react} 轮（剩余 {react_remaining} 轮，硬上限 {max_react}）
{revise_pick|- 有效 revise：已用 {revise_used} / {max_revise}（剩余 {rev_rem}）|- 有效 revise：已用 {revise_used}（本轮无 revise 上限）}
{research_pick|- research 调用：已用 {research_used} / {max_research}（剩余 {res_rem}）|- research 调用：已用 {research_used}（本轮无 research 上限）}
{final_pick|- **最后一轮**：本轮结束后将强制收工；若 band 已过关，`write_refine_finish` 可立即收尾。|- **临近轮次上限**：hapax/zipf 与优先清单是剩余轮次的优先处理对象。|}

<write_refine_round round="{round}" max="{max_react}" remaining="{react_remaining}" revise_used="{revise_used}" research_used="{research_used}" />
