use std::ops::Range;

/// Markers bounding the Subtex-managed section inside a convention file
/// (`AGENTS.md`). Everything outside the markers is user-owned and never
/// touched by Subtex flows.
pub const MANAGED_SECTION_BEGIN: &str = "<!-- subtex:begin -->";
pub const MANAGED_SECTION_END: &str = "<!-- subtex:end -->";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedSectionSpan {
    /// Byte range covering both markers, inclusive.
    pub full: Range<usize>,
    /// Byte range between the markers, markers excluded.
    pub body: Range<usize>,
}

/// Locate the managed section. An unclosed begin marker counts as absent —
/// malformed markers are left to the user, and upsert appends a fresh section.
pub fn find_managed_section(content: &str) -> Option<ManagedSectionSpan> {
    let begin = content.find(MANAGED_SECTION_BEGIN)?;
    let body_start = begin + MANAGED_SECTION_BEGIN.len();
    let end_rel = content[body_start..].find(MANAGED_SECTION_END)?;
    let end_marker_start = body_start + end_rel;
    Some(ManagedSectionSpan {
        full: begin..end_marker_start + MANAGED_SECTION_END.len(),
        body: body_start..end_marker_start,
    })
}

/// Current body of the managed section, trimmed. `None` when absent.
pub fn managed_section_body(content: &str) -> Option<String> {
    find_managed_section(content).map(|s| content[s.body.start..s.body.end].trim().to_string())
}

/// Insert or replace the managed section, preserving everything outside it
/// byte-for-byte. When the content carries an *unclosed* begin marker the
/// write is skipped (content returned unchanged) — appending a fresh section
/// there would later make replacement swallow user content.
pub fn upsert_managed_section(content: &str, body: &str) -> String {
    let body = body.trim();
    match find_managed_section(content) {
        Some(span) => {
            let mut out = String::with_capacity(content.len() + body.len() + 2);
            out.push_str(&content[..span.body.start]);
            out.push('\n');
            out.push_str(body);
            out.push('\n');
            out.push_str(&content[span.body.end..]);
            out
        }
        None if content.contains(MANAGED_SECTION_BEGIN) => content.to_string(),
        None if content.trim().is_empty() => {
            format!("{MANAGED_SECTION_BEGIN}\n{body}\n{MANAGED_SECTION_END}\n")
        }
        None => {
            let base = content.strip_suffix('\n').unwrap_or(content);
            format!("{base}\n\n{MANAGED_SECTION_BEGIN}\n{body}\n{MANAGED_SECTION_END}\n")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_section_appends_block() {
        let content = "# Project\n\nAgent rules live here.\n";
        let out = upsert_managed_section(content, "rule: A");

        assert!(out.starts_with(content));
        assert!(out.contains(MANAGED_SECTION_BEGIN));
        assert!(out.contains("rule: A"));
        assert!(out.ends_with(&format!("{MANAGED_SECTION_END}\n")));
        assert_eq!(managed_section_body(&out).unwrap(), "rule: A");
    }

    #[test]
    fn body_extraction_roundtrip() {
        let out = upsert_managed_section("# P\n", "rule: A\nrule: B");
        assert_eq!(managed_section_body(&out).unwrap(), "rule: A\nrule: B");
        let prefix_len = out.find(MANAGED_SECTION_BEGIN).unwrap();
        assert_eq!(&out[..prefix_len], "# P\n\n");
    }

    #[test]
    fn replace_keeps_outside_byte_identical() {
        let head = "# Project\n\nAgent rules live here.\n";
        let rest = "\n\n## Other section\n\nuser content\n";
        let content = format!("{head}{MANAGED_SECTION_BEGIN}\nold body\n{MANAGED_SECTION_END}{rest}");

        let out = upsert_managed_section(&content, "new body");

        assert!(out.starts_with(&format!("{head}{MANAGED_SECTION_BEGIN}")));
        assert!(out.ends_with(&format!("{MANAGED_SECTION_END}{rest}")));
        assert_eq!(managed_section_body(&out).unwrap(), "new body");
    }

    #[test]
    fn unclosed_begin_marker_blocks_the_write() {
        let content = format!("intro\n{}\ndangling", MANAGED_SECTION_BEGIN);
        assert!(find_managed_section(&content).is_none());
        let out = upsert_managed_section(&content, "rule: A");
        assert_eq!(out, content); // nothing written, nothing lost
    }

    #[test]
    fn empty_content_starts_with_markers() {
        let out = upsert_managed_section("", "rule: A");
        assert!(out.starts_with(MANAGED_SECTION_BEGIN));
        assert!(out.ends_with(&format!("{MANAGED_SECTION_END}\n")));
    }

    #[test]
    fn body_is_trimmed_on_write() {
        let out = upsert_managed_section("x\n", "\n\n  rule: A  \n\n");
        assert_eq!(managed_section_body(&out).unwrap(), "rule: A");
    }
}
