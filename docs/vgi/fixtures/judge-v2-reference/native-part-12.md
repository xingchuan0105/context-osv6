
【重要】本题是多轮对话/记忆题，答案可合法依赖对话历史（prior turns）而非检索 context。因此：
- faithfulness.verdict 必须返回 "not_applicable"（score 填 1.0 占位，不评分）；
- 不得因缺少检索 context 而编造 unsupported_claims；
- context_sufficiency.verdict 必须返回 "unknown"；
- answer_correctness / answer_relevancy / refusal 正常评分。
