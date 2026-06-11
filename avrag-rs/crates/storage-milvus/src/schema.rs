use crate::config::MilvusConfig;
use avrag_auth::AuthContext;
use serde_json::{Value, json};
use uuid::Uuid;

pub const TEXT_OUTPUT_FIELDS: [&str; 11] = [
    "chunk_id",
    "doc_id",
    "parse_run_id",
    "page",
    "text",
    "chunk_type",
    "parser_backend",
    "source_locator",
    "doc_version",
    "org_id",
    "workspace_id",
];

pub const MULTIMODAL_OUTPUT_FIELDS: [&str; 15] = [
    "chunk_id",
    "doc_id",
    "asset_id",
    "parse_run_id",
    "page",
    "context_text",
    "caption",
    "image_path",
    "chunk_type",
    "parser_backend",
    "retrieval_weight",
    "source_locator",
    "doc_version",
    "org_id",
    "workspace_id",
];

pub const RELATION_OUTPUT_FIELDS: [&str; 13] = [
    "relation_id",
    "doc_id",
    "parse_run_id",
    "subject",
    "predicate",
    "object",
    "relation_text",
    "supporting_chunk_ids",
    "metadata",
    "doc_version",
    "org_id",
    "workspace_id",
    "id",
];

pub fn schema_text(config: &MilvusConfig) -> (Value, Vec<Value>) {
    let schema = collection_schema(
        vec![
            varchar_field("id", 128, true, false, false),
            varchar_field("org_id", 64, false, false, false),
            varchar_field("workspace_id", 64, false, true, false),
            varchar_field("doc_id", 64, false, false, false),
            varchar_field("chunk_id", 64, false, false, false),
            varchar_field("parse_run_id", 64, false, false, false),
            int64_field("doc_version", false),
            int64_field("page", true),
            varchar_field("text", 65_535, false, false, true),
            float_vector_field("text_dense", config.text_vector_dim),
            sparse_vector_field("text_sparse"),
            varchar_field("chunk_type", 64, false, false, false),
            varchar_field("parser_backend", 64, false, true, false),
            json_field("source_locator", true),
        ],
        vec![json!({
            "name": "text_bm25",
            "type": "BM25",
            "inputFieldNames": ["text"],
            "outputFieldNames": ["text_sparse"],
            "params": {}
        })],
    );
    let indexes = vec![
        dense_index("text_dense", "text_dense_idx", &config.metric_type),
        bm25_index("text_sparse", "text_sparse_idx"),
    ];
    (schema, indexes)
}

pub fn schema_multimodal(config: &MilvusConfig) -> (Value, Vec<Value>) {
    let schema = collection_schema(
        vec![
            varchar_field("id", 128, true, false, false),
            varchar_field("org_id", 64, false, false, false),
            varchar_field("workspace_id", 64, false, true, false),
            varchar_field("doc_id", 64, false, false, false),
            varchar_field("chunk_id", 64, false, false, false),
            varchar_field("asset_id", 64, false, false, false),
            varchar_field("parse_run_id", 64, false, false, false),
            int64_field("doc_version", false),
            int64_field("page", true),
            varchar_field("context_text", 65_535, false, false, true),
            varchar_field("caption", 4_096, false, true, true),
            varchar_field("image_path", 2_048, false, true, false),
            float_vector_field("multimodal_dense", config.multimodal_vector_dim),
            varchar_field("chunk_type", 64, false, false, false),
            varchar_field("parser_backend", 64, false, true, false),
            float_field("retrieval_weight", true),
            json_field("source_locator", true),
        ],
        Vec::new(),
    );
    let indexes = vec![dense_index(
        "multimodal_dense",
        "multimodal_dense_idx",
        &config.metric_type,
    )];
    (schema, indexes)
}

pub fn schema_entities(config: &MilvusConfig) -> (Value, Vec<Value>) {
    let schema = collection_schema(
        vec![
            varchar_field("id", 128, true, false, false),
            varchar_field("org_id", 64, false, false, false),
            varchar_field("workspace_id", 64, false, true, false),
            varchar_field("doc_id", 64, false, false, false),
            varchar_field("entity_id", 64, false, false, false),
            varchar_field("parse_run_id", 64, false, false, false),
            int64_field("doc_version", false),
            varchar_field("name", 512, false, false, true),
            varchar_field("normalized_name", 512, false, false, true),
            varchar_field("entity_type", 128, false, true, false),
            float_vector_field("entity_dense", config.text_vector_dim),
            json_field("supporting_chunk_ids", false),
            json_field("metadata", true),
        ],
        Vec::new(),
    );
    let indexes = vec![dense_index(
        "entity_dense",
        "entity_dense_idx",
        &config.metric_type,
    )];
    (schema, indexes)
}

