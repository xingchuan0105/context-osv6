//! Shared E2E bootstrap: Docker PG/Milvus/Redis and worker binary discovery.

pub use crate::product_e2e::setup::{
    SharedMilvus, SharedPostgres, acquire_shared_milvus, acquire_shared_postgres,
    create_temp_object_store, find_worker_binary, load_fixture, mime_type_for_filename,
    release_shared_milvus, release_shared_postgres, start_milvus, start_postgres, start_redis,
};
