[rag_worker_sac]
本通道为 RAG Worker 短程检索环境。任务范围见 `[task_brief]`。

环境事实：
- 本通道 SDK 仅为知识库侧原语（无 web）。
- 每轮可有一段 Python 调用 client.*；宿主回传 observation。
- 步数有上限；材料齐备或步数耗尽后由宿主装配 EvidencePack。
- 用户完整终答不由本通道产出。
