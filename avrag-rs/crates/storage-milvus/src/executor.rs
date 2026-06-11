use crate::lib_impl::MilvusDataPlane;
use crate::types::Result;
use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait WriteExecutor: Send + Sync {
    async fn insert(&self, collection: &str, rows: Vec<Value>) -> Result<()>;
    async fn delete(&self, collection: &str, filter: String) -> Result<()>;
}

pub struct RealExecutor<'a> {
    pub plane: &'a MilvusDataPlane,
}

#[async_trait]
impl WriteExecutor for RealExecutor<'_> {
    async fn insert(&self, collection: &str, rows: Vec<Value>) -> Result<()> {
        self.plane.insert_entities(collection, rows).await
    }
    async fn delete(&self, collection: &str, filter: String) -> Result<()> {
        self.plane.delete_by_filter(collection, filter).await
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::MilvusStorageError;
    use std::sync::Mutex;

    #[derive(Debug, Clone)]
    pub enum Call {
        Insert {
            collection: String,
            row_count: usize,
        },
        Delete {
            collection: String,
            filter: String,
        },
    }

    pub struct FakeExecutor {
        pub calls: Mutex<Vec<Call>>,
        pub fail_insert_on: Mutex<Option<String>>,
        pub fail_delete_on: Mutex<Option<String>>,
    }

    impl FakeExecutor {
        pub fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                fail_insert_on: Mutex::new(None),
                fail_delete_on: Mutex::new(None),
            }
        }

        pub fn with_insert_failure(collection: &str) -> Self {
            let ex = Self::new();
            *ex.fail_insert_on.lock().unwrap() = Some(collection.to_string());
            ex
        }

        pub fn calls(&self) -> Vec<Call> {
            self.calls.lock().unwrap().clone()
        }

        pub fn insert_calls(&self) -> Vec<(String, usize)> {
            self.calls()
                .into_iter()
                .filter_map(|c| match c {
                    Call::Insert {
                        collection,
                        row_count,
                    } => Some((collection, row_count)),
                    _ => None,
                })
                .collect()
        }

        pub fn delete_calls(&self) -> Vec<(String, String)> {
            self.calls()
                .into_iter()
                .filter_map(|c| match c {
                    Call::Delete { collection, filter } => Some((collection, filter)),
                    _ => None,
                })
                .collect()
        }
    }

    #[async_trait]
    impl WriteExecutor for FakeExecutor {
        async fn insert(&self, collection: &str, rows: Vec<Value>) -> Result<()> {
            self.calls.lock().unwrap().push(Call::Insert {
                collection: collection.to_string(),
                row_count: rows.len(),
            });
            if let Some(ref fail_on) = *self.fail_insert_on.lock().unwrap()
                && fail_on == collection
            {
                return Err(MilvusStorageError::Backend {
                    message: format!("injected insert failure on {}", collection),
                });
            }
            Ok(())
        }

        async fn delete(&self, collection: &str, filter: String) -> Result<()> {
            self.calls.lock().unwrap().push(Call::Delete {
                collection: collection.to_string(),
                filter: filter.clone(),
            });
            if let Some(ref fail_on) = *self.fail_delete_on.lock().unwrap()
                && fail_on == collection
            {
                return Err(MilvusStorageError::Backend {
                    message: format!("injected delete failure on {}", collection),
                });
            }
            Ok(())
        }
    }
}
