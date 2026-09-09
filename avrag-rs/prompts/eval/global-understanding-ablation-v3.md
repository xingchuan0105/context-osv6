# 文档消融独立评分 v3

本评测沿用原生E2E的正确性、拒答预期和逐题rubric_notes，补充固定请求点覆盖度及冻结正文支持度。题目及参考答案只出现在评测输入中，不是产品规则。输入中的model_answer是唯一待评分答案；reference_answer和authoritative_sources都不是该答案的内容。authoritative_sources是对应文档的冻结原文片段及原始文件元数据，不含生成的summary/profile/TOC。文档作者的观点只代表文档观点，不是外部世界事实。

observed_material仅记录该答案运行时实际收到的材料，可能包含生成内容，用于核对“当前检索未返回某信息”等局部观察。authoritative_sources用于核对事实；完整原文中存在某信息，不表示该答案当时看到了它。答案仅说“当前片段未见”，与声称“整篇文档不存在”具有不同含义。

正确性correctness（0至1）沿用原生E2E语义：与参考答案语义等价为高分，改写等价；事实、主体、数量或关键限定错误导致扣分；缺失关键答案点体现部分正确。rubric_notes的条件性满分路径仍有效：对局部材料缺口的诚实说明可满足指定题的正确性规则，但没有提供的事实请求点仍不算覆盖。只有参考和正文冲突时才列出冲突，证据不足不等于事实错误。

expected_should_answer=false表示题目前提陷阱或应拒答题。实质性纠正错误前提、说明材料没有所问成文策略，可以附带明确区分的相关背景，不要求固定拒答措辞；声明后仍把错误前提当成既定事实或编造具体内容属于未正确拒答。正确拒答时correctness为1，错误接受前提时给低分。expected_should_answer=true时，完整答案或rubric_notes允许的缺口说明按其规则判定。refusal输出实质是否拒答及是否符合该题预期。

points逐一对应输入required_points的id，不增减。每点包含addressed、answer_quote、factual_status。addressed表示答案真正提供了该点内容，即使内容有误；仅说“不知道/未找到”时该事实点addressed=false。答案原句必须出自model_answer，不能从参考答案或正文中抄取。未回答点answer_quote为空、factual_status=missing。已回答点factual_status为correct/incorrect/unverified。完整性由脚本计算addressed=true的点数/总点数，正确与完整分别报告；不能把证据中的事实当作答案已回答的事实。

claims覆盖答案的实质性事实和有实质影响的推断；相同事实只计一次，可合并紧密相关且支持情况相同的短句。每个answer_quote都必须是model_answer的原文片段。status为supported、unsupported或unverified。supported附source_id及该source的原文evidence_quote；无支持或与正文矛盾为unsupported，并解释理由；证据不能判定为unverified。明确标注且有原文依据的推断可supported；“当前回传未见”的观察用observed_material中的O编号核对，不能用另一侧或完整原文强行反驳。原始文件扩展名可以证明文件格式。生成材料本身不能证明外部事实。

supported的evidence_quote必须是所列source_id对应text中的连续原句；answer_quote也必须是答案连续原句。JSON字符串中保留真实标点，换行可以使用JSON转义。正文支持度由脚本从claims计算；有unverified时保留未决标记，不制造满分。答案里没有实质性事实时claims可为空。

输出结构为单个JSON对象：
{"correctness":{"score":0.0,"rationale":""},"refusal":{"is_refusal":false,"correct_for_expectation":true,"rationale":""},"points":[{"id":"p1","addressed":true,"answer_quote":"答案原句","factual_status":"correct"}],"claims":[{"answer_quote":"答案原句","status":"supported","source_id":"S1","evidence_quote":"该来源原句","rationale":""}],"issues":[]}

条件性满分例外的最低已答对点数由missing_full_credit_min_correct_points给出。null表示没有该例外。门槛为1时，至少一个请求点确实正确回答才满足；两项都未知不满足。expected_should_answer=true且没有任何正确请求点、也不满足例外时correctness为0；遗漏点且不满足例外时不能为1。refusal.correct_for_expectation等于(is_refusal != expected_should_answer)；指定rubric允许的分槽缺口说明属于is_refusal=false，不能将全题未回答套用成部分完成。

引用可以用省略号连接多个原文片段；每个片段均来自同一答案或同一来源，按原顺序排列。代码只忽略空白、Markdown强调/代码/表格标记和引用ID，不忽略字词、数字、比较符号。原文中的实际条件和数量不能被格式规范化改变。
