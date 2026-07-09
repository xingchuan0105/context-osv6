use super::*;
use sea_query::{Iden, PostgresQueryBuilder, Query};

#[derive(Iden)]
#[iden = "workspaces"]
pub enum Workspaces {
    Table,
    Id,
    OrgId,
    Title,
    UpdatedAt,
}

pub fn build_notebook_search_query(
    org_id: Uuid,
    title_filter: Option<&str>,
) -> String {
    let mut query = Query::select();
    query
        .columns([Workspaces::Id, Workspaces::Title, Workspaces::UpdatedAt])
        .from(Workspaces::Table)
        .and_where(sea_query::Expr::col(Workspaces::OrgId).eq(org_id));

    if let Some(title) = title_filter {
        query.and_where(sea_query::Expr::col(Workspaces::Title).like(format!("%{}%", title)));
    }

    query.order_by(Workspaces::UpdatedAt, sea_query::Order::Desc);

    query.to_string(PostgresQueryBuilder)
}
