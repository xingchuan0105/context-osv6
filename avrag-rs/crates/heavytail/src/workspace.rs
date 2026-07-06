//! Line-addressed draft workspace: sentence IDs, canonical rendering, plain export.

use crate::segment::{split_sentences, RawSentence};

/// Stable sentence address: `s07`, `s07a`, `s07ab`, …
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SentenceId(pub String);

impl SentenceId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Split children: `s07` → (`s07a`, `s07b`); `s07a` → (`s07aa`, `s07ab`).
    pub fn children(&self) -> (SentenceId, SentenceId) {
        (
            SentenceId(format!("{}a", self.0)),
            SentenceId(format!("{}b", self.0)),
        )
    }

    /// `^s[0-9]+[a-z]*$`
    pub fn is_valid(s: &str) -> bool {
        let Some(rest) = s.strip_prefix('s') else {
            return false;
        };
        if rest.is_empty() {
            return false;
        }
        let mut chars = rest.chars();
        let first = chars.next().unwrap();
        if !first.is_ascii_digit() {
            return false;
        }
        for c in chars {
            if !c.is_ascii_digit() && !c.is_ascii_lowercase() {
                return false;
            }
        }
        true
    }

    fn numeric_part(&self) -> usize {
        self.0
            .chars()
            .skip(1)
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or(0)
    }
}

