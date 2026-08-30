//! Role-guard live tests: a SUPERUSER or BYPASSRLS login must be refused by
//! `BootstrapRepository::connect` (fail closed, no env escape). Provisioning
//! throwaway roles requires CREATEROLE, so the admin pool comes from
//! `AVRAG_GUARD_ADMIN_URL` (a local admin DSN, e.g. a superuser); the connect
//! target under test is DATABASE_URL itself.
use super::support::*;

fn dsn_credentials(database_url: &str) -> String {
    let after_scheme = database_url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(database_url);
    after_scheme
        .split('@')
        .next()
        .unwrap_or_default()
        .to_string()
}

async fn admin_pool() -> Option<PgPool> {
    let Ok(database_url) = env::var("AVRAG_GUARD_ADMIN_URL") else {
        return None;
    };
    if database_url.trim().is_empty() {
        return None;
    }
    Some(
        PgPoolOptions::new()
            .max_connections(2)
            .connect(&database_url)
            .await
            .unwrap(),
    )
}

#[tokio::test]
async fn superuser_role_is_refused_at_connect() {
    let Some(admin) = admin_pool().await else {
        return;
    };
    let suffix = &Uuid::new_v4().simple().to_string()[..12];
    let role = format!("avrag_guard_su_{suffix}");
    sqlx::query(&format!(
        "CREATE ROLE \"{role}\" LOGIN SUPERUSER PASSWORD 'x'"
    ))
    .execute(&admin)
    .await
    .unwrap();

    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://avrag:avrag@127.0.0.1:5432/avrag_rs".to_string()
    });
    let role_url = database_url.replacen(&dsn_credentials(&database_url), &format!("{role}:x"), 1);
    let err = BootstrapRepository::connect(&role_url).await.err();

    let _ = sqlx::query(&format!("DROP ROLE IF EXISTS \"{role}\""))
        .execute(&admin)
        .await;
    assert!(
        err.is_some(),
        "SUPERUSER runtime role must fail the startup guard"
    );
}

#[tokio::test]
async fn bypassrls_role_is_refused_at_connect() {
    let Some(admin) = admin_pool().await else {
        return;
    };
    let suffix = &Uuid::new_v4().simple().to_string()[..12];
    let role = format!("avrag_guard_byp_{suffix}");
    sqlx::query(&format!(
        "CREATE ROLE \"{role}\" LOGIN NOSUPERUSER BYPASSRLS PASSWORD 'x'"
    ))
    .execute(&admin)
    .await
    .unwrap();

    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://avrag:avrag@127.0.0.1:5432/avrag_rs".to_string()
    });
    let role_url = database_url.replacen(&dsn_credentials(&database_url), &format!("{role}:x"), 1);
    let err = BootstrapRepository::connect(&role_url).await.err();

    let _ = sqlx::query(&format!("DROP ROLE IF EXISTS \"{role}\""))
        .execute(&admin)
        .await;
    assert!(
        err.is_some(),
        "BYPASSRLS runtime role must fail the startup guard"
    );
}