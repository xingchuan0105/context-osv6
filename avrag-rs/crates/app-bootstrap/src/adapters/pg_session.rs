//! Shared PostgreSQL session / RLS helpers for bootstrap adapters.
#![allow(dead_code)]

use common::AppError;
use sqlx::{PgConnection, PgPool, Postgres, Transaction};

pub(crate) fn db_err(error: sqlx::Error) -> AppError {
    AppError::internal(error.to_string())
}

pub(crate) async fn begin_tx<'a>(pool: &'a PgPool) -> Result<Transaction<'a, Postgres>, AppError> {
    pool.begin().await.map_err(db_err)
}

pub(crate) async fn begin_super_admin_tx<'a>(
    pool: &'a PgPool,
) -> Result<Transaction<'a, Postgres>, AppError> {
    begin_super_admin_tx_sqlx(pool).await.map_err(db_err)
}

pub(crate) async fn begin_super_admin_tx_sqlx<'a>(
    pool: &'a PgPool,
) -> Result<Transaction<'a, Postgres>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    set_config(&mut tx, "app.current_role", "super_admin").await?;
    Ok(tx)
}

pub(crate) async fn set_current_org(conn: &mut PgConnection, org_id: &str) -> Result<(), AppError> {
    set_config(conn, "app.current_org", org_id)
        .await
        .map_err(db_err)
}

pub(crate) async fn set_current_role(conn: &mut PgConnection, role: &str) -> Result<(), AppError> {
    set_config(conn, "app.current_role", role)
        .await
        .map_err(db_err)
}

pub(crate) async fn set_current_user(
    conn: &mut PgConnection,
    user_id: &str,
) -> Result<(), AppError> {
    set_config(conn, "app.current_user", user_id)
        .await
        .map_err(db_err)
}

pub(crate) async fn set_public_share_token(
    conn: &mut PgConnection,
    token: &str,
) -> Result<(), AppError> {
    set_config(conn, "app.public_share_token", token)
        .await
        .map_err(db_err)
}

pub(crate) struct RlsContext<'a> {
    pub org_id: Option<&'a str>,
    pub role: Option<&'a str>,
    pub user_id: Option<&'a str>,
    pub public_share_token: Option<&'a str>,
}

pub(crate) async fn set_rls_context(
    conn: &mut PgConnection,
    ctx: RlsContext<'_>,
) -> Result<(), AppError> {
    if let Some(org_id) = ctx.org_id {
        set_current_org(conn, org_id).await?;
    }
    if let Some(role) = ctx.role {
        set_current_role(conn, role).await?;
    }
    if let Some(user_id) = ctx.user_id {
        set_current_user(conn, user_id).await?;
    }
    if let Some(token) = ctx.public_share_token {
        set_public_share_token(conn, token).await?;
    }
    Ok(())
}

pub(crate) async fn set_config(
    conn: &mut PgConnection,
    key: &str,
    value: &str,
) -> Result<(), sqlx::Error> {
    let query = format!("select set_config('{key}', $1, true)");
    sqlx::query(&query).bind(value).execute(conn).await?;
    Ok(())
}

pub(crate) async fn set_config_sqlx(
    conn: &mut PgConnection,
    key: &str,
    value: &str,
) -> Result<(), sqlx::Error> {
    set_config(conn, key, value).await
}

pub(crate) async fn set_current_role_sqlx(
    conn: &mut PgConnection,
    role: &str,
) -> Result<(), sqlx::Error> {
    set_config(conn, "app.current_role", role).await
}

pub(crate) async fn set_current_user_sqlx(
    conn: &mut PgConnection,
    user_id: &str,
) -> Result<(), sqlx::Error> {
    set_config(conn, "app.current_user", user_id).await
}