impl std::fmt::Display for SentenceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SentenceRecord {
    pub id: SentenceId,
    pub text: String,
    pub para: usize,
    pub tombstone: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RhythmMode {
    ShortBurst,
    LongFlow,
    Mixed,
}

impl RhythmMode {
    fn canonical_label(self) -> &'static str {
        match self {
            Self::ShortBurst => "short-burst",
            Self::LongFlow => "long-flow",
            Self::Mixed => "mixed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ParagraphRecord {
    pub idx: usize,
    pub rhythm: RhythmMode,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DraftWorkspace {
    pub sentences: Vec<SentenceRecord>,
    pub paragraphs: Vec<ParagraphRecord>,
}

impl DraftWorkspace {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a freshly drafted section (free prose) — segmentation + new IDs
    /// continuing the global counter. Never reuses tombstoned IDs.
    pub fn append_section(&mut self, prose: &str, rhythms: &[RhythmMode]) {
        let raw = split_sentences(prose, false);
        if raw.is_empty() {
            return;
        }

        let base_para = self.paragraphs.len();
        let mut num = self.next_sentence_num();

        let max_local_para = raw.iter().map(|s| s.para_idx).max().unwrap_or(0);
        for local_para in 0..=max_local_para {
            let global_para = base_para + local_para;
            let rhythm = rhythms
                .get(local_para)
                .copied()
                .or_else(|| rhythms.last().copied())
                .unwrap_or(RhythmMode::Mixed);
            self.paragraphs.push(ParagraphRecord {
                idx: global_para,
                rhythm,
            });
        }

        for RawSentence { text, para_idx } in raw {
            let id = SentenceId(format!("s{num:02}"));
            num += 1;
            self.sentences.push(SentenceRecord {
                id,
                text,
                para: base_para + para_idx,
                tombstone: false,
            });
        }
    }

    /// Canonical form for LLM consumption (spec §5.1): headers + id-prefixed lines.
    pub fn render_canonical(&self) -> String {
        let mut out = String::new();
        for para in &self.paragraphs {
            let live: Vec<_> = self
                .sentences
                .iter()
                .filter(|s| s.para == para.idx && !s.tombstone)
                .collect();
            if live.is_empty() {
                continue;
            }
            if !out.is_empty() {
                while out.ends_with('\n') {
                    out.pop();
                }
                out.push('\n');
            }
            out.push_str(&format!(
                "# p{} | rhythm: {}\n",
                para.idx + 1,
                para.rhythm.canonical_label()
            ));
            for s in live {
                out.push_str(&format!("{}| {}\n", s.id, s.text));
            }
        }
        while out.ends_with('\n') {
            out.pop();
        }
        out
    }

    /// Final text: strip IDs/tombstones, join by paragraph.
    pub fn render_plain(&self) -> String {
        self.paragraphs
            .iter()
            .filter_map(|para| {
                let text: String = self
                    .sentences
                    .iter()
                    .filter(|s| s.para == para.idx && !s.tombstone)
                    .map(|s| s.text.as_str())
                    .collect();
                if text.is_empty() {
                    None
                } else {
                    Some(text)
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Live (non-tombstoned) sentences in document order.
    pub fn live(&self) -> impl Iterator<Item = &SentenceRecord> {
        self.sentences.iter().filter(|s| !s.tombstone)
    }

    pub fn find_index(&self, id: &SentenceId) -> Option<usize> {
        self.sentences.iter().position(|s| &s.id == id)
    }

    pub fn get(&self, id: &SentenceId) -> Option<&SentenceRecord> {
        self.sentences.iter().find(|s| &s.id == id)
    }

    pub fn get_mut(&mut self, id: &SentenceId) -> Option<&mut SentenceRecord> {
        self.sentences.iter_mut().find(|s| &s.id == id)
    }

    pub fn is_tombstoned(&self, id: &SentenceId) -> bool {
        self.get(id).is_some_and(|s| s.tombstone)
    }

    fn next_sentence_num(&self) -> usize {
        self.sentences
            .iter()
            .map(|s| s.id.numeric_part())
            .max()
            .map(|n| n + 1)
            .unwrap_or(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normalize_prose(prose: &str) -> String {
        let raw = split_sentences(prose, false);
        if raw.is_empty() {
            return String::new();
        }
        let mut paragraphs: Vec<Vec<String>> = Vec::new();
        for s in raw {
            let idx = s.para_idx;
            while paragraphs.len() <= idx {
                paragraphs.push(Vec::new());
            }
            paragraphs[idx].push(s.text);
        }
        paragraphs
            .into_iter()
            .map(|sents| sents.concat())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    #[test]
    fn sentence_id_is_valid() {
        assert!(SentenceId::is_valid("s01"));
        assert!(SentenceId::is_valid("s07"));
        assert!(SentenceId::is_valid("s07a"));
        assert!(SentenceId::is_valid("s07ab"));
        assert!(!SentenceId::is_valid("s"));
        assert!(!SentenceId::is_valid("x07"));
        assert!(!SentenceId::is_valid("s07A"));
        assert!(!SentenceId::is_valid("07"));
    }

    #[test]
    fn sentence_id_children() {
        let id = SentenceId("s07".into());
        let (a, b) = id.children();
        assert_eq!(a.0, "s07a");
        assert_eq!(b.0, "s07b");
        assert!(a < b);

        let (aa, ab) = a.children();
        assert_eq!(aa.0, "s07aa");
        assert_eq!(ab.0, "s07ab");
        assert!(aa < ab);
    }

    #[test]
    fn canonical_round_trip() {
        let prose = "第一句。第二句。\n\n第三段第一句。第三段第二句。";
        let mut ws = DraftWorkspace::default();
        ws.append_section(
            prose,
            &[RhythmMode::Mixed, RhythmMode::ShortBurst],
        );
        assert_eq!(ws.render_plain(), normalize_prose(prose));
    }

    #[test]
    fn id_continuation_across_sections() {
        let mut ws = DraftWorkspace::default();
        ws.append_section("第一句。第二句。", &[RhythmMode::Mixed]);
        assert_eq!(ws.sentences[0].id.0, "s01");
        assert_eq!(ws.sentences[1].id.0, "s02");

        ws.append_section("第三句。", &[RhythmMode::Mixed]);
        assert_eq!(ws.sentences[2].id.0, "s03");

        ws.sentences[1].tombstone = true;
        ws.append_section("第四句。", &[RhythmMode::Mixed]);
        let last = ws.sentences.last().unwrap();
        assert_eq!(last.id.0, "s04");
        assert!(ws.sentences.iter().any(|s| s.id.0 == "s02" && s.tombstone));
    }

    #[test]
    fn children_document_order() {
        let parent = SentenceId("s03".into());
        let (a, b) = parent.children();
        assert!(a < b);

        let mut ws = DraftWorkspace::default();
        ws.sentences.push(SentenceRecord {
            id: parent,
            text: "整句。".into(),
            para: 0,
            tombstone: true,
        });
        ws.sentences.push(SentenceRecord {
            id: a.clone(),
            text: "前半。".into(),
            para: 0,
            tombstone: false,
        });
        ws.sentences.push(SentenceRecord {
            id: b.clone(),
            text: "后半。".into(),
            para: 0,
            tombstone: false,
        });
        ws.paragraphs.push(ParagraphRecord {
            idx: 0,
            rhythm: RhythmMode::Mixed,
        });

        let ids: Vec<_> = ws.live().map(|s| s.id.clone()).collect();
        assert_eq!(ids, vec![a, b]);
    }

    #[test]
    fn render_canonical_fixture() {
        let prose = "深夜的交易大厅安静得反常。屏幕还亮着。风险引擎在凌晨两点十七分弹出第一条告警，没有人注意到它。\n\n长段落的示例句子在这里展开。";
        let mut ws = DraftWorkspace::default();
        ws.append_section(
            prose,
            &[RhythmMode::ShortBurst, RhythmMode::LongFlow],
        );

        let expected = "\
# p1 | rhythm: short-burst
s01| 深夜的交易大厅安静得反常。
s02| 屏幕还亮着。
s03| 风险引擎在凌晨两点十七分弹出第一条告警，没有人注意到它。
# p2 | rhythm: long-flow
s04| 长段落的示例句子在这里展开。";

        assert_eq!(ws.render_canonical(), expected);
    }

    #[test]
    fn live_skips_tombstones() {
        let mut ws = DraftWorkspace::default();
        ws.append_section("甲。乙。", &[RhythmMode::Mixed]);
        ws.sentences[0].tombstone = true;
        let live: Vec<_> = ws.live().map(|s| s.text.clone()).collect();
        assert_eq!(live, vec!["乙。"]);
    }
}
