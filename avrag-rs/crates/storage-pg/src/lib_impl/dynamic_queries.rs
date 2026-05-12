use sea_query::{Iden, PostgresQueryBuilder, Query};
use uuid::Uuid;

#[derive(Iden)]
pub enum Notebooks {
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
        .columns([Notebooks::Id, Notebooks::Title, Notebooks::UpdatedAt])
        .from(Notebooks::Table)
        .and_where(sea_query::Expr::col(Notebooks::OrgId).eq(org_id));

    if let Some(title) = title_filter {
        query.and_where(sea_query::Expr::col(Notebooks::Title).like(format!("%{}%", title)));
    }

    query.order_by(Notebooks::UpdatedAt, sea_query::Order::Desc);

    query.to_string(PostgresQueryBuilder)
}
