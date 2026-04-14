use anyhow::Result;
use sqlx::PgConnection;

pub(crate) async fn set_current_org(conn: &mut PgConnection, org_id: &str) -> Result<()> {
    sqlx::query("select set_config('app.current_org', $1, true)")
        .bind(org_id)
        .execute(conn)
        .await?;
    Ok(())
}

pub(crate) async fn set_current_role(conn: &mut PgConnection, role: &str) -> Result<()> {
    sqlx::query("select set_config('app.current_role', $1, true)")
        .bind(role)
        .execute(conn)
        .await?;
    Ok(())
}

pub(crate) async fn set_public_share_token(conn: &mut PgConnection, token: &str) -> Result<()> {
    sqlx::query("select set_config('app.public_share_token', $1, true)")
        .bind(token)
        .execute(conn)
        .await?;
    Ok(())
}
