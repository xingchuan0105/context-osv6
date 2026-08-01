//! S1 确定性工具语义测试（移植 `check_supervise.py` Part 1 的 12 项用例；
//! 手写迷你 grids，不依赖 /tmp 真实语料）。

use avrag_struct_supervision::{Grid, Row, SuperviseInput, session::Session};

fn row(line: usize, cells: &[&str]) -> Row {
    Row {
        line,
        cells: cells.iter().map(|c| c.to_string()).collect(),
    }
}

fn input(text: &str, grids: Vec<Grid>) -> SuperviseInput {
    SuperviseInput {
        doc_id: Some("test".into()),
        source_text: text.to_string(),
        grids,
    }
}

/// 三张表的迷你语料：t0 layout 表、t1 11 列脏表（Unnamed 假表头）、t2 与 t1 同签名。
fn fixture() -> SuperviseInput {
    let text = [
        "| 文档标题 |",
        "| --- |",
        "| 布局说明 |",
        "",
        "| Unnamed: 0 | Unnamed: 1 | 名称 | 金额 | 单位 | 备注 | 空列 | a8 | a9 | a10 | a11 |",
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
        "| 序号 | 阶段 | 品名 | 总价 | 元 | 无 | | 1 | 2 | 3 | 4 |",
        "| 1 | 概念 | 甲 | 100 | 元 | 无 | | 5 | 6 | 7 | 8 |",
        "| 2 | 验证 | 乙 | 200 | 元 | 无 | | 9 | 10 | 11 | 12 |",
        "",
        "| Unnamed: 0 | Unnamed: 1 | 名称 | 金额 | 单位 | 备注 | 空列 | a8 | a9 | a10 | a11 |",
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
        "| 3 | 发布 | 丙 | 300 | 元 | 无 | | 13 | 14 | 15 | 16 |",
    ]
    .join("\n");
    let hdr11 = [
        "Unnamed: 0",
        "Unnamed: 1",
        "名称",
        "金额",
        "单位",
        "备注",
        "空列",
        "a8",
        "a9",
        "a10",
        "a11",
    ];
    let d1 = [
        "序号", "阶段", "品名", "总价", "元", "无", "空列", "1", "2", "3", "4",
    ];
    let d2 = ["1", "概念", "甲", "100", "元", "无", "", "5", "6", "7", "8"];
    let d3 = [
        "2", "验证", "乙", "200", "元", "无", "", "9", "10", "11", "12",
    ];
    let d4 = [
        "3", "发布", "丙", "300", "元", "无", "", "13", "14", "15", "16",
    ];
    let grids = vec![
        Grid {
            start_line: 1,
            rows: vec![row(2, &["文档标题"]), row(4, &["布局说明"])],
            notes: vec![],
        },
        Grid {
            start_line: 6,
            rows: vec![row(6, &hdr11), row(8, &d1), row(9, &d2), row(10, &d3)],
            notes: vec![],
        },
        Grid {
            start_line: 12,
            rows: vec![row(12, &hdr11), row(14, &d4)],
            notes: vec![],
        },
    ];
    input(&text, grids)
}

#[test]
fn quarantine_blocks_later_annotate() {
    let mut s = Session::new(&fixture()).unwrap();
    s.t_quarantine(&serde_json::json!({"table_id": "t2", "reason": "回归测试"}));
    let annot = serde_json::json!([{
        "table_id": "t2", "table_kind": "detail", "confidence": "low"
    }]);
    let r = s.t_annotate(annot.as_array().unwrap());
    assert!(r.contains("终态不被"), "{r}");
    assert!(s.finals["t2"].excluded);
}

#[test]
fn exclude_directive_blocks_later_annotate() {
    let mut s = Session::new(&fixture()).unwrap();
    s.t_apply_directive(&serde_json::json!({
        "table_id": "t0", "directive": {"action": "exclude", "reason": "layout"}
    }));
    let annot = serde_json::json!([{
        "table_id": "t0", "table_kind": "layout", "confidence": "low"
    }]);
    let r = s.t_annotate(annot.as_array().unwrap());
    assert!(r.contains("终态不被"), "{r}");
    assert!(s.finals["t0"].excluded);
}

#[test]
fn rotate_header_rejects_zero_and_out_of_range() {
    let mut s = Session::new(&fixture()).unwrap();
    let r0 = s.t_apply_directive(&serde_json::json!({
        "table_id": "t1", "directive": {"action": "rotate_header", "header_row": 0}
    }));
    assert!(r0.contains("未通过校验"), "{r0}");
    let r999 = s.t_apply_directive(&serde_json::json!({
        "table_id": "t1", "directive": {"action": "rotate_header", "header_row": 999}
    }));
    assert!(r999.contains("未通过校验"), "{r999}");
}

#[test]
fn set_header_rejects_line_without_evidence() {
    let mut s = Session::new(&fixture()).unwrap();
    let r = s.t_apply_directive(&serde_json::json!({
        "table_id": "t1",
        "directive": {
            "action": "set_header",
            "headers": ["甲", "乙", "丙", "丁", "戊", "己", "庚", "辛", "壬", "癸", "子"],
            "evidence_source_line": 1,
        }
    }));
    assert!(r.contains("未通过校验"), "{r}");
}

