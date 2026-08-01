文档「{doc_name}」的表格提取与校验已完成。共 {n_tables} 张表。校验由 SQL 确定性执行,其数值即事实。

{per_table:
---
表 {table_id} | {n_cols} 列 × {n_rows} 行 | 状态:{status}
表头:{headers}
采样:{sample_rows}
{check_lines}
{notes_line}
---
}
状态为「待诊断」的表存在至少一项失败校验。全部表给出终态(high/low/quarantine)并完成语义标注后,done 工具可结束监督。
