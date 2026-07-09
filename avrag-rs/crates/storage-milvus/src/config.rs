use crate::types::MilvusStorageError;

/// 租户上下文，强制在所有数据访问点传播
#[derive(Debug, Clone)]
pub struct TenantContext {
    pub org_id: String,
    pub user_id: String,
    pub workspace_id: Option<String>,
    pub doc_scope: Vec<String>,
}

impl TenantContext {
    /// 构建 Milvus 过滤条件（强制注入租户 ID）
    pub fn build_milvus_filter(&self, base_filter: Option<&str>) -> String {
        let tenant_filter = format!("org_id == '{}'", self.org_id);
        match base_filter {
            Some(base) if !base.is_empty() => format!("({}) && ({})", tenant_filter, base),
            _ => tenant_filter,
        }
    }

    /// 验证数据访问权限
    pub fn verify_access(&self, target_org_id: &str) -> Result<(), MilvusStorageError> {
        if self.org_id != target_org_id {
            return Err(MilvusStorageError::TenantAccessDenied {
                message: "cross_tenant_access_denied".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MilvusConfig {
    pub url: String,
    pub token: Option<String>,
    pub database: Option<String>,
    pub collection_prefix: String,
    pub text_vector_dim: usize,
    pub multimodal_vector_dim: usize,
    pub metric_type: String,
}

impl Default for MilvusConfig {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:19530".to_string(),
            token: None,
            database: Some("default".to_string()),
            collection_prefix: "avrag".to_string(),
            text_vector_dim: 1024,
            multimodal_vector_dim: 1024,
            metric_type: "COSINE".to_string(),
        }
    }
}

impl MilvusConfig {
    pub fn collection_names(&self) -> MilvusCollectionNames {
        let prefix = self.collection_prefix.trim().trim_end_matches('_');
        let prefix = if prefix.is_empty() { "avrag" } else { prefix };
        MilvusCollectionNames {
            text_chunks: format!("{prefix}_rag_text_chunks"),
            multimodal_chunks: format!("{prefix}_rag_multimodal_chunks"),
            kg_entities: format!("{prefix}_rag_kg_entities"),
            kg_relations: format!("{prefix}_rag_kg_relations"),
            graph_passages: format!("{prefix}_rag_graph_passages"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilvusCollectionNames {
    pub text_chunks: String,
    pub multimodal_chunks: String,
    pub kg_entities: String,
    pub kg_relations: String,
    pub graph_passages: String,
}
