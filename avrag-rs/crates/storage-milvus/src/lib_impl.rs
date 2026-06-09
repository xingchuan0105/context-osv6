use crate::config::MilvusConfig;
use crate::types::{MilvusStorageError, Result};
use reqwest::Client;
use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub struct MilvusDataPlane {
    pub(crate) config: MilvusConfig,
    pub(crate) client: Client,
}

impl MilvusDataPlane {
    pub fn new(config: MilvusConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    pub fn config(&self) -> &MilvusConfig {
        &self.config
    }

    pub(crate) fn endpoint(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.config.url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    pub(crate) fn with_database(&self, mut body: Value) -> Value {
        if let Some(database) = self
            .config
            .database
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            body["dbName"] = json!(database);
        }
        body
    }

    pub(crate) async fn post_json(&self, path: &str, body: Value) -> Result<Value> {
        let mut request = self.client.post(self.endpoint(path)).json(&body);
        if let Some(token) = self
            .config
            .token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            request = request.bearer_auth(token);
        }

        let response = request.send().await?;
        let status = response.status();
        let body_text = response.text().await?;
        if !status.is_success() {
            return Err(MilvusStorageError::Backend {
                message: format!("Milvus request {path} failed with {status}: {body_text}"),
            });
        }

        let value = serde_json::from_str::<Value>(&body_text).unwrap_or_else(|_| json!({}));
        let code = value.get("code").and_then(Value::as_i64).unwrap_or(0);
        if code != 0 && code != 200 {
            return Err(MilvusStorageError::Backend {
                message: value
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or(body_text.as_str())
                    .to_string(),
            });
        }
        Ok(value)
    }

    pub(crate) async fn list_collections(&self) -> Result<Vec<String>> {
        let response = self
            .post_json(
                "/v2/vectordb/collections/list",
                self.with_database(json!({})),
            )
            .await?;
        Ok(collection_names_from_response(&response))
    }

    pub(crate) async fn insert_entities(&self, collection: &str, rows: Vec<Value>) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        self.post_json(
            "/v2/vectordb/entities/insert",
            self.with_database(json!({
                "collectionName": collection,
                "data": rows
            })),
        )
        .await?;
        Ok(())
    }

    pub(crate) async fn delete_by_filter(&self, collection: &str, filter: String) -> Result<()> {
        self.post_json(
            "/v2/vectordb/entities/delete",
            self.with_database(json!({
                "collectionName": collection,
                "filter": filter
            })),
        )
        .await?;
        Ok(())
    }
}

pub(crate) fn collection_names_from_response(response: &Value) -> Vec<String> {
    response["data"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}
