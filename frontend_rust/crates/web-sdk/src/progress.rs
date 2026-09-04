use crate::reducer::{ActivityEntry, TurnStatus};

/// 相邻且 title+detail 相同的进度条合并为一条（host 可能重发同一步）。
pub fn activities_for_display(entries: &[ActivityEntry]) -> Vec<ActivityEntry> {
    let mut out: Vec<ActivityEntry> = Vec::new();
    for entry in entries {
        if let Some(prev) = out.last() {
            if prev.title == entry.title && prev.detail == entry.detail {
                continue;
            }
        }
        out.push(entry.clone());
    }
    out
}

/// 终态默认折叠进度步骤；流式中保持展开。
pub fn progress_folded(status: &TurnStatus) -> bool {
    status.is_terminal()
}

pub fn progress_summary_label(status: &TurnStatus) -> &'static str {
    match status {
        TurnStatus::Done => "思考完成",
        TurnStatus::Cancelled => "已停止",
        TurnStatus::Error { .. } => "未完成",
        TurnStatus::Streaming => "进行中",
        TurnStatus::Idle => "",
    }
}
