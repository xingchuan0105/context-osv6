
【重要】上方 context 是**内建工具的真实输出**（如 weather_query / calculator / doc_profile），是本题答案的权威依据：
- faithfulness 按答案是否被这些工具输出支持来评分（不是 not_applicable）；
- answer_correctness 可以也应该验证——答案与工具输出一致即高分；
- 仅当确实没有任何工具输出时才考虑 not_applicable。
