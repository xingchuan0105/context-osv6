# P4 分享与计费

**日期：** 2026-08-11  
**状态：** 钱包扣费全链路 + 公开分享只读 + ETag 已通。

## 计费

| 表 | 用途 |
|----|------|
| `osv7_wallets` | user_id(text) → balance_fen |
| `osv7_wallet_ledger` | topup / usage_debit，幂等键 |
| `osv7_byok` | embedding\|chat\|… BYOK 开关 |

| 单价（smoke） | fen |
|---------------|-----|
| chat 每轮 | 10 |
| embed 每 chunk | 1 |
| search | 1（预留） |

- **余额地板**：chat/ingest 前 `EnsureFloor`；不足 → `402` + `balance_insufficient`  
- **BYOK**：对应 capability 不扣平台余额  
- **包纪律**：扣款只经 `billing.WalletService`

## 分享

| 表 | 用途 |
|----|------|
| `osv7_share_links` | token → workspace 只读链接 |

| API | 说明 |
|-----|------|
| `POST /v1/share` | 创建 token（无 LLM） |
| `GET /public/s/{token}` | 公开视图：chunk/doc 计数、样例 snippet、可选会话气泡 |
| ETag / If-None-Match | 内容指纹；**不**含 access_count |

## 单体入口

`cmd/osv7d` 挂载 chat / sessions / billing / share。

## 冒烟

```bash
bash scripts/p4-share-billing-smoke.sh
```

通过项：topup 100 → chat 扣至 90 → 0 余额 402 → BYOK chat 200 → ingest 扣 2 至 88 → public share + **HTTP 304**.

## 未做

- 对接 v6 `wallets`/`share_tokens` 生产表（FK users）  
- 压测 QPS 基线（share 水平复制未跑）  
- 真·支付充值  
- 前端 pricing/topup 接 osv7d