pub fn schema_relations(config: &MilvusConfig) -> (Value, Vec<Value>) {
    let schema = collection_schema(
        vec![
            varchar_field("id", 128, true, false, false),
            varchar_field("org_id", 64, false, false, false),
            varchar_field("workspace_id", 64, false, true, false),
            varchar_field("doc_id", 64, false, false, false),
            varchar_field("relation_id", 64, false, false, false),
            varchar_field("parse_run_id", 64, false, false, false),
            int64_field("doc_version", false),
            varchar_field("subject", 512, false, false, true),
            varchar_field("predicate", 256, false, false, true),
            varchar_field("object", 512, false, false, true),
            varchar_field("relation_text", 2_048, false, false, true),
            float_vector_field("relation_dense", config.text_vector_dim),
            json_field("supporting_chunk_ids", false),
            json_field("metadata", true),
        ],
        Vec::new(),
    );
    let indexes = vec![dense_index(
        "relation_dense",
        "relation_dense_idx",
        &config.metric_type,
    )];
    (schema, indexes)
}

pub fn schema_graph_passages(config: &MilvusConfig) -> (Value, Vec<Value>) {
    let schema = collection_schema(
        vec![
            varchar_field("id", 128, true, false, false),
            varchar_field("org_id", 64, false, false, false),
            varchar_field("workspace_id", 64, false, true, false),
            varchar_field("doc_id", 64, false, false, false),
            varchar_field("chunk_id", 64, false, true, false),
            varchar_field("passage_id", 64, false, false, false),
            varchar_field("parse_run_id", 64, false, false, false),
            int64_field("doc_version", false),
            varchar_field("text", 65_535, false, false, true),
            float_vector_field("passage_dense", config.text_vector_dim),
            json_field("relation_ids", false),
            json_field("metadata", true),
        ],
        Vec::new(),
    );
    let indexes = vec![dense_index(
        "passage_dense",
        "passage_dense_idx",
        &config.metric_type,
    )];
    (schema, indexes)
}

pub fn collection_schema(fields: Vec<Value>, functions: Vec<Value>) -> Value {
    let mut schema = json!({
        "autoID": false,
        "enableDynamicField": false,
        "fields": fields,
    });
    if !functions.is_empty() {
        schema["functions"] = Value::Array(functions);
    }
    schema
}

pub fn varchar_field(
    name: &str,
    max_length: usize,
    is_primary: bool,
    nullable: bool,
    enable_analyzer: bool,
) -> Value {
    let mut field = json!({
        "fieldName": name,
        "dataType": "VarChar",
        "elementTypeParams": {
            "max_length": max_length,
        }
    });
    if is_primary {
        field["isPrimary"] = json!(true);
    }
    if nullable {
        field["nullable"] = json!(true);
    }
    if enable_analyzer {
        field["elementTypeParams"]["enable_analyzer"] = json!(true);
    }
    field
}

pub fn int64_field(name: &str, nullable: bool) -> Value {
    let mut field = json!({
        "fieldName": name,
        "dataType": "Int64",
    });
    if nullable {
        field["nullable"] = json!(true);
    }
    field
}

pub fn float_field(name: &str, nullable: bool) -> Value {
    let mut field = json!({
        "fieldName": name,
        "dataType": "Float",
    });
    if nullable {
        field["nullable"] = json!(true);
    }
    field
}

pub fn float_vector_field(name: &str, dim: usize) -> Value {
    json!({
        "fieldName": name,
        "dataType": "FloatVector",
        "elementTypeParams": {
            "dim": dim,
        }
    })
}

pub fn sparse_vector_field(name: &str) -> Value {
    json!({
        "fieldName": name,
        "dataType": "SparseFloatVector",
    })
}

pub fn json_field(name: &str, nullable: bool) -> Value {
    let mut field = json!({
        "fieldName": name,
        "dataType": "JSON",
    });
    if nullable {
        field["nullable"] = json!(true);
    }
    field
}

pub fn dense_index(field_name: &str, index_name: &str, metric_type: &str) -> Value {
    json!({
        "fieldName": field_name,
        "indexName": index_name,
        "metricType": metric_type,
        "params": {
            "index_type": "AUTOINDEX"
        }
    })
}

pub fn bm25_index(field_name: &str, index_name: &str) -> Value {
    json!({
        "fieldName": field_name,
        "indexName": index_name,
        "metricType": "BM25",
        "params": {
            "index_type": "SPARSE_INVERTED_INDEX",
            "inverted_index_algo": "DAAT_MAXSCORE",
            "bm25_k1": 1.2,
            "bm25_b": 0.75
        }
    })
}

pub fn doc_filter(auth: &AuthContext, doc_ids: Option<&[Uuid]>) -> String {
    let mut filter = format!("org_id == {}", milvus_string(&auth.org_id().to_string()));
    if let Some(doc_ids) = doc_ids {
        if doc_ids.is_empty() {
            return "doc_id == 'none'".to_string(); // Block everything if empty
        }
        let docs = doc_ids
            .iter()
            .map(|doc_id| milvus_string(&doc_id.to_string()))
            .collect::<Vec<_>>()
            .join(", ");
        filter.push_str(&format!(" and doc_id in [{docs}]"));
    }
    filter
}

