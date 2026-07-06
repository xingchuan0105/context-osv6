//! Quote-aware Chinese sentence segmentation for free prose.

/// A sentence extracted from prose, tagged with its paragraph index (0-based).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSentence {
    pub text: String,
    pub para_idx: usize,
}

/// Character count excluding whitespace (spec: length = non-whitespace chars).
pub fn char_len(s: &str) -> usize {
    s.chars().filter(|c| !c.is_whitespace()).count()
}

fn is_sentence_terminator(c: char, semicolon_splits: bool) -> bool {
    matches!(c, '。' | '！' | '？' | '!' | '?') || (semicolon_splits && c == '；')
}

fn matching_closer(opener: char) -> Option<char> {
    match opener {
        '"' => Some('"'),
        '\u{201c}' => Some('\u{201d}'),
        '\'' => Some('\''),
        '\u{2018}' => Some('\u{2019}'),
        '「' => Some('」'),
        '『' => Some('』'),
        '（' | '(' => Some('）'),
        _ => None,
    }
}

fn is_opener(c: char) -> bool {
    matches!(
        c,
        '"' | '\u{201c}' | '\'' | '\u{2018}' | '「' | '『' | '（' | '('
    )
}

fn is_closer(c: char) -> bool {
    matches!(
        c,
        '"' | '\u{201d}' | '\'' | '\u{2019}' | '」' | '』' | '）' | ')'
    )
}

fn absorb_trailing_closers(chars: &[char], end: usize) -> usize {
    let mut i = end;
    while i < chars.len() && (is_closer(chars[i]) || chars[i].is_whitespace()) {
        i += 1;
    }
    i
}

fn at_ellipsis(chars: &[char], idx: usize) -> bool {
    if idx >= chars.len() {
        return false;
    }
    if chars[idx] == '…' {
        return true;
    }
    if chars[idx] == '.' || chars[idx] == '．' {
        let ch = chars[idx];
        let mut j = idx;
        while j < chars.len() && chars[j] == ch {
            j += 1;
        }
        return j - idx >= 2;
    }
    false
}

fn skip_ellipsis(chars: &[char], mut idx: usize) -> usize {
    if idx >= chars.len() {
        return idx;
    }
    if chars[idx] == '…' {
        while idx < chars.len() && chars[idx] == '…' {
            idx += 1;
        }
        return idx;
    }
    if chars[idx] == '.' || chars[idx] == '．' {
        let ch = chars[idx];
        while idx < chars.len() && chars[idx] == ch {
            idx += 1;
        }
        return idx;
    }
    idx
}

/// Split free prose into sentences.
///
/// Terminators: `。！？` (and `；` when `semicolon_splits` — default false).
/// Trailing closing quotes/brackets follow their sentence. Paragraphs split on blank lines.
/// Ellipsis (`……`) is never a terminator.
pub fn split_sentences(prose: &str, semicolon_splits: bool) -> Vec<RawSentence> {
    let mut out = Vec::new();
    for (para_idx, para) in split_paragraphs(prose).into_iter().enumerate() {
        out.extend(split_paragraph(&para, para_idx, semicolon_splits));
    }
    out
}

fn split_paragraphs(prose: &str) -> Vec<String> {
    let trimmed = prose.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut paragraphs = Vec::new();
    let chars: Vec<char> = trimmed.chars().collect();
    let mut para_start = 0usize;
    let mut i = 0usize;

    while i < chars.len() {
        if chars[i] == '\n' {
            let break_start = i;
            let mut j = i;
            let mut newline_count = 0usize;
            while j < chars.len() {
                if chars[j] == '\n' {
                    newline_count += 1;
                    j += 1;
                } else if chars[j].is_whitespace() {
                    j += 1;
                } else {
                    break;
                }
            }
            if newline_count >= 2 {
                let para: String = chars[para_start..break_start].iter().collect();
                let t = para.trim();
                if !t.is_empty() {
                    paragraphs.push(t.to_string());
                }
                para_start = j;
                i = j;
                continue;
            }
        }
        i += 1;
    }

    let tail: String = chars[para_start..].iter().collect();
    let t = tail.trim();
    if !t.is_empty() {
        paragraphs.push(t.to_string());
    }
    paragraphs
}

