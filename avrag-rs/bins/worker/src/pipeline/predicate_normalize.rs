//! Closed-set **ontological** predicates for KG triples (rules-only).
//!
//! Paradigm: foundational / conceptual ontology — few structural relations
//! between entities (type, mereology, participation, dependence, location,
//! denotation). Domain meaning lives on **nodes** (kinds, names), not on an
//! open verb vocabulary.
//!
//! Unknown predicates are dropped when strict (default). Soft:
//! `TRIPLET_PREDICATE_STRICT=0`.

/// Canonical edge types (ZH ids stored in `predicate`).
///
/// | id | Ontological role |
/// |----|------------------|
/// | 类型 | is-a / classification (individual → kind, phase, category) |
/// | 部分 | mereology (S is part of O) |
/// | 参与 | participation (continuant → process / activity / event) |
/// | 依赖 | existential / functional dependence (S depends on O) |
/// | 位于 | location (S located in place or temporal region O) |
/// | 标识 | denotation (stable code/id → short name; catalog rows) |
pub const CANONICAL_PREDICATES: &[&str] =
    &["类型", "部分", "参与", "依赖", "位于", "标识"];

/// Normalize to closed ontology. Empty canonical ⇒ drop edge (strict).
pub(crate) fn normalize_predicate(predicate: &str) -> (String, Option<String>) {
    let trimmed = predicate.trim();
    if trimmed.is_empty() {
        return (String::new(), None);
    }

    if CANONICAL_PREDICATES.iter().any(|c| *c == trimmed) {
        return (trimmed.to_string(), None);
    }

    let key = trimmed.to_lowercase();
    if let Some(target) = find_synonym(&key, trimmed) {
        if target == trimmed {
            return (target.to_string(), None);
        }
        return (target.to_string(), Some(trimmed.to_string()));
    }

    let compact: String = trimmed
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_' && *c != '-')
        .collect::<String>()
        .to_lowercase();
    if let Some(target) = PREDICATE_SYNONYMS.iter().find_map(|(variants, target)| {
        if variants.iter().any(|v| {
            let vc: String = v
                .chars()
                .filter(|c| !c.is_whitespace() && *c != '_' && *c != '-')
                .collect::<String>()
                .to_lowercase();
            vc == compact
        }) {
            Some(*target)
        } else {
            None
        }
    }) {
        return (target.to_string(), Some(trimmed.to_string()));
    }

    if predicate_strict_mode() {
        (String::new(), Some(trimmed.to_string()))
    } else {
        (trimmed.to_string(), None)
    }
}

fn find_synonym(key: &str, trimmed: &str) -> Option<&'static str> {
    PREDICATE_SYNONYMS.iter().find_map(|(variants, target)| {
        if variants
            .iter()
            .any(|v| v.eq_ignore_ascii_case(key) || *v == trimmed)
        {
            Some(*target)
        } else {
            None
        }
    })
}

/// Default **strict**. `TRIPLET_PREDICATE_STRICT=0|false|off` keeps unknowns.
pub(crate) fn predicate_strict_mode() -> bool {
    match std::env::var("TRIPLET_PREDICATE_STRICT") {
        Ok(v) => {
            let t = v.trim().to_ascii_lowercase();
            !(t == "0" || t == "false" || t == "off" || t == "no")
        }
        Err(_) => true,
    }
}

