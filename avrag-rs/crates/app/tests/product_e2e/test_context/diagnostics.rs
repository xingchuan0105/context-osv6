//! Ingestion failure diagnostics for product E2E.

use uuid::Uuid;

use super::TestContext;

pub(crate) async fn dump_ingestion_failure_diagnostics(ctx: &TestContext, doc_id: &str) {
    let parsed_doc_id = match Uuid::parse_str(doc_id) {
        Ok(id) => id,
        Err(error) => {
            eprintln!("[ingest_diagnostic] invalid document id `{doc_id}`: {error}");
            return;
        }
    };

    match sqlx::PgPool::connect(&ctx.pg_url).await {
        Ok(pool) => {
            match sqlx::query_as::<_, (String, Option<String>, Option<String>, i32)>(
                r#"
                select status, last_error, locked_by, attempt_count
                from ingestion_tasks
                where document_id = $1
                order by enqueued_at desc
                limit 1
                "#,
            )
            .bind(parsed_doc_id)
            .fetch_optional(&pool)
            .await
            {
                Ok(Some((status, last_error, locked_by, attempt_count))) => {
                    eprintln!(
                        "[ingest_diagnostic] doc={doc_id} status={status} attempt_count={attempt_count} locked_by={locked_by:?} last_error={last_error:?}"
                    );
                }
                Ok(None) => {
                    eprintln!("[ingest_diagnostic] doc={doc_id} ingestion_tasks row not found");
                }
                Err(error) => {
                    eprintln!("[ingest_diagnostic] query ingestion_tasks failed for doc={doc_id}: {error}");
                }
            }
        }
        Err(error) => {
            eprintln!("[ingest_diagnostic] connect pg failed for doc={doc_id}: {error}");
        }
    }

    if ctx.worker_log_path.is_some() {
        let tail = ctx.worker_log_tail(30);
        if tail.trim().is_empty() {
            eprintln!("[ingest_diagnostic] worker.log tail(30): <empty>");
        } else {
            eprintln!("[ingest_diagnostic] worker.log tail(30):\n{tail}");
        }
    } else {
        eprintln!("[ingest_diagnostic] worker log path unavailable");
    }
}
