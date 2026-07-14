# 本地 ↔ VPS 对齐方案

**日期**: 2026-07-14  
**状态**: Plan + **P0/P1/P2-backend 已落地**（2026-07-14）  
- P0: `deploy-frontend.sh` / `deploy-status.sh`；frontend 发版写 `DEPLOYED.txt`  
- P1: `vps-pull-config.sh`；`deploy/nginx` + `deploy/systemd` 入库；AGENTS/SOLO 写明「发版只走 `scripts/deploy-*`」  
- P2-backend: `deploy-backend.sh` + `deploy/docker/run-avrag-containers.sh`（API/worker + migrations/prompts）  
- P2-public-sites: 仍待（landing/why/canju 统一入口）  

**问题**: 部分产物/配置直接写在 VPS；页面与脚本改在本地磁盘。两端无单一真相，易漂移。  
**原则**: **源码与可重建配置只认本地 trunk；VPS 只跑「发布物」与「运行时状态」。**

---

## 1. 现状诊断（为什么不对齐）

| 类型 | 本地 | VPS | 风险 |
|------|------|-----|------|
| 应用源码（frontend / avrag-rs / desktop） | `master` 大量未提交改动 | **无 git**；`/opt/avrag-rs/frontend` 是 standalone 拷贝 | 改本地 ≠ 线上；线上无法 diff 回仓库 |
| Desktop 安装包 | `dist/desktop-release/`（gitignored） | `/var/www/releases/desktop/` | 本地重打包才与线上一致；包本身不进 git 正确 |
| nginx | `deploy/nginx/*.snippet.conf` 片段 | `/etc/nginx/conf.d/*` 手改 | 片段未保证已与线上 conf 全文一致入库 |
| systemd unit | 无完整 unit 入库 | `/etc/systemd/system/avrag-*.service` 等 | 重建机器靠记忆 |
| 运行时 env | `avrag-rs/.env`（本地 dev） | `/etc/avrag-rs/avrag.env` | **故意不同**；禁止把 prod 密钥回写进 git |
| 其它站 | landing/why/cchess/theme 多仓库 | `/var/www/*`、`/opt/whyiamright` | 跨仓发布未统一入口 |

**结论**：  
- **对**的部分：二进制/安装包、standalone 前端应以「构建产物上传」方式上 VPS（不是在 VPS 上改源码）。  
- **错/乱**的部分：没有「从本地某一 commit 可复现线上」的发版清单；nginx/unit 手改未闭环回仓库。

---

## 2. 单一真相（Source of Truth）

```text
                    ┌─────────────────────────┐
                    │  本地 trunk (master)     │  ← 唯一源码真相
                    │  + 版本 tag（发版时）     │
                    └───────────┬─────────────┘
                                │  build / package
                                ▼
                    ┌─────────────────────────┐
                    │  发布物 (artifacts)      │
                    │  · avrag-api/worker     │
                    │  · frontend standalone  │
                    │  · desktop latest.json  │
                    │  · (可选) landing 静态   │
                    └───────────┬─────────────┘
                                │  publish scripts
                                ▼
                    ┌─────────────────────────┐
                    │  VPS runtime            │  ← 只消费发布物
                    │  /opt/avrag-rs/*        │
                    │  /var/www/releases/*    │
                    │  systemd + nginx        │
                    │  /etc/avrag-rs/*.env    │  ← 仅密钥/机独配置
                    └─────────────────────────┘
```

| 资产 | 真相所在 | VPS 角色 |
|------|----------|----------|
| 业务代码 | 本地 git | 只收编译/导出结果 |
| 发版脚本 | 本地 `scripts/*` | 由人/CI 在**本地执行**推到 VPS |
| nginx/systemd 模板 | 本地 `deploy/` | 同步后 `nginx -t && reload` |
| 生产密钥 | **仅 VPS** `/etc/avrag-rs/`（+ 本地加密备份可选） | 永不 `git add` |
| 用户数据 / PG / objects | VPS `/data` | 备份另案 |

---

## 3. 发布物矩阵（「推什么」）

