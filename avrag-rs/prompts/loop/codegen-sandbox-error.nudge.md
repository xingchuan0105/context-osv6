[sandbox_error]
本轮代码执行失败（连续沙箱失败 {n_fail}/{n_max}；达到上限后 retrieve 结束并进入 synthesis）。stderr 见上方块。

环境事实：
- 调用形式为 client.方法名(...)；可用方法：dense、lexical、grep、struct_catalog、struct_query、calculator、web、fetch、doc_profile、doc_summary、history、user_profile、user_context、save、load。
- 无 top_k 参数；无 client.graph、graph_search、read_lines、dense_search、hybrid_search、client.rag 等旧别名或未挂载入口。
- 检索与计算结果需出现在 stdout（print）或经 client.* 回传；仅赋值不输出时观察面可为空。
- 本轮若在异常前已有 client.* Ok 回传，对应证据与 alias 仍保留在上下文中。
- 常见失败形态：AttributeError（方法名不存在）、TypeError/参数形状不符、未捕获 Traceback、import 沙箱未提供的模块。
[/sandbox_error]
