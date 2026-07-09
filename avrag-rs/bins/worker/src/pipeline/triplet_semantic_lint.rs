//! Column-semantics lint for catalog mapping edges (prompt G1–G3).
//! Generalizes by cell role / label shape — not document-specific code prefixes.

const MAPPING_PREDICATES: &[&str] = &["标识为", "maps to"];
const MAX_MAPPING_OBJECT_CHARS: usize = 12;
const MAX_ZH_PREDICATE_CHARS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TripletSemanticViolation {
    MappingObjectNotShortLabel,
    OrgRoleNotCatalogMapping,
    PredicateTooLong,
}

pub(crate) fn triplet_semantic_violation(
    subject: &str,
    predicate: &str,
    object: &str,
) -> Option<TripletSemanticViolation> {
    let subject = subject.trim();
    let predicate = predicate.trim();
    let object = object.trim();

    if predicate.chars().count() > MAX_ZH_PREDICATE_CHARS
        && predicate.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
    {
        return Some(TripletSemanticViolation::PredicateTooLong);
    }

    let is_mapping = MAPPING_PREDICATES
        .iter()
        .any(|p| predicate.eq_ignore_ascii_case(p));
    if !is_mapping {
        return None;
    }

    if mapping_object_not_short_label(object) {
        return Some(TripletSemanticViolation::MappingObjectNotShortLabel);
    }

    if org_role_not_catalog_mapping_subject(subject) {
        return Some(TripletSemanticViolation::OrgRoleNotCatalogMapping);
    }

    None
}

fn mapping_object_not_short_label(object: &str) -> bool {
    object.chars().count() > MAX_MAPPING_OBJECT_CHARS
        || object.contains('，')
        || object.contains('。')
        || object.contains(',')
}

/// Org/role token without catalog-id shape (no digit segment after hyphen).
fn org_role_not_catalog_mapping_subject(subject: &str) -> bool {
    if subject.contains('-') {
        return false;
    }
    subject
        .chars()
        .all(|c| c.is_ascii_alphabetic() && c.is_ascii_uppercase())
        && (2..=8).contains(&subject.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_catalog_mapping() {
        assert!(triplet_semantic_violation("ACT-100", "标识为", "概念启动").is_none());
    }

    #[test]
    fn rejects_duty_sentence_object() {
        assert_eq!(
            triplet_semantic_violation("ME-10", "标识为", "探索可选概念和提供技术可选方案"),
            Some(TripletSemanticViolation::MappingObjectNotShortLabel)
        );
    }
}
