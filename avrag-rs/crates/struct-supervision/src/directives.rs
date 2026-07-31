//! 修复指令：schema 校验 + 确定性守卫 + 应用（对齐 `supervise._apply`）。
//! 指令是 LLM 的唯一干预通道；LLM 永不提供单元格值（prompt 侧禁区，本模块只执行）。

use crate::grid::{Row, header_sig};
use crate::session::Session;

/// 应用指令到 `session.grids[tid]`；成功返回 Ok(())，守卫不过返回 Err(拒绝原因)。
/// 副作用：rotate/set/reparse 改 grids 行；merge/exclude 改 finals 与 reports。
pub fn apply(s: &mut Session, tid: &str, d: &serde_json::Value) -> Result<(), String> {
    let action = d.get("action").and_then(|v| v.as_str()).unwrap_or("");
    let idx: usize = tid[1..]
        .parse()
        .map_err(|_| format!("{tid} 非 t{{idx}} 形态"))?;
    if idx >= s.grids.len() {
        return Err(format!("表 {tid} 不存在"));
    }
    match action {
        "rotate_header" => rotate_header(s, idx, d),
        "set_header" => set_header(s, idx, d),
        "merge_tables" => merge_tables(s, tid, idx, d),
        "reparse_region" => reparse_region(s, idx, d),
        "exclude" => {
            s.finals.insert(
                tid.to_string(),
                crate::session::FinalState {
                    table_id: tid.to_string(),
                    excluded: true,
                    reason: d.get("reason").and_then(|v| v.as_str()).map(str::to_string),
                    ..Default::default()
                },
            );
            Ok(())
        }
        other => Err(format!("未知 action:{other}")),
    }
}

/// rotate_header：header_row 提升为表头（带 drop_columns_matching 守卫——非全空列拒丢）。
fn rotate_header(s: &mut Session, idx: usize, d: &serde_json::Value) -> Result<(), String> {
    let hr = d.get("header_row").and_then(|v| v.as_i64()).unwrap_or(1);
    if hr < 1 || hr as usize > s.grids[idx].n_rows() {
        return Err(format!("header_row={hr} 超出数据行范围(1..{})", s.grids[idx].n_rows()));
    }
    let pat = d.get("drop_columns_matching").and_then(|v| v.as_str());
    let g = &mut s.grids[idx];
    let hdr = g.header().to_vec();
    let mut keep: Vec<usize> = (0..hdr.len()).collect();
    if let Some(pat) = pat {
        let re =
            regex::Regex::new(pat).map_err(|e| format!("drop_columns_matching 正则无效:{e}"))?;
        keep = (0..hdr.len())
            .filter(|&i| {
                !(re.is_match(&hdr[i])
                    && g.data()
                        .iter()
                        .all(|r| r.cells.get(i).map(|c| c.is_empty()).unwrap_or(true)))
            })
            .collect();
        if keep.is_empty() {
            return Err("守卫:drop 后无剩余列".into());
        }
    }
    let body = g.rows.clone();
    let hr_usize = hr as usize;
    let new_header = Row {
        line: body[hr_usize].line,
        cells: keep
            .iter()
            .map(|&k| body[hr_usize].cells.get(k).cloned().unwrap_or_default())
            .collect(),
    };
    g.rows = vec![new_header];
    g.rows.extend(
        body.iter()
            .skip(hr_usize + 1)
            .map(|r| Row {
                line: r.line,
                cells: keep
                    .iter()
                    .map(|&k| r.cells.get(k).cloned().unwrap_or_default())
                    .collect(),
            }),
    );
    g.notes.push(format!("directive:rotate_header(header_row={hr})"));
    Ok(())
}