| 组件 | 本地构建命令 | 产物路径 | VPS 落点 | 推荐脚本 |
|------|--------------|----------|----------|----------|
| API / Worker | `cargo build --release -p avrag-api -p avrag-worker`（本地 Ubuntu 24.04 / glibc 2.39） | `target/release/*` | `/opt/avrag-rs/bin` + `avrag-runtime:24.04` host 网络 | **`scripts/deploy-backend.sh`** |
| App 前端 | `cd frontend_next && pnpm build` | `.next/standalone` + static + public | `/opt/avrag-rs/frontend` + `avrag-frontend.service` | `scripts/deploy-frontend.sh` |
| Desktop 包 | package 脚本 | `dist/desktop-release/` | `/var/www/releases/desktop/` | **已有** `package-desktop-release.sh` + `publish-desktop-release.sh` |
| Migrations / prompts | 随后端包 | 仓库目录 | `/opt/avrag-rs/migrations` `prompts` | 后端 deploy 一并 rsync |
| Landing / Canju / Ghost / Why | 各仓 build | 静态或 standalone | `/var/www/*` `/opt/whyiamright` | 各 `publish-*.sh` 或统一 `deploy-public-sites.sh` |

**禁止**：SSH 上 VPS 直接改 React/TS/业务逻辑再「凑合用」。

**允许**：SSH 查日志、临时 `nginx -t`、只读排查；**持久修改必须回本地再发布**。

---

## 4. 对齐工作流（日常）

### 4.1 开发（本地）

```text
1. 改代码（frontend / avrag-rs / desktop / docs）
2. 本地验证：pnpm test / cargo test / 手动
3. git commit（本地 trunk；按需 push backup 分支，注意 [skip ci]）
4. 不直接改 VPS
```

### 4.2 发版（本地 → VPS）

```text
1. 确认 working tree 干净或明确「以当前工作区为发布源」
2. 记录 RELEASE_REV=$(git rev-parse --short HEAD)（或 dirty 标记）
3. 构建对应组件
4. 执行 publish/deploy 脚本（scp/rsync）
5. 健康检查 curl
6. 写/更新 VPS /opt/avrag-rs/DEPLOYED.txt：
     rev=...
     components=frontend,desktop-release,...
     at=ISO8601
```

### 4.3 对账（发现漂移时）

```text
本地：git status / git diff
线上：DEPLOYED.txt + 关键文件 mtime + latest.json version
若线上新、本地无 → 事故：从备份/记忆补回本地（不应发生）
若本地新、线上旧 → 正常：走发版
```

---

## 5. 一次性对齐清单（现在要做）

### 5.1 源码侧（本地）

| # | 动作 | 说明 |
|---|------|------|
| A1 | 梳理当前未提交改动 | 分成：frontend 产品 UI、tokens、desktop 下载、后端 migrations path、docs |
| A2 | 提交到本地 master（可分 commit） | 让「要上线的页面」进入 git 历史 |
| A3 | 可选：`backup/…` 分支 push（`[skip ci]` 或非 master） | 防本机丢失；不触发重 CI |
| A4 | 保证 `deploy/nginx/*`、`scripts/deploy-*.sh` 在仓库 | 配置即代码 |

### 5.2 运行侧（VPS）

| # | 动作 | 说明 |
|---|------|------|
| B1 | **不要**在 VPS 上再改业务文件 | |
| B2 | 用本地**已提交（或明确工作区）**重新 `pnpm build` + deploy frontend | 覆盖 `/opt/avrag-rs/frontend` |
| B3 | desktop-release 已用脚本发布则可保留；本地 `dist/` 与线上 `latest.json` 对一下 sha256 | |
| B4 | 把线上 nginx 中 app 段 **导出** 与仓库 `deploy/nginx`  diff；缺的补进仓库 | |
| B5 | 导出 unit 文件到 `deploy/systemd/*.service`（脱敏） | 可重建 |
| B6 | 更新 `DEPLOYED.txt` 写入 git rev | 下次对账用 |

### 5.3 密钥

| # | 动作 |
|---|------|
| C1 | 生产 env **只在** `/etc/avrag-rs/avrag.env` |
| C2 | 本地 `avrag-rs/.env` 保持 dev；对照 `.env.example` 键名齐全即可 |
| C3 | 若需备份 prod env：加密存放本机 `~/backups/vps-deploy-secrets/`（已有先例），**不进 git** |

---

## 6. 建议落地的脚本与目录（收口）

