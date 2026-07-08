//! Post-extraction predicate normalization (rules-only, no LLM cost).

/// Normalize a predicate to the closed-set vocabulary; returns `(canonical, original_if_changed)`.
pub(crate) fn normalize_predicate(predicate: &str) -> (String, Option<String>) {
    let trimmed = predicate.trim();
    if trimmed.is_empty() {
        return (String::new(), None);
    }

    let key = trimmed.to_lowercase();
    let canonical = PREDICATE_SYNONYMS
        .iter()
        .find_map(|(variants, target)| {
            if variants.iter().any(|v| v.eq_ignore_ascii_case(&key) || *v == trimmed) {
                Some((*target).to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| trimmed.to_string());

    if canonical == trimmed {
        (canonical, None)
    } else {
        (canonical, Some(trimmed.to_string()))
    }
}

/// (variant forms, canonical predicate)
const PREDICATE_SYNONYMS: &[(&[&str], &str)] = &[
    (&["隶属于", "归属于", "是…的一部分", "part of", "belongs to"], "属于"),
    (&["对应", "对应于", "maps to"], "映射到"),
    (
        &["is deprecated", "已废弃", "被废弃", "deprecated"],
        "被替代为",
    ),
    (&["implements", "实现于", "implemented by"], "实现"),
    (&["calls", "invokes", "invoked by"], "调用"),
    (&["executes", "executed by"], "执行"),
    (&["包括", "含有", "includes", "contains", "comprises"], "包含"),
    (
        &["adds method", "adds test", "add method", "add test"],
        "新增",
    ),
    (&["transitions to", "transitioned to"], "转换至"),
    (&["编写", "著", "authored", "authored by"], "撰写"),
    (&["extends with", "extended with"], "扩展"),
    (&["supports", "supported by"], "支持"),
    (&["performs in", "performed in"], "执行于"),
    (&["operates as", "operated as"], "作为"),
    (&["outputs", "output"], "输出"),
    (&["涉及", "relates to", "related to"], "关联"),
    (&["使用", "uses", "used by"], "使用"),
    (&["继承", "inherits", "inherited from"], "继承"),
    (&["通过", "via", "through"], "经由"),
    (&["用于", "used for"], "用于"),
    (&["封禁", "blocks", "blocked by"], "阻止"),
    (&["强制于", "enforced on"], "强制于"),
    (&["新增于", "added to"], "新增"),
    (&["零覆盖于", "zero coverage on"], "零覆盖于"),
    (&["被拒绝", "rejected"], "被拒绝"),
    (&["设计", "designed"], "设计"),
    (&["分析", "analyzed"], "分析"),
    (&["组织", "organized"], "组织"),
    (&["定位", "positioned as"], "定位为"),
    (&["开始", "started"], "开始"),
    (&["输出", "produces"], "产出"),
];

#[cfg(test)]
mod tests {
    use super::normalize_predicate;

    #[test]
    fn maps_synonym_to_canonical() {
        let (c, orig) = normalize_predicate("implements");
        assert_eq!(c, "实现");
        assert_eq!(orig.as_deref(), Some("implements"));
    }

    #[test]
    fn preserves_unknown_predicate() {
        let (c, orig) = normalize_predicate("自定义关系");
        assert_eq!(c, "自定义关系");
        assert!(orig.is_none());
    }

    #[test]
    fn maps_to_chinese_canonical() {
        let (c, _) = normalize_predicate("maps to");
        assert_eq!(c, "映射到");
    }
}