/// set_header：以证据行文字替换表头（守卫：所有表头文字必须出现在证据行）。
fn set_header(s: &mut Session, idx: usize, d: &serde_json::Value) -> Result<(), String> {
    let headers: Vec<String> = d
        .get("headers")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let ev = d
        .get("evidence_source_line")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let g = &mut s.grids[idx];
    if headers.len() != g.header().len() {
        return Err(format!(
            "headers 数({}) != 现列数({})",
            headers.len(),
            g.header().len()
        ));
    }
    let line = if (1..=s.lines.len() as i64).contains(&ev) {
        s.lines[ev as usize - 1].clone()
    } else {
        String::new()
    };
    let missing: Vec<&String> = headers.iter().filter(|h| !line.contains(h.as_str())).collect();
    if !missing.is_empty() {
        return Err(format!("守卫:{missing:?} 未出现在证据行 L{ev}"));
    }
    g.rows[0].cells = headers;
    g.notes.push(format!("directive:set_header(evidence=L{ev})"));
    Ok(())
}

/// merge_tables：同表头签名表合并（守卫：签名必须一致；目标外的表标记 excluded）。
fn merge_tables(s: &mut Session, tid: &str, idx: usize, d: &serde_json::Value) -> Result<(), String> {
    let mut ids: Vec<String> = d
        .get("table_ids")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    if !ids.iter().any(|i| i == tid) {
        ids.insert(0, tid.to_string());
    }
    if ids.len() < 2 {
        return Err("merge_tables 需要 ≥2 个 table_id".into());
    }
    let sig = header_sig(&s.grids[idx].header());
    let mut tgt_rows = s.grids[idx].rows.clone();
    let mut notes = s.grids[idx].notes.clone();
    for other in &ids[1..] {
        if !s.reports.contains_key(other) {
            return Err(format!("{other} 不存在"));
        }
        let oi: usize = other[1..].parse().unwrap_or(0);
        if oi >= s.grids.len() {
            return Err(format!("{other} 不存在"));
        }
        let og = &s.grids[oi];
        if header_sig(&og.header()) != sig {
            return Err(format!("守卫:{other} 表头签名不一致"));
        }
        tgt_rows.extend(og.rows[1..].iter().cloned());
        s.finals.insert(
            other.clone(),
            crate::session::FinalState {
                table_id: other.clone(),
                excluded: true,
                reason: Some(format!("merged_into {tid}")),
                ..Default::default()
            },
        );
        s.reports.remove(other);
    }
    s.grids[idx].rows = tgt_rows;
    notes.push(format!("directive:merge_tables({:?})", &ids[1..]));
    s.grids[idx].notes = notes;
    Ok(())
}

/// reparse_region：源行区间重新解析管道行作为表数据（守卫：≥2 行、非分隔行）。
fn reparse_region(s: &mut Session, idx: usize, d: &serde_json::Value) -> Result<(), String> {
    let a = d.get("start_line").and_then(|v| v.as_i64()).unwrap_or(0);
    let b = d.get("end_line").and_then(|v| v.as_i64()).unwrap_or(0);
    let n = s.lines.len() as i64;
    if !(1 <= a && a < b && b <= n) {
        return Err(format!("行区间 L{a}–L{b} 无效(全文 1..{n})"));
    }
    let sep_re = regex::Regex::new(r"^[-:\s]*$").unwrap();
    let mut rows = Vec::new();
    for i in (a as usize - 1)..(b as usize) {
        let ln = s.lines[i].trim();
        if !ln.starts_with('|') {
            continue;
        }
        // 手动切管道符（Rust regex 无 look-behind）：跳过 `\|` 转义。
        let mut cells = Vec::new();
        let mut cur = String::new();
        let mut prev_esc = false;
        for ch in ln.chars() {
            if ch == '|' && !prev_esc {
                let c = cur.trim().to_string();
                if !c.is_empty() {
                    cells.push(c);
                }
                cur.clear();
            } else {
                cur.push(ch);
            }
            prev_esc = ch == '\\';
        }
        let tail = cur.trim().to_string();
        if !tail.is_empty() {
            cells.push(tail);
        }
        if !cells.is_empty() && !cells.iter().all(|c| sep_re.is_match(c)) {
            rows.push(Row { line: i + 1, cells });
        }
    }
    if rows.len() < 2 {
        return Err(format!("区域 L{a}–L{b} 未解析出 ≥2 行管道行"));
    }
    let g = &mut s.grids[idx];
    g.rows = rows;
    g.start_line = a as usize;
    g.notes.push(format!("directive:reparse_region(L{a}-L{b})"));
    Ok(())
}