```text
deploy/
  nginx/
    app-contextlm.conf.example      # 从线上导出后入库（无密钥）
    app-releases-desktop.snippet.conf  # 已有
  systemd/
    avrag-frontend.service
    avrag-api.service.example       # 或 docker 说明
    why-frontend.service
    why-api.service
scripts/
  package-desktop-release.sh        # 已有
  publish-desktop-release.sh        # 已有
  deploy-frontend.sh                # ✅ build+rsync+restart
  deploy-backend.sh                 # ✅ bin+migrations+prompts + recreate containers
  deploy-status.sh                  # ✅ curl 健康 + 打印 DEPLOYED
  vps-pull-config.sh                # ✅ 只拉 nginx/unit 到 deploy/ 供 diff
deploy/docker/
  run-avrag-containers.sh           # ✅ VPS 上 recreate api/worker
```

**`deploy-frontend.sh` 契约（建议）**：

1. `git rev-parse HEAD` 写入发布元数据  
2. `pnpm build`  
3. 打包 standalone  
4. rsync 到 VPS  
5. `systemctl restart avrag-frontend`  
6. curl `/desktop` `/health`  
7. SSH 写 `DEPLOYED.txt`

以后 **禁止** 在对话里临时拼一长串 scp；一律走脚本。

---

## 7. 双端「对齐」定义（可检查）

满足以下即视为对齐：

1. **页面行为**：线上 `/desktop` 下载按钮与本地 `frontend_next` 当前 trunk 行为一致（同 commit 构建）。  
2. **安装包**：线上 `latest.json` 的 `sha256` = 本地 `dist/desktop-release` 对应文件（或发版记录中的 hash）。  
3. **配置**：线上 nginx 关键 location 能在 `deploy/nginx` 找到对应片段/全文。  
4. **可复现**：清空 `/opt/avrag-rs/frontend` 后，仅用本地脚本 + 当前 commit 能恢复。  
5. **不可复现部分仅限**：密钥、用户数据、证书私钥。

---

## 8. 与「直接在 VPS 开」的关系

| 做法 | 评价 |
|------|------|
| 在 VPS 上 **放** 安装包 / standalone | ✅ 正确（VPS 是 CDN/运行机） |
| 在 VPS 上 **改** 页面源码或手改 conf 不回写 | ❌ 造成漂移 |
| 本地改完再 publish | ✅ 标准路径 |
| 会话里 ad-hoc scp | ⚠️ 可应急；事后必须补脚本 + commit |

本次 desktop 下载：  
- 包与 `latest.json` 在 VPS 静态目录 → **合理**  
- `/desktop` UI 在本地改并 build 部署 → **合理**  
- 缺的是：**commit + 固定 deploy-frontend 脚本 + DEPLOYED rev**，否则三个月后无法回答「线上对应哪版代码」。

---

## 9. 实施顺序（建议 1 个工作单元内做完）

```text
P0  ✅ 本地：相关改动 commit 分条落盘（frontend / scripts / docs / deploy）
P0  ✅ 补 deploy-frontend.sh + deploy-status.sh
P0  ✅ 用脚本重发 frontend，写 DEPLOYED.txt(rev=…)
P1  ✅ vps-pull-config：nginx + systemd 入库
P1  ✅ 文档：本文件链到 AGENTS.md / SOLO「发版只走 scripts/deploy-*」
P2  ✅ deploy-backend.sh 收口 API/worker
P2  ⬜ 公域站（landing/why/canju）统一 publish 入口
```

---

## 10. 决策默认

| # | 决策 | 默认 |
|---|------|------|
| 1 | 源码真相 | 本地 `master` |
| 2 | VPS 无业务 git checkout | 是（只收 artifacts） |
| 3 | 发版入口 | `scripts/deploy-*.sh` / `publish-*.sh` |
| 4 | 生产 env | 仅 VPS；密钥不入库 |
| 5 | 会话内 ad-hoc 部署 | 仅紧急；事后脚本化 + 记录 rev |

---

## 11. 发版命令速查

```bash
# 前端（Next standalone → /opt/avrag-rs/frontend）
bash scripts/deploy-frontend.sh

# 后端（bins + migrations + prompts → docker recreate）
bash scripts/deploy-backend.sh
# 已有 release 二进制、仅重发：
SKIP_BUILD=1 bash scripts/deploy-backend.sh
# 只同步 migrations/prompts 并 restart：
ASSETS_ONLY=1 bash scripts/deploy-backend.sh

# Desktop 安装包
bash scripts/package-desktop-release.sh && bash scripts/publish-desktop-release.sh

# 对账
bash scripts/deploy-status.sh

# 从 VPS 拉 nginx/unit 回本地 diff
bash scripts/vps-pull-config.sh
```

**下一步**：P2 公域站统一入口（可选）；日常发版一律走上表脚本。
