//! SaC SDK capability gate (A3).
//!
//! Product modes open a **subset** of sandbox primitives. Host denies RPC
//! methods outside the subset with `capability_denied`.
//! 原语清单单一事实源在 `contracts::sdk_primitives` 注册表(D10)。

use std::collections::HashSet;

/// Resolve SaC primitive ids from product capability flags.
/// 从 `contracts::sdk_primitives` 注册表派生(D10)——不再维护三组硬编码常量。
pub fn sdk_primitives_for_caps(rag: bool, search: bool) -> Vec<&'static str> {
    use contracts::sdk_primitives::{SdkCapability, ids_for};
    let mut out = ids_for(SdkCapability::BASE);
    if rag {
        for p in ids_for(SdkCapability::RAG) {
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    if search {
        for p in ids_for(SdkCapability::SEARCH) {
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// `allowed` empty ⇒ no gate (tests / legacy). Non-empty ⇒ only listed methods.
/// legacy 别名(dense_search/lexical_search)已在注册表/shime 层退役(D10)。
pub fn method_allowed(allowed: &HashSet<String>, method: &str) -> bool {
    if allowed.is_empty() {
        return true;
    }
    allowed.contains(method)
}

/// Error payload for denied RPC (Python shim raises RuntimeError with message).
pub fn capability_denied_error(method: &str) -> serde_json::Value {
    serde_json::json!({
        "error": {
            "code": "capability_denied",
            "message": format!(
                "SDK method `{method}` is not enabled for this capability mode"
            ),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_chat_only_base() {
        let p = sdk_primitives_for_caps(false, false);
        assert!(p.contains(&"history"));
        assert!(p.contains(&"save"));
        assert!(!p.contains(&"dense"));
        assert!(!p.contains(&"web"));
        assert!(!p.contains(&"grep"));
    }

    #[test]
    fn rag_has_retrieval_not_web() {
        let p = sdk_primitives_for_caps(true, false);
        assert!(p.contains(&"dense"));
        assert!(p.contains(&"lexical"));
        assert!(p.contains(&"grep"));
        assert!(!p.contains(&"graph"));
        assert!(!p.contains(&"web"));
        assert!(!p.contains(&"fetch"));
    }

    #[test]
    fn search_has_web_not_dense() {
        let p = sdk_primitives_for_caps(false, true);
        assert!(p.contains(&"web"));
        assert!(p.contains(&"fetch"));
        assert!(!p.contains(&"dense"), "search-only must not mount dense");
        assert!(!p.contains(&"grep"));
    }

    #[test]
    fn dual_is_union() {
        let p = sdk_primitives_for_caps(true, true);
        assert!(p.contains(&"grep"));
        assert!(p.contains(&"web"));
        // dense comes from RAG only (VGRAG inside dense); dual still has it via rag.
        assert!(p.contains(&"dense"));
    }

    #[test]
    fn empty_allowlist_is_open() {
        let set = HashSet::new();
        assert!(method_allowed(&set, "dense"));
        assert!(method_allowed(&set, "web"));
    }

    #[test]
    fn legacy_alias_no_longer_allowed() {
        // D10:legacy 别名(dense_search/lexical_search)已退役,只认注册表 id。
        let set: HashSet<String> = ["dense".into()].into_iter().collect();
        assert!(!method_allowed(&set, "dense_search"));
        assert!(!method_allowed(&set, "lexical_search"));
        assert!(method_allowed(&set, "dense"));
    }
}
