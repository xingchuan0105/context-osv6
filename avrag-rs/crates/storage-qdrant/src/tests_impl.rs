#[cfg(test)]
mod tests {
    use super::*;
    use avrag_auth::{ActorId, SubjectKind};

    fn test_context(org_id: &str) -> AuthContext {
        AuthContext::new(
            org_id.parse::<OrgId>().expect("valid org id"),
            SubjectKind::User,
        )
        .with_actor_id(ActorId::new(Uuid::new_v4()))
    }

    #[test]
    fn secure_filter_builder_injects_org_scope() {
        let context = test_context("00000000-0000-0000-0000-000000000001");
        let filter = SecureQdrantFilterBuilder::for_context(&context).expect("filter");

        assert_eq!(filter.must.len(), 1);
        assert_eq!(filter.must[0].key, "org_id");
        assert_eq!(filter.must[0].value, "00000000-0000-0000-0000-000000000001");
    }

    #[test]
    fn secure_filter_builder_can_append_doc_scope() {
        let context = test_context("00000000-0000-0000-0000-000000000001");
        let doc_id = Uuid::new_v4();
        let filter = SecureQdrantFilterBuilder::with_doc_filter(&context, doc_id).expect("filter");

        assert_eq!(filter.must.len(), 2);
        assert_eq!(filter.must[1].key, "doc_id");
        assert_eq!(filter.must[1].value, doc_id.to_string());
    }

    #[test]
    fn filter_json_matches_qdrant_shape() {
        let filter = QdrantFilter {
            must: vec![
                FieldMatch {
                    key: "org_id".to_string(),
                    value: "org-1".to_string(),
                },
                FieldMatch {
                    key: "doc_id".to_string(),
                    value: "doc-1".to_string(),
                },
            ],
        };

        let value = filter_to_json(&filter);
        assert_eq!(value["must"][0]["key"], "org_id");
        assert_eq!(value["must"][0]["match"]["value"], "org-1");
        assert_eq!(value["must"][1]["key"], "doc_id");
    }
}
