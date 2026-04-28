# Local Dev Runbook

## 启动 API

```bash
cargo run -p avrag-api
```

正式前端位于 `../frontend_next`：

```bash
cd ../frontend_next
pnpm install
pnpm typecheck
pnpm dev
```

`../frontend_rust` 不再由 `avrag-api` 服务，仅保留为历史工程。

推荐先启动本地依赖：

```bash
./scripts/dev-services-up.sh
```

该脚本启动 PostgreSQL、Redis 和 MinIO；Milvus 需要单独启动并通过 `MILVUS_URL` 指向。

可选环境变量：

- `AVRAG_API_ADDR`：监听地址，默认 `0.0.0.0:8080`
- `AVRAG_PUBLIC_BASE_URL`：返回给前端的绝对上传地址，默认 `http://127.0.0.1:8080`
- `DATABASE_URL`：设置后启用 PostgreSQL-backed runtime
- `MILVUS_URL`：Milvus REST 地址，默认 `http://127.0.0.1:19530`
- `AVRAG_RUN_MIGRATIONS`：默认 `true`，在 PostgreSQL 模式下启动时自动执行迁移
- `RUST_LOG`：日志级别，默认 `info`

运行模式：

- 未设置 `DATABASE_URL`：内存 fallback，`/ready` 返回 `m1-m2-memory`
- 设置 `DATABASE_URL`：尝试连接 PostgreSQL 并执行迁移；默认会初始化 Milvus retrieval data plane，`/ready` 返回 `m1-m2-postgres`

## 手工验证建议

1. `GET /health`
2. `POST /api/v1/notebooks`
3. `POST /api/v1/notebooks/{id}/documents`
4. `PUT /dev-upload/{doc_id}`
5. `GET /api/v1/documents/{id}/status`
6. `POST /api/v1/chat` 或 `POST /api/v1/chat?stream=true`

## 说明

当前代码支持 PostgreSQL + Milvus 启动路径，但本地 smoke 仍可基于内存模式完成。
若你的 WSL 环境没有接通 Docker Desktop、本机 PostgreSQL 或 Milvus，请先使用内存模式联调前端协议。

关闭本地依赖：

```bash
./scripts/dev-services-down.sh
```

Worker skeleton 运行与任务契约说明见：

- `docs/runbooks/worker-dev.md`
