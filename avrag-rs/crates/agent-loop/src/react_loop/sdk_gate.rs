//! SaC SDK capability gate (A3).
//!
//! Product modes open a **subset** of sandbox primitives. Host denies RPC
//! methods outside the subset with `capability_denied`.

use std::collections::HashSet;

/// Always available when the SaC bridge is active (cross-turn + light memory).
const BASE_PRIMITIVES: &[&str] = &["save", "load", "history", "user_profile"];

/// Workspace retrieval (rag).
const RAG_PRIMITIVES: &[&str] = &[
    "dense",
    "lexical",
    "grep",
    "doc_profile",
    "doc_summary",
];

/// Web surface (search). Design also allows `dense` for hybrid search mode.
const SEARCH_PRIMITIVES: &[&str] = &["web", "fetch", "dense"];

/// Resolve SaC primitive ids from product capability flags.
pub fn sdk_primitives_for_caps(rag: bool, search: bool) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = BASE_PRIMITIVES.to_vec();
    if rag {
        for p in RAG_PRIMITIVES {
            if !out.contains(p) {
                out.push(p);
            }
        }
    }
    if search {
        for p in SEARCH_PRIMITIVES {
            if !out.contains(p) {
                out.push(p);
            }
        }
    }
    out
}

/// Canonicalize legacy alias names before allow-check.
pub fn canonicalize_sdk_method(method: &str) -> &str {
    match method {
        "dense_search" => "dense",
        "lexical_search" => "lexical",
        other => other,
    }
}

/// `allowed` empty ⇒ no gate (tests / legacy). Non-empty ⇒ only listed methods.
pub fn method_allowed(allowed: &HashSet<String>, method: &str) -> bool {
    if allowed.is_empty() {
        return true;
    }
    let canon = canonicalize_sdk_method(method);
    allowed.contains(canon)
}

/// Error payload for denied RPC (Python shim raises RuntimeError with message).
pub fn capability_denied_error(method: &str) -> serde_json::Value {
    let canon = canonicalize_sdk_method(method);
    serde_json::json!({
        "error": {
            "code": "capability_denied",
            "message": format!(
                "SDK method `{canon}` is not enabled for this capability mode"
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
        assert!(!p.contains(&"web"));
        assert!(!p.contains(&"fetch"));
    }

    #[test]
    fn search_has_web_and_dense() {
        let p = sdk_primitives_for_caps(false, true);
        assert!(p.contains(&"web"));
        assert!(p.contains(&"fetch"));
        assert!(p.contains(&"dense"));
        assert!(!p.contains(&"grep"));
    }

    #[test]
    fn dual_is_union() {
        let p = sdk_primitives_for_caps(true, true);
        assert!(p.contains(&"grep"));
        assert!(p.contains(&"web"));
        assert!(p.contains(&"dense"));
    }

    #[test]
    fn empty_allowlist_is_open() {
        let set = HashSet::new();
        assert!(method_allowed(&set, "dense"));
        assert!(method_allowed(&set, "web"));
    }

    #[test]
    fn legacy_alias_maps_to_dense() {
        let set: HashSet<String> = ["dense".into()].into_iter().collect();
        assert!(method_allowed(&set, "dense_search"));
        assert!(!method_allowed(&set, "lexical_search"));
    }
}