/// Synonyms → ontological canonical (not a business-verb enum: fold domain
/// verbs into foundational relations).
const PREDICATE_SYNONYMS: &[(&[&str], &str)] = &[
    // ── 类型 (is-a / classification) ──
    (
        &[
            "类型",
            "是",
            "为",
            "属于",
            "隶属于",
            "归属于",
            "归入",
            "归属",
            "类别为",
            "分类为",
            "is a",
            "is-a",
            "isa",
            "instance of",
            "type of",
            "rdf:type",
            "a kind of",
            "kind of",
            "belongs to",
            "member of",
            "part of category",
            "阶段为",
            "角色为",
        ],
        "类型",
    ),
    // ── 部分 (mereology: S part of O) ──
    (
        &[
            "部分",
            "部分于",
            "是…的一部分",
            "组成于",
            "构成…的部分",
            "part of",
            "part-of",
            "component of",
            "子",
            // inverse surface forms → still store as 部分 with S=part O=whole when LLM
            // emitted whole→part: host cannot always flip; map both surface words here
            // and prompt requires S=part, O=whole.
            "包含",
            "包括",
            "含有",
            "includes",
            "contains",
            "comprises",
            "has part",
            "has_part",
        ],
        "部分",
    ),
    // ── 参与 (continuant participates in process) ──
    (
        &[
            "参与",
            "参加",
            "参与于",
            "执行",
            "执行于",
            "进行",
            "负责",
            "承担",
            "implements",
            "implemented by",
            "实现",
            "实现于",
            "落地",
            "落实",
            "调用",
            "calls",
            "invokes",
            "executes",
            "performed by",
            "performs",
            "participates in",
            "participation",
            "撰写",
            "编写",
            "authored",
            "written by",
            "设计",
            "分析",
            "组织",
        ],
        "参与",
    ),
    // ── 依赖 (S depends on O) ──
    (
        &[
            "依赖",
            "依赖于",
            "depends on",
            "dependent on",
            "requires",
            "需要",
            "基于",
            "based on",
            "使用",
            "采用",
            "uses",
            "used by",
            "utilizes",
            "利用",
            "应用",
            "通过",
            "via",
            "through",
            "经由",
            "用于",
            "used for",
            "适用于",
            "适用",
            "applies to",
            "supported by",
            "支持",
            "supports",
        ],
        "依赖",
    ),
    // ── 位于 (location in space/time continuum) ──
    (
        &[
            "位于",
            "地处",
            "设在",
            "发生在",
            "发生于",
            "located in",
            "located at",
            "based in",
            "headquartered in",
            "in",
            "at",
        ],
        "位于",
    ),
    // ── 标识 (denotation: code → name) ──
    (
        &[
            "标识",
            "标识为",
            "标识为",
            "maps to",
            "mapped to",
            "map to",
            "映射到",
            "对应于",
            "对应",
            "名为",
            "称作",
            "叫做",
            "denotes",
            "stands for",
            "named",
            "labelled",
            "labeled",
        ],
        "标识",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ontology_set_is_small() {
        assert!(CANONICAL_PREDICATES.len() <= 8);
        assert!(CANONICAL_PREDICATES.contains(&"类型"));
        assert!(CANONICAL_PREDICATES.contains(&"部分"));
        assert!(CANONICAL_PREDICATES.contains(&"参与"));
    }

    #[test]
    fn belongs_to_is_type() {
        let (c, _) = normalize_predicate("属于");
        assert_eq!(c, "类型");
        let (c2, _) = normalize_predicate("belongs to");
        assert_eq!(c2, "类型");
    }

    #[test]
    fn maps_to_is_denotation() {
        let (c, _) = normalize_predicate("maps to");
        assert_eq!(c, "标识");
        let (c2, _) = normalize_predicate("标识为");
        assert_eq!(c2, "标识");
    }

    #[test]
    fn implements_is_participation() {
        let (c, _) = normalize_predicate("implements");
        assert_eq!(c, "参与");
    }

    #[test]
    fn uses_is_dependence() {
        let (c, _) = normalize_predicate("使用");
        assert_eq!(c, "依赖");
    }

    #[test]
    fn drops_unknown_strict() {
        let (c, orig) = normalize_predicate("自定义关系");
        assert!(c.is_empty());
        assert_eq!(orig.as_deref(), Some("自定义关系"));
    }

    #[test]
    fn canonical_passthrough() {
        let (c, o) = normalize_predicate("部分");
        assert_eq!(c, "部分");
        assert!(o.is_none());
    }
}
