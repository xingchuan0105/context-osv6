use super::support::*;

#[tokio::test]
async fn get_workspace_returns_none_for_other_org_when_database_available() {
    let Some(database_url) = env::var("DATABASE_URL").ok() else {
        return;
    };
    let __bootstrap = BootstrapRepository::connect(&database_url).await.unwrap();
    __bootstrap.migrate().await.unwrap();
    let repo = PgAppRepository { pool: __bootstrap.pool.clone() };
    repo.bootstrap().migrate().await.unwrap();

    let org_a = UserId::from(Uuid::new_v4());
    let org_b = UserId::from(Uuid::new_v4());
    let ctx_a = AuthContext::new(org_a, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));
    let ctx_b = AuthContext::new(org_b, contracts::auth_runtime::SubjectKind::User)
        .with_actor_id(ActorId::new(Uuid::new_v4()));

    let notebook = repo
        .bootstrap().create_workspace(&ctx_a, "org-a notebook", "isolation test")
        .await
        .unwrap();
    let workspace_id = Uuid::parse_str(&notebook.id).unwrap();

    let fetched = repo.bootstrap().get_workspace(&ctx_b, workspace_id).await.unwrap();
    assert!(
        fetched.is_none(),
        "org B must not read org A's notebook via get_workspace"
    );
}

