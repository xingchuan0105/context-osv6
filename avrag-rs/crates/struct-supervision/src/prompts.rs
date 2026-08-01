//! prompts 落点（repo law：LLM 指令正文不进 Rust 代码）：
//! `prompts/pipeline/table-supervision/` 的 system prompt 与全部观察模板
//! 经 include_str! 加载；本模块只做占位符替换与数据拼装。

use std::collections::BTreeMap;

pub const SYSTEM_PROMPT: &str =
    include_str!("../../../prompts/pipeline/table-supervision/supervision.system.v1.md");

pub fn system_prompt() -> &'static str {
    SYSTEM_PROMPT
}

/// 观察模板渲染上下文：`{key}` 字面替换、`{block:\n...\n}` 行循环、
/// `{pick|备选0|备选1}` 按 `picks` 索引选择（备选内仅支持 `{key}`）。
#[derive(Default, Clone)]
pub struct ObsCtx {
    keys: BTreeMap<String, String>,
    blocks: BTreeMap<String, Vec<BTreeMap<String, String>>>,
    picks: BTreeMap<String, usize>,
}

impl ObsCtx {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn key(mut self, k: &str, v: impl Into<String>) -> Self {
        self.keys.insert(k.to_string(), v.into());
        self
    }

    pub fn block(mut self, k: &str, items: Vec<BTreeMap<String, String>>) -> Self {
        self.blocks.insert(k.to_string(), items);
        self
    }

    pub fn pick(mut self, k: &str, idx: usize) -> Self {
        self.picks.insert(k.to_string(), idx);
        self
    }
}

/// 渲染观察模板 `name`（对应 `prompts/pipeline/table-supervision/obs-*.md`）。
pub fn obs(name: &str, ctx: ObsCtx) -> String {
    let template = match name {
        "annotate" => {
            include_str!("../../../prompts/pipeline/table-supervision/obs-annotate.md")
        }
        "check-error" => {
            include_str!("../../../prompts/pipeline/table-supervision/obs-check-error.md")
        }
        "check-guard" => {
            include_str!("../../../prompts/pipeline/table-supervision/obs-check-guard.md")
        }
        "check-result" => {
            include_str!("../../../prompts/pipeline/table-supervision/obs-check-result.md")
        }
        "directive-applied" => {
            include_str!("../../../prompts/pipeline/table-supervision/obs-directive-applied.md")
        }
        "directive-missing" => {
            include_str!("../../../prompts/pipeline/table-supervision/obs-directive-missing.md")
        }
        "directive-rejected" => {
            include_str!("../../../prompts/pipeline/table-supervision/obs-directive-rejected.md")
        }
        "done" => include_str!("../../../prompts/pipeline/table-supervision/obs-done.md"),
        "health-report" => {
            include_str!("../../../prompts/pipeline/table-supervision/obs-health-report.md")
        }
        "no-tool" => include_str!("../../../prompts/pipeline/table-supervision/obs-no-tool.md"),
        "progress" => {
            include_str!("../../../prompts/pipeline/table-supervision/obs-progress.md")
        }
        "quarantine" => {
            include_str!("../../../prompts/pipeline/table-supervision/obs-quarantine.md")
        }
        "slice" => include_str!("../../../prompts/pipeline/table-supervision/obs-slice.md"),
        "table-missing" => {
            include_str!("../../../prompts/pipeline/table-supervision/obs-table-missing.md")
        }
        "unknown-tool" => {
            include_str!("../../../prompts/pipeline/table-supervision/obs-unknown-tool.md")
        }
        other => panic!("unknown supervision observation template: {other}"),
    };
    render(template, &ctx)
}

