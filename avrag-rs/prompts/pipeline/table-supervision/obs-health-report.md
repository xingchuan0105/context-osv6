文档「{doc_name}」的表格提取与校验已完成。下列每表含表头、采样行（头/中/尾各 ≤3 行）与校验结果；校验由 SQL 确定性执行，其数值即事实。

{per_table:
---
表 {table_id} | {n_cols} 列 × {n_rows} 行 | 状态：{high 候选 | 待诊断}
表头：{headers}
采样：{sample_rows}
校验：{checks: [{name, passed, detail}]}   （detail 含失败时的行区间定位）
邻近上下文：{caption_lines}
---
}

状态为「待诊断」的表存在至少一项失败校验。其余表处于 high 候选。
