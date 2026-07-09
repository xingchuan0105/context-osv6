# Milvus WSL Manual Runbook

## 适用场景

当 WSL2 下 `docker compose -f docker-compose.milvus.yml up -d` 出现异常（例如 `SIGBUS` / compose 进程异常退出）时，可改用手动 `docker run` 启动 Milvus 依赖栈。

## 一次性启动（手动容器）

```bash
docker network create avrag-milvus-net || true

docker run -d --rm \
  --name avrag-milvus-etcd \
  --network avrag-milvus-net \
  -e ETCD_AUTO_COMPACTION_MODE=revision \
  -e ETCD_AUTO_COMPACTION_RETENTION=1000 \
  -e ETCD_QUOTA_BACKEND_BYTES=4294967296 \
  -e ETCD_SNAPSHOT_COUNT=50000 \
  quay.io/coreos/etcd:v3.5.5 \
  etcd -advertise-client-urls=http://0.0.0.0:2379 \
       -listen-client-urls=http://0.0.0.0:2379 \
       --data-dir=/etcd

docker run -d --rm \
  --name avrag-milvus-minio \
  --network avrag-milvus-net \
  -e MINIO_ACCESS_KEY=minioadmin \
  -e MINIO_SECRET_KEY=minioadmin \
  minio/minio:RELEASE.2023-03-20T20-16-18Z \
  server /minio_data

docker run -d --rm \
  --name avrag-milvus-standalone \
  --network avrag-milvus-net \
  -p 19530:19530 \
  -p 19091:9091 \
  -e ETCD_ENDPOINTS=avrag-milvus-etcd:2379 \
  -e MINIO_ADDRESS=avrag-milvus-minio:9000 \
  -e MINIO_ACCESS_KEY=minioadmin \
  -e MINIO_SECRET_KEY=minioadmin \
  milvusdb/milvus:v2.4.6 \
  milvus run standalone
```

## 健康检查

```bash
curl -s -X POST http://127.0.0.1:19530/v2/vectordb/collections/list \
  -H 'Content-Type: application/json' \
  -d '{"dbName":"default"}'
```

返回 HTTP 200 且有 JSON 响应即视为可用。

## 停止与清理

```bash
docker stop avrag-milvus-standalone avrag-milvus-minio avrag-milvus-etcd
docker network rm avrag-milvus-net
```

## 备注

- 使用手动模式时，`MILVUS_URL` 仍保持 `http://127.0.0.1:19530`。
- 宿主机 `19091` 映射 Milvus metrics，避免与 `pdf-renderer` 默认 `9091` 冲突。
