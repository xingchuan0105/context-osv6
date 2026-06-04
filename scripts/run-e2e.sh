#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "═══════════════════════════════════════════════════"
echo "  Context-OS E2E 本地运行脚本"
echo "═══════════════════════════════════════════════════"

# ── 检查必需服务 ──────────────────────────────────
check_service() {
  local name=$1 url=$2
  if curl -fsS "$url" >/dev/null 2>&1; then
    echo -e "${GREEN}✓${NC} $name"
    return 0
  else
    echo -e "${RED}✗${NC} $name (未运行)"
    return 1
  fi
}

missing=0
check_service "PostgreSQL (5432)"  "http://127.0.0.1:5432"  || missing=1
check_service "Redis (6379)"       "http://127.0.0.1:6379"  || missing=1
check_service "Milvus (19530)"     "http://127.0.0.1:19530/healthz" || missing=1
check_service "MinIO (9000)"       "http://127.0.0.1:9000/minio/health/live" || missing=1
check_service "avrag-api (8080)"   "http://127.0.0.1:8080/health" || missing=1

if [[ $missing -eq 1 ]]; then
  echo ""
  echo -e "${YELLOW}⚠ 部分服务未启动，请参考以下命令：${NC}"
  echo ""
  echo "  # 启动数据库等基础设施"
  echo "  cd avrag-rs && docker compose -f docker-compose.milvus.yml up -d"
  echo ""
  echo "  # 启动 MinIO (如 milvus compose 中未包含)"
  echo "  docker run -d --name minio -p 9000:9000 -e MINIO_ROOT_USER=minioadmin -e MINIO_ROOT_SECRET=minioadmin minio/minio server /data"
  echo ""
  echo "  # 启动 avrag-api"
  echo "  cd avrag-rs && cargo run --release --bin avrag-api"
  echo ""
  echo "  # 启动 avrag-worker (负责 ingestion)"
  echo "  cd avrag-rs && cargo run --release --bin avrag-worker"
  echo ""
  echo "服务全部就绪后重新运行本脚本。"
  exit 1
fi

# ── 运行测试 ──────────────────────────────────────
echo ""
echo "运行 E2E 测试..."
cd frontend_next

# 默认跑 auth + functional，可传参数覆盖
projects="${1:---project=auth --project=functional}"

npx playwright test $projects --reporter=list

echo ""
echo -e "${GREEN}✓ E2E 测试完成${NC}"
