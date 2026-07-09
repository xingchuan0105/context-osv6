//! Jieba tokenization and content-word filtering for fingerprint metrics.
//!
//! # MVP rarity model (spec Open Question 1)
//!
//! There is **no global word-frequency table** in this MVP. "Rare" is defined
//! **relationally** within a draft:
//!
//! - **Demote** ops compare in-draft word frequencies against the draft's own Zipf fit.
//! - **Promote** candidates come from the reservoir (material cards / topic terms),
//!   filtered to words absent from the draft.
//!
//! **Upgrade path:** embed or load a BCC/BLCU frequency table (or jieba IDF ranks) and
//! use corpus-wide rarity thresholds for promote/demote enumeration.

use std::collections::HashSet;
use std::sync::OnceLock;

use jieba_rs::Jieba;

use crate::segment::char_len;

static JIEBA: OnceLock<Jieba> = OnceLock::new();

fn jieba() -> &'static Jieba {
    JIEBA.get_or_init(Jieba::new)
}

static STOPWORD_SET: OnceLock<HashSet<&'static str>> = OnceLock::new();

fn stopword_set() -> &'static HashSet<&'static str> {
    STOPWORD_SET.get_or_init(|| STOPWORDS.iter().copied().collect())
}

/// ~200 common Chinese function words (particles, pronouns, auxiliaries, conjunctions).
pub const STOPWORDS: &[&str] = &[
    "的", "了", "是", "在", "我", "有", "和", "就", "不", "人", "都", "一", "一个", "上", "也", "很",
    "到", "说", "要", "去", "你", "会", "着", "没有", "看", "好", "自己", "这", "那", "他", "她", "它",
    "们", "我们", "你们", "他们", "她们", "它们", "这个", "那个", "这些", "那些", "这里", "那里", "这样", "那样", "什么", "怎么",
    "为什么", "哪里", "哪个", "多少", "几", "一些", "一点", "一下", "一起", "一直", "一定", "一样", "一般", "一边", "一面", "已经",
    "还是", "或者", "而且", "但是", "因为", "所以", "如果", "虽然", "然而", "于是", "然后", "接着", "并且", "以及", "及其", "与其",
    "至于", "关于", "对于", "由于", "通过", "根据", "按照", "为了", "作为", "成为", "进行", "可以", "可能", "应该", "必须", "需要",
    "能够", "愿意", "希望", "觉得", "认为", "知道", "看到", "听到", "得到", "做出", "开始", "继续", "停止", "结束", "出现", "发生",
    "存在", "包括", "属于", "来自", "在于", "被", "把", "给", "让", "向", "对", "从", "以", "与", "及", "并",
    "而", "且", "或", "则", "即", "乃", "所", "之", "其", "此", "彼", "某", "各", "每", "另", "别",
    "其他", "其它", "任何", "所有", "整个", "部分", "方面", "情况", "问题", "时候", "时间", "地方", "方式", "程度", "结果", "原因",
    "过程", "状态", "关系", "作用", "意义", "内容", "形式", "条件", "水平", "能力", "方法", "完全", "非常", "十分", "特别", "尤其",
    "比较", "相对", "更加", "越来越", "最", "更", "还", "再", "又", "才", "刚", "曾", "将", "呢", "吧", "啊",
    "呀", "嘛", "吗", "么", "哪", "谁", "怎样", "如何", "何时", "何地", "为何", "是不是", "有没有", "能不能", "会不会", "要不要",
    "好不好", "对不对", "行不行", "而已", "罢了", "的话", "来说", "而言", "之类", "等等", "什么的", "也好", "也罢", "似的", "左右", "上下",
    "前后", "内外", "中间", "当中", "之间", "之中", "之内", "之外", "以上", "以下", "以前", "以后", "以来", "直到", "直至", "自从",
    "因而", "因此", "故", "且说", "再说", "总之", "总而言之", "换言之", "也就是说", "换句话说", "由此可见", "不仅如此", "不但", "不仅", "无论", "不管",
    "尽管", "即使", "假如", "要是", "以免", "以便", "对此", "得", "过", "啦", "哇", "哟", "嗯", "哼", "唉", "哦",
    "啥", "咋", "呗", "哎", "嘿", "噢", "与其", "不如", "宁可", "宁愿", "宁肯", "倒不如", "就是说", "要我说", "依我看", "照我看",
    "在我看来", "依我之见", "照我看来", "据我所知", "就我而言", "就我看来", "就我所知", "就我所见", "那就是", "那就是说", "换言之", "换言之", "换言之", "换言之", "换言之", "换言之",
];

/// Jieba cut; drops punctuation-only and whitespace tokens.
pub fn tokens(text: &str) -> Vec<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    jieba()
        .cut(trimmed, false)
        .into_iter()
        .filter(|w| is_token_word(w))
        .map(|w| w.to_string())
        .collect()
}

fn is_token_word(w: &str) -> bool {
    let t = w.trim();
    !t.is_empty() && t.chars().any(|c| c.is_alphanumeric() || is_cjk(c))
}

fn is_cjk(c: char) -> bool {
    matches!(
        c,
        '\u{4e00}'..='\u{9fff}'
            | '\u{3400}'..='\u{4dbf}'
            | '\u{20000}'..='\u{2a6df}'
            | '\u{2a700}'..='\u{2b73f}'
            | '\u{2b740}'..='\u{2b81f}'
            | '\u{2b820}'..='\u{2ceaf}'
            | '\u{2ceb0}'..='\u{2ebef}'
            | '\u{30000}'..='\u{3134f}'
    )
}

/// Content word: at least two non-whitespace characters and not a stopword.
pub fn is_content_word(w: &str) -> bool {
    char_len(w) >= 2 && !stopword_set().contains(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_drop_punctuation() {
        let ts = tokens("你好，世界！");
        assert!(ts.iter().all(|w| !w.chars().all(|c| c.is_ascii_punctuation())));
        assert!(ts.contains(&"你好".to_string()) || ts.contains(&"世界".to_string()));
    }

    #[test]
    fn content_word_filter() {
        assert!(!is_content_word("的"));
        assert!(!is_content_word("了"));
        assert!(!is_content_word("是"));
        assert!(!is_content_word("a"));
        assert!(is_content_word("人工智能"));
        assert!(is_content_word("深度学习"));
    }

    #[test]
    fn stopwords_count_about_two_hundred() {
        assert!(
            STOPWORDS.len() >= 180,
            "expected ~200 stopwords, got {}",
            STOPWORDS.len()
        );
    }

    #[test]
    fn tokens_on_prose_sentence() {
        let ts = tokens("深度学习改变了自然语言处理。");
        assert!(!ts.is_empty());
        assert!(ts.iter().any(|w| w.contains("学习") || w.contains("深度")));
    }
}