pub fn milvus_string(s: &str) -> String {
    format!("'{}'", s.replace('\'', "\\'"))
}

pub fn validate_existing_collection_schema(
    collection_name: &str,
    expected_schema: &Value,
    describe_response: &Value,
) -> crate::types::Result<()> {
    let expected_fields = schema_fields(expected_schema).ok_or_else(|| {
        crate::types::MilvusStorageError::Backend {
            message: format!(
                "collection {collection_name} expected schema has no fields; cannot validate"
            ),
        }
    })?;

    let actual_fields =
        describe_schema_fields(describe_response).ok_or_else(|| crate::types::MilvusStorageError::Backend {
            message: format!(
                "collection {collection_name} describe response missing schema fields; cannot validate compatibility"
            ),
        })?;

    let actual_specs = actual_fields
        .iter()
        .filter_map(|field| {
            let name = field_name(field)?;
            Some((name.to_string(), field_spec(field)))
        })
        .collect::<std::collections::HashMap<_, _>>();

    for expected_field in expected_fields {
        let Some(expected_name) = field_name(expected_field) else {
            continue;
        };
        let expected_spec = field_spec(expected_field);
        let Some(actual_spec) = actual_specs.get(expected_name) else {
            return Err(crate::types::MilvusStorageError::Backend {
                message: format!(
                    "collection {collection_name} is incompatible: missing expected field `{expected_name}`"
                ),
            });
        };

        if let Some(expected_type) = expected_spec.data_type.as_deref() {
            let Some(actual_type) = actual_spec.data_type.as_deref() else {
                return Err(crate::types::MilvusStorageError::Backend {
                    message: format!(
                        "collection {collection_name} field `{expected_name}` missing field type in describe response"
                    ),
                });
            };
            if !expected_type.eq_ignore_ascii_case(actual_type) {
                return Err(crate::types::MilvusStorageError::Backend {
                    message: format!(
                        "collection {collection_name} field `{expected_name}` has type `{actual_type}`, expected `{expected_type}`"
                    ),
                });
            }
        }

        if let Some(expected_dim) = expected_spec.vector_dim {
            match actual_spec.vector_dim {
                Some(actual_dim) if actual_dim == expected_dim => {}
                Some(actual_dim) => {
                    return Err(crate::types::MilvusStorageError::Backend {
                        message: format!(
                            "collection {collection_name} field `{expected_name}` dim mismatch: expected {expected_dim}, got {actual_dim}"
                        ),
                    });
                }
                None => {
                    return Err(crate::types::MilvusStorageError::Backend {
                        message: format!(
                            "collection {collection_name} field `{expected_name}` missing vector dim in describe response"
                        ),
                    });
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct FieldSpec {
    data_type: Option<String>,
    vector_dim: Option<usize>,
}

fn field_spec(field: &Value) -> FieldSpec {
    FieldSpec {
        data_type: field_data_type(field),
        vector_dim: field_dim(field),
    }
}

pub fn schema_fields(schema: &Value) -> Option<&Vec<Value>> {
    schema["fields"].as_array()
}

pub fn describe_schema_fields(response: &Value) -> Option<&Vec<Value>> {
    // Milvus v2.4: data.schema.fields
    // Milvus v2.6+: data.fields
    response["data"]["schema"]["fields"]
        .as_array()
        .or_else(|| response["data"]["fields"].as_array())
}

pub fn field_name(field: &Value) -> Option<&str> {
    // Milvus v2.4: fieldName
    // Milvus v2.6+: name
    field["fieldName"]
        .as_str()
        .or_else(|| field["name"].as_str())
}

pub fn field_data_type(field: &Value) -> Option<String> {
    // Milvus v2.4: dataType
    // Milvus v2.6+: type
    field["dataType"]
        .as_str()
        .or_else(|| field["type"].as_str())
        .map(|s| s.to_string())
}

pub fn field_dim(field: &Value) -> Option<usize> {
    // Milvus v2.4: elementTypeParams.dim (object)
    // Milvus v2.6+: params array [{"key":"dim","value":"1024"}]
    field["elementTypeParams"]["dim"]
        .as_i64()
        .or_else(|| field["elementTypeParams"]["max_length"].as_i64())
        .or_else(|| {
            field["params"].as_array().and_then(|arr| {
                arr.iter().find_map(|p| {
                    if p["key"].as_str()? == "dim" || p["key"].as_str()? == "max_length" {
                        p["value"].as_str()?.parse::<i64>().ok()
                    } else {
                        None
                    }
                })
            })
        })
        .map(|v| v as usize)
}

pub fn collection_names_from_response(response: &Value) -> Vec<String> {
    response["data"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}
