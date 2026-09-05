use web_sdk::{
    Capability, capabilities_to_wire, derive_agent_type_label, mode_line, normalize_capabilities,
    reconcile_session_rag, toggle_capability,
};

#[test]
fn normalize_dedupes_orders_and_drops_unknown() {
    assert_eq!(
        normalize_capabilities(["search", "rag", "rag", "write", "web"]),
        vec![Capability::Rag, Capability::Search]
    );
    assert!(normalize_capabilities(["chat", ""]).is_empty());
}

#[test]
fn toggle_and_agent_type_match_product_labels() {
    let none: Vec<Capability> = Vec::new();
    assert_eq!(derive_agent_type_label(&none), "chat");
    assert_eq!(capabilities_to_wire(&none), Vec::<String>::new());

    let search = toggle_capability(&none, Capability::Search);
    assert_eq!(search, vec![Capability::Search]);
    assert_eq!(derive_agent_type_label(&search), "search");

    let both = toggle_capability(&search, Capability::Rag);
    assert_eq!(both, vec![Capability::Rag, Capability::Search]);
    assert_eq!(derive_agent_type_label(&both), "rag+search");
    assert_eq!(
        capabilities_to_wire(&both),
        vec!["rag".to_string(), "search".to_string()]
    );

    let rag_only = toggle_capability(&both, Capability::Search);
    assert_eq!(rag_only, vec![Capability::Rag]);
    assert_eq!(derive_agent_type_label(&rag_only), "rag");
}

#[test]
fn reconcile_attaches_and_strips_only_when_not_manual() {
    let empty = Vec::new();
    assert_eq!(
        reconcile_session_rag(&empty, false, 1),
        vec![Capability::Rag]
    );
    assert_eq!(reconcile_session_rag(&empty, true, 1), empty);

    let rag = vec![Capability::Rag];
    assert!(reconcile_session_rag(&rag, false, 0).is_empty());
    assert_eq!(reconcile_session_rag(&rag, true, 0), rag);

    let search = vec![Capability::Search];
    assert_eq!(
        reconcile_session_rag(&search, false, 2),
        vec![Capability::Rag, Capability::Search]
    );
    assert_eq!(reconcile_session_rag(&search, true, 2), search);
}

#[test]
fn mode_line_covers_empty_search_and_both() {
    assert_eq!(
        mode_line(&[], false),
        "未添加会话文件：添加文件后可检索本会话。"
    );
    assert_eq!(mode_line(&[], true), "聊天：回答不检索文档与网络。");
    assert_eq!(
        mode_line(&[Capability::Search], false),
        "网络搜索：回答会检索网页。"
    );
    assert_eq!(
        mode_line(&[Capability::Rag], true),
        "知识库：回答检索已选文档。"
    );
    assert_eq!(
        mode_line(&[Capability::Rag, Capability::Search], true),
        "知识库 + 网络搜索：回答检索已选文档与网页。"
    );
}