/// `{key}` 字面替换（pick 备选用；备选内只允许键，不做块/选支嵌套解析）。
fn render_keys_only(template: &str, ctx: &ObsCtx) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let tail = &rest[start + 1..];
        match tail.find('}') {
            Some(close) => {
                if let Some(v) = ctx.keys.get(&tail[..close]) {
                    out.push_str(v);
                }
                rest = &tail[close + 1..];
            }
            None => {
                out.push('{');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

/// 跳过直到与 `}` 配对的花括号（pick 备选内允许 `{key}` 的场景）。
fn scan_close(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

fn render(template: &str, ctx: &ObsCtx) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let tail = &rest[start + 1..];
        let Some(seg_end) = tail.find('}') else {
            out.push('{');
            rest = tail;
            continue;
        };
        let seg = &tail[..seg_end];
        if let Some(colon) = seg.find(':') {
            // 块：{name:\n...\n}，行内占位符经递归展开
            let name = &seg[..colon];
            let content = &tail[colon + 1..];
            if let Some(close) = content.find("\n}") {
                let mut inner = &content[..close];
                if inner.starts_with('\n') {
                    inner = &inner[1..];
                }
                let items = ctx.blocks.get(name).map(Vec::as_slice).unwrap_or(&[]);
                let rendered: Vec<String> = items
                    .iter()
                    .map(|row| {
                        let mut sub = ctx.clone();
                        sub.keys.extend(row.iter().map(|(k, v)| (k.clone(), v.clone())));
                        render(inner, &sub)
                    })
                    .collect();
                out.push_str(&rendered.join("\n"));
                rest = &content[close + 2..];
                continue;
            }
        }
        if let Some(pipe) = seg.find('|') {
            // pick：{name|备选0|备选1}
            let name = &seg[..pipe];
            let content = &tail[pipe + 1..];
            if let Some(close) = scan_close(content) {
                let alts = content[..close].split('|').collect::<Vec<_>>();
                let idx = ctx.picks.get(name).copied().unwrap_or(0);
                if let Some(alt) = alts.get(idx.min(alts.len().saturating_sub(1))) {
                    out.push_str(&render_keys_only(alt, ctx));
                }
                rest = &content[close + 1..];
                continue;
            }
        }
        if let Some(v) = ctx.keys.get(seg) {
            out.push_str(v);
        }
        rest = &tail[seg_end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> ObsCtx {
        ObsCtx::new()
    }

    #[test]
    fn renders_keys_blocks_and_picks() {
        let mut rows = Vec::new();
        for (id, n) in [("t0", "3"), ("t1", "5")] {
            let mut r = BTreeMap::new();
            r.insert("table_id".into(), id.into());
            r.insert("n_cols".into(), "2".into());
            r.insert("n_rows".into(), n.into());
            r.insert("status".into(), "high 候选".into());
            r.insert("headers".into(), "[\"h\"]".into());
            r.insert("sample_rows".into(), "  采样: a".into());
            r.insert("check_lines".into(), "校验:全部通过".into());
            r.insert("notes_line".into(), String::new());
            rows.push(r);
        }
        let out = obs(
            "health-report",
            ctx()
                .key("doc_name", "x.md")
                .key("n_tables", "2")
                .block("per_table", rows),
        );
        assert!(out.contains("文档「x.md」的表格提取与校验已完成。共 2 张表。"));
        assert!(out.contains("表 t0 | 2 列 × 3 行 | 状态:high 候选"));
        assert!(out.contains("表 t1 | 2 列 × 5 行 | 状态:high 候选"));
        assert!(out.contains("状态为「待诊断」的表存在至少一项失败校验。"));
    }

    #[test]
    fn pick_selects_alternative_by_index() {
        let out = obs(
            "annotate",
            ctx()
                .pick("case", 2)
                .key("table_id", "t0"),
        );
        assert!(out.contains("t0: 校验未全部通过,confidence=high 未生效(守卫)"));
        assert!(!out.contains("不存在"));
        let out = obs("annotate", ctx().pick("case", 4));
        assert_eq!(out.trim(), "未提供 tables");
    }

    #[test]
    fn pick_alternative_renders_keys() {
        let out = obs(
            "directive-applied",
            ctx()
                .pick("rebuild_ok", 0)
                .key("action", "split")
                .key("table_id", "t0")
                .key("n_cols", "2")
                .key("n_rows", "9")
                .key("status", "high")
                .key("headers", "[\"a\", \"b\"]")
                .key("checks", "全部通过"),
        );
        assert!(out.contains("指令 split 已通过 schema 校验与确定性守卫，应用于表 t0"));
        assert!(out.contains("新健康报告:2 列 × 9 行，状态:high"));
        let out = obs(
            "directive-applied",
            ctx()
                .pick("rebuild_ok", 1)
                .key("table_id", "t0")
                .key("rebuild_error", "boom"),
        );
        assert!(out.contains("指令已应用,但内存库重建失败:boom"));
    }

    #[test]
    fn slice_kind_pick_switches_wording() {
        let out = obs(
            "slice",
            ctx()
                .pick("slice_kind", 0)
                .key("table_id", "t0")
                .key("from", "1")
                .key("to", "3")
                .key("total", "10")
                .key("slice", "L1: a"),
        );
        assert!(out.contains("的原文切片如下"));
        let out = obs(
            "slice",
            ctx()
                .pick("slice_kind", 1)
                .key("table_id", "t0")
                .key("from", "1")
                .key("to", "3")
                .key("total", "10")
                .key("slice", "row 1: a"),
        );
        assert!(out.contains("的解析切片如下"));
    }
}