fn split_paragraph(text: &str, para_idx: usize, semicolon_splits: bool) -> Vec<RawSentence> {
    let mut out = Vec::new();
    let mut sentence_start = 0usize;
    let mut i = 0usize;
    let chars: Vec<char> = text.chars().collect();
    let mut quote_stack: Vec<char> = Vec::new();

    while i < chars.len() {
        let c = chars[i];

        if is_opener(c) {
            quote_stack.push(c);
            i += 1;
            continue;
        }

        if is_closer(c) {
            if quote_stack
                .last()
                .and_then(|&op| matching_closer(op))
                .is_some_and(|cl| cl == c)
            {
                quote_stack.pop();
            }
            i += 1;
            continue;
        }

        if at_ellipsis(&chars, i) {
            i = skip_ellipsis(&chars, i);
            continue;
        }

        if is_sentence_terminator(c, semicolon_splits) {
            if !quote_stack.is_empty() {
                let expected = quote_stack.last().and_then(|&op| matching_closer(op));
                let mut k = i + 1;
                while k < chars.len() && chars[k].is_whitespace() {
                    k += 1;
                }
                if expected != Some(chars.get(k).copied().unwrap_or('\0')) {
                    i += 1;
                    continue;
                }
            }

            let end = absorb_trailing_closers(&chars, i + 1);
            let text: String = chars[sentence_start..end].iter().collect();
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                out.push(RawSentence {
                    text: trimmed.to_string(),
                    para_idx,
                });
            }
            sentence_start = end;
            i = end;
            quote_stack.clear();
            continue;
        }

        i += 1;
    }

    let tail: String = chars[sentence_start..].iter().collect();
    let trimmed = tail.trim();
    if !trimmed.is_empty() {
        out.push(RawSentence {
            text: trimmed.to_string(),
            para_idx,
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_len_excludes_whitespace() {
        assert_eq!(char_len("a b\tc"), 3);
        assert_eq!(char_len("你好 世界"), 4);
    }

    #[test]
    fn quote_attached_to_sentence() {
        let sents = split_sentences("他说：\"走吧。\"然后离开了。", false);
        assert_eq!(sents.len(), 2);
        assert_eq!(sents[0].text, "他说：\"走吧。\"");
        assert_eq!(sents[1].text, "然后离开了。");
    }

    #[test]
    fn curly_quotes_attached() {
        let prose = "她问：\u{201c}真的吗？\u{201d}我点头。";
        let sents = split_sentences(prose, false);
        assert_eq!(sents.len(), 2);
        assert!(sents[0].text.ends_with('\u{201d}'));
    }

    #[test]
    fn ellipsis_not_terminator() {
        let sents = split_sentences("他等等……然后走了。", false);
        assert_eq!(sents.len(), 1);
        assert!(sents[0].text.contains('…'));
    }

    #[test]
    fn mid_quote_period_does_not_split() {
        let prose = "这是\u{201c}一半。继续\u{201d}的测试。";
        let sents = split_sentences(prose, false);
        assert_eq!(sents.len(), 1);
    }

    #[test]
    fn paragraph_blank_lines() {
        let prose = "第一句。\n\n第二段第一句。\n\n\n第三段。";
        let sents = split_sentences(prose, false);
        assert_eq!(sents.len(), 3);
        assert_eq!(sents[0].para_idx, 0);
        assert_eq!(sents[1].para_idx, 1);
        assert_eq!(sents[2].para_idx, 2);
    }

    #[test]
    fn final_fragment_without_terminator() {
        let sents = split_sentences("只有前半句。没有结尾", false);
        assert_eq!(sents.len(), 2);
        assert_eq!(sents[0].text, "只有前半句。");
        assert_eq!(sents[1].text, "没有结尾");
    }

    #[test]
    fn semicolon_splits_when_enabled() {
        let sents = split_sentences("甲；乙。", true);
        assert_eq!(sents.len(), 2);
        assert_eq!(sents[0].text, "甲；");
        assert_eq!(sents[1].text, "乙。");
    }

    #[test]
    fn semicolon_not_split_by_default() {
        let sents = split_sentences("甲；乙。", false);
        assert_eq!(sents.len(), 1);
    }
}
