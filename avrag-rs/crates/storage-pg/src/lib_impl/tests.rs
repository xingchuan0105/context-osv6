#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn update_document_summary_overwrites_existing_summary_when_database_available() {
        let Some(database_url) = env::var("DATABASE_URL").ok() else {
            return;
        };
        let repo = PgAppRepository::connect(&database_url).await.unwrap();
        repo.migrate().await.unwrap();

        let org_id = OrgId::from(Uuid::new_v4());
        let user_id = Uuid::new_v4();
        let ctx = AuthContext::new(org_id, avrag_auth::SubjectKind::User)
            .with_actor_id(ActorId::new(user_id));

        let notebook = repo
            .create_notebook(&ctx, "summary test notebook", "summary test")
            .await
            .unwrap();
        let document = repo
            .create_document(
                &ctx,
                Uuid::parse_str(&notebook.id).unwrap(),
                "summary-test.txt",
                42,
                "text/plain",
            )
            .await
            .unwrap();
        let document_id = Uuid::parse_str(&document.id).unwrap();

        repo.store_document_body(&ctx, document_id, "First line. Second line. Third line.")
            .await
            .unwrap();
        let summary_output = common::SummaryOutput {
            summary_text: "LLM upgraded summary".to_string(),
            summary_metadata: common::SummaryMetadata {
                doc_id: document_id.to_string(),
                filename: "summary-test.txt".to_string(),
                docname: "summary test".to_string(),
                language: "en".to_string(),
                domain: "test".to_string(),
                genre: "test".to_string(),
                era: "contemporary".to_string(),
            },
        };
        repo.update_document_summary(&ctx, document_id, &summary_output)
            .await
            .unwrap();

        let preview = repo
            .get_parsed_preview(&ctx, document_id, 0, 10)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(preview.summary.as_deref(), Some("LLM upgraded summary"));
    }
}

