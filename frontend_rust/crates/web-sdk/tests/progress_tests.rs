use web_sdk::{
    ActivityEntry, TurnStatus, activities_for_display, progress_folded, progress_summary_label,
};

fn entry(title: &str, detail: Option<&str>) -> ActivityEntry {
    ActivityEntry {
        phase: "compose".to_string(),
        title: title.to_string(),
        detail: detail.map(str::to_string),
    }
}

#[test]
fn live_is_not_folded_terminal_is() {
    assert!(!progress_folded(&TurnStatus::Streaming));
    assert!(!progress_folded(&TurnStatus::Idle));
    assert!(progress_folded(&TurnStatus::Done));
    assert!(progress_folded(&TurnStatus::Cancelled));
    assert!(progress_folded(&TurnStatus::Error {
        code: "x".into(),
        message: "y".into(),
    }));
}

#[test]
fn summary_labels_match_terminal_state() {
    assert_eq!(progress_summary_label(&TurnStatus::Done), "思考完成");
    assert_eq!(progress_summary_label(&TurnStatus::Cancelled), "已停止");
    assert_eq!(
        progress_summary_label(&TurnStatus::Error {
            code: "x".into(),
            message: "y".into(),
        }),
        "未完成"
    );
}

#[test]
fn consecutive_duplicate_steps_collapse() {
    let entries = [
        entry("检索", Some("q1")),
        entry("检索", Some("q1")),
        entry("检索", Some("q2")),
        entry("撰写", None),
        entry("撰写", None),
    ];
    let shown = activities_for_display(&entries);
    assert_eq!(shown.len(), 3);
    assert_eq!(shown[0].detail.as_deref(), Some("q1"));
    assert_eq!(shown[1].detail.as_deref(), Some("q2"));
    assert_eq!(shown[2].title, "撰写");
}