#[test]
fn failing_table_rejects_high_confidence() {
    let mut s = Session::new(&fixture()).unwrap();
    // t1 有 header_suspicious 失败校验
    assert!(!s.reports["t1"].all_passed());
    let annot = serde_json::json!([{
        "table_id": "t1", "table_kind": "detail", "confidence": "high"
    }]);
    let r = s.t_annotate(annot.as_array().unwrap());
    assert!(r.contains("守卫"), "{r}");
    assert!(!s.finals.contains_key("t1"));
}

#[test]
fn run_check_read_only_guard_rejects_attach() {
    let s = Session::new(&fixture()).unwrap();
    let r = s.t_run_check(&serde_json::json!({"sql": "ATTACH '/etc/passwd'"}));
    assert!(r.contains("守卫"), "{r}");
}

#[test]
fn rotate_header_applies_and_passes_recheck() {
    let mut s = Session::new(&fixture()).unwrap();
    // t1：Unnamed 假表头 + 第一数据行像真表头 → rotate_header(header_row=1)
    let r = s.t_apply_directive(&serde_json::json!({
        "table_id": "t1",
        "directive": {"action": "rotate_header", "header_row": 1},
    }));
    assert!(r.contains("已通过"), "{r}");
    // rotate 后表头来自原第 1 数据行；header_suspicious（Unnamed）检查应通过
    assert_eq!(s.grids[1].header()[0], "序号");
    assert!(
        !s.reports["t1"]
            .checks_full
            .iter()
            .any(|c| c.name == "header_suspicious" && !c.passed)
    );
    // 复验：数据行数为 2（原 3 行数据 - 1 行提升为表头）
    assert_eq!(s.grids[1].n_rows(), 2);
}

#[test]
fn drop_columns_matching_guard_keeps_nonempty() {
    let mut s = Session::new(&fixture()).unwrap();
    // 守卫：pattern 命中的列若数据区非全空 → 拒丢（列被保留，指令仍成功）。
    // t1 第 7 列「空列」在 d1 有值（"空列"）→ 保留。
    let r = s.t_apply_directive(&serde_json::json!({
        "table_id": "t1",
        "directive": {
            "action": "rotate_header",
            "header_row": 1,
            "drop_columns_matching": "^空列$",
        },
    }));
    assert!(r.contains("已通过"), "{r}");
    assert!(
        s.grids[1].header().iter().any(|h| h == "空列"),
        "{:?}",
        s.grids[1].header()
    );
    assert!(
        s.grids[1].header().iter().any(|h| h == "品名"),
        "{:?}",
        s.grids[1].header()
    );

    // t2 第 7 列数据区全空 → 可丢。
    let mut s2 = Session::new(&fixture()).unwrap();
    let r2 = s2.t_apply_directive(&serde_json::json!({
        "table_id": "t2",
        "directive": {
            "action": "rotate_header",
            "header_row": 1,
            "drop_columns_matching": "^空列$",
        },
    }));
    assert!(r2.contains("已通过"), "{r2}");
    assert!(
        !s2.grids[2].header().iter().any(|h| h == "空列"),
        "{:?}",
        s2.grids[2].header()
    );
}

#[test]
fn merge_tables_requires_same_signature() {
    let mut s = Session::new(&fixture()).unwrap();
    // t1 + t2 同签名 → 成功；t0 + t1 签名不同 → 拒
    let r = s.t_apply_directive(&serde_json::json!({
        "table_id": "t1",
        "directive": {"action": "merge_tables", "table_ids": ["t2"]},
    }));
    assert!(r.contains("已通过"), "{r}");
    assert!(s.finals.contains_key("t2"));
    assert!(s.finals["t2"].excluded);
    assert!(!s.reports.contains_key("t2"));

    let mut s2 = Session::new(&fixture()).unwrap();
    let r2 = s2.t_apply_directive(&serde_json::json!({
        "table_id": "t0",
        "directive": {"action": "merge_tables", "table_ids": ["t1"]},
    }));
    assert!(r2.contains("未通过校验"), "{r2}");
}

#[test]
fn reparse_region_rebuilds_table() {
    let mut s = Session::new(&fixture()).unwrap();
    // t0 的源行 1..=4 区间内有 2 行管道行 → 重建成功
    let r = s.t_apply_directive(&serde_json::json!({
        "table_id": "t0",
        "directive": {"action": "reparse_region", "start_line": 1, "end_line": 4},
    }));
    assert!(r.contains("已通过"), "{r}");
    assert!(s.grids[0].n_rows() >= 1);
}

#[test]
fn unfinished_excludes_finalized() {
    let mut s = Session::new(&fixture()).unwrap();
    s.t_quarantine(&serde_json::json!({"table_id": "t2", "reason": "x"}));
    let un = s.unfinished();
    assert!(!un.contains(&"t2".to_string()));
    assert!(un.contains(&"t0".to_string()));
    assert!(un.contains(&"t1".to_string()));
}

#[test]
fn supervise_input_roundtrips_json() {
    let input = fixture();
    let bytes = serde_json::to_vec(&input).unwrap();
    let back = SuperviseInput::from_json_bytes(&bytes).unwrap();
    assert_eq!(back.grids.len(), 3);
    assert_eq!(back.grids[1].header()[0], "Unnamed: 0");
}

#[test]
fn run_check_executes_select() {
    let s = Session::new(&fixture()).unwrap();
    let r = s.t_run_check(&serde_json::json!({"sql": "SELECT COUNT(*) FROM t1"}));
    assert!(r.contains("run_check 完成"), "{r}");
    assert!(r.contains("3"), "{r}");
}
