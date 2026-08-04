# 桌面客户端便携数据面设计（捆绑 PostgreSQL + pgvector + Redis）

**日期**: 2026-08-04  
**状态**: Design accepted（决策已拍板 2026-08-04）— **BR1 脚本/路径已落地**；首次 assemble+VPS 上传与 BR2 NSIS 嵌入待做
**作者意图**: 安装包从「壳 + sidecar（~37MB，用户自装库）」升级为「双击安装即可起本机数据面」，仍 **无 Docker、无云账号**，检索默认 **`RETRIEVAL_BACKEND=pgvector`**（VGRAG 主路径，G1/G2 已过）。

### 已拍板（2026-08-04）

| # | 议题 | 结论 |
|---|------|------|
| 1 | Redis Windows 形态 | **R1**：钉版本社区/官方系 Windows `redis-server` 构建（MIT/BSD 类优先；写入 THIRD_PARTY） |
| 2 | 应用退出时库进程 | **默认停库**（`pg_ctl stop` + redis shutdown）；不提供「退出后保持库运行」高级项（本期） |
| 3 | 大二进制存放 | **放 VPS 静态目录**（与现桌面发版同机）：`/var/www/releases/desktop/runtime/`；构建时拉取打进 NSIS。**不进 git**。详见 §15.1 |
| 4 | 首包小 + 首次启动在线下载 runtime | **不做**（安装包一次性带齐 runtime；用户不二次下载） |

**相关**:

| 文档 / 代码 | 角色 |
|-------------|------|
| `docs/desktop/SMOKE_CHECKLIST.md` | 安装冒烟 |
| `desktop/runtime/README.md` | 现行 native（系统包）栈 |
| `desktop/src-tauri/src/commands/native_stack.rs` | 已有 Rust 启停骨架（找 PATH 上的 pg_ctl） |
| `scripts/desktop-local-stack.sh` | bash native/docker 回退 |
| G1/G2 规格 | `avrag-rs/docs/engineering/2026-08-04-pgvector-graph-hop-g1-spec.md` |

---

## 1. 问题陈述

### 1.1 现状

| 产物 | 约体积 | 含什么 | 用户还要装什么 |
|------|--------|--------|----------------|
| `Context-OS Client_*.exe` NSIS | **~37MB** | Tauri 壳、前端静态、`avrag-api`/`avrag-worker`、图标/许可 | **PostgreSQL 16 + pgvector + Redis**（或 Docker 回退） |

因此即使用户装完客户端，**完整 RAG 仍依赖本机先装数据库**——与「双击即用 / 数据私有化」体感冲突。

### 1.2 目标（本设计）

| # | 目标 | 非目标（本期） |
|---|------|----------------|
| T1 | Windows x64 **安装包自带**可启动的 PG+pgvector + Redis | 不捆绑完整 Docker / Milvus |
| T2 | 首次启动 **自动 initdb + migrate + 起库**，无需用户理解端口 | 不实现多用户企业集群 |
| T3 | 数据目录在 **用户本机 AppData**（可备份、可卸载可选保留） | 不做跨机同步 |
| T4 | 与现产品契约一致：`RETRIEVAL_BACKEND=pgvector`、`DENSE_BACKEND=vgrag` | 不改云端默认 Milvus |
| T5 | 体积可预期、可审计、可签名 | 不追求「小于 50MB」 |

### 1.3 成功标准

1. 干净 Windows 10/11 机器：**不装 Docker、不装系统 PostgreSQL**，仅跑我们的 setup.exe 即可完成：许可 → 试用 → 起栈 → 上传 1 文档 → 检索/问答（需用户配置 BYOK Key）。  
2. 安装包下载体积 **目标 ≤ 180MB**，硬顶 **≤ 250MB**（压缩后 NSIS）。  
3. 卸载：可移除程序与运行时二进制；**数据目录默认保留**（可勾选删除）。  
4. 开发机 Linux 仍可用系统包 / Docker；便携包 **优先 Windows 交付**，Linux 便携为可选第二波。

---

## 2. 决策摘要

| 决策 | 选择 | 理由 |
|------|------|------|
| D1 检索后端 | **pgvector only**（桌面） | G1/G2 已验；避免 3GB+ Milvus |
| D2 是否仍要 Redis | **要，便携捆绑** | 产品 bootstrap 仍接 Redis（缓存/限流/worker 锁）；单机可后做「Redis 可选」但本期不拆 |
| D3 分发形态 | **安装目录内 `runtime/` 旁路二进制** + 数据在 **AppData** | 二进制可签名、可升级；数据不进 Program Files |
| D4 启动方式 | **Tauri 进程树内拉起**（`native_stack` 扩展），不依赖 bash/Docker | Windows 用户无 Git Bash |
| D5 体积策略 | **PG 精简发行 + 仅必要扩展 + Redis 单文件** | 见 §4 |
| D6 Docker | **保留为 advanced 回退**，默认关闭 | 已有 compose；不删 |
| D7 系统已装 PG | **优先捆绑路径**；若 `COS_USE_SYSTEM_PG=1` 可走系统 | 避免双实例端口冲突默认用 5433 |

---

## 3. 架构

```text
┌─────────────────────────────────────────────────────────────┐
│  Context-OS Client.exe (Tauri)                                │
│   ensure_local_stack / 首次启动                               │
│     1. resolve_runtime_bins()  →  安装目录\runtime\...        │
│     2. ensure_pg()             →  initdb / pg_ctl start       │
│     3. ensure_redis()          →  redis-server                │
│     4. write client.env        →  AppData\...\client.env      │
│     5. migrate                 →  内嵌 migrate 或 sqlx 侧车   │
│     6. ensure_local_product    →  avrag-api + worker          │
└───────────────┬─────────────────────────────┬─────────────────┘
                │ loopback only               │
                ▼                             ▼
     127.0.0.1:5433                    127.0.0.1:6380
     postgres + vector                 redis
                │
                ▼
     %LOCALAPPDATA%\Context-OS Client\
        data\pg\          # PGDATA
        data\redis\
        logs\
        client.env
        jwt.secret
```

**原则**:

- 只监听 **127.0.0.1**（不暴露到局域网）。  
- 口令本地随机生成写入 `client.env`（不硬编码进安装包）。  
- 产品进程与库进程 **同生命周期**：应用启动 ensure；**退出一律 stop 库**（见 §6.2 / 已拍板）。

---

## 4. 体积与组件选型

### 4.1 目标体积分解（Windows x64，压缩前 / NSIS 后粗估）

| 组件 | 未压缩粗估 | 进 NSIS 后粗估 | 说明 |
|------|------------|----------------|------|
| 现有壳 + api/worker | ~55–70MB | ~35–40MB | 已有 |
| PostgreSQL 16 精简树 | ~90–150MB | ~45–80MB | 去掉无用 locale/contrib 文档 |
| pgvector | ~1–5MB | ~1–3MB | `.dll` + control/sql |
| Redis（Windows 构建） | ~5–15MB | ~3–8MB | 或 Memurai Dev 许可需审 |
| migrate 工具 / 脚本 | ~5–15MB | ~3–8MB | 见 §6.3 |
| **合计目标** | | **~100–180MB** | 硬顶 250MB |

### 4.2 PostgreSQL 来源（推荐顺序）

| 选项 | 做法 | 利 | 弊 |
|------|------|----|----|
| **P1（推荐）** | 官方 Windows x64 zip 或 EDB 便携布局 → CI 裁剪后入仓/下载缓存 | 合法清晰、可复现 | 首次集成需写裁剪清单 |
| P2 | `postgresql_embedded` / theseus 系在 **构建机** 下载再打进包 | 自动化 | 许可与路径要钉版本 |
| P3 | 要求用户装系统 PG | 包小 | **违背本设计目标** |

**版本钉扎**: PostgreSQL **16.x**（与现 migrate / 开发机一致）。升级 major 需单独立项。

### 4.3 pgvector 来源

| 选项 | 做法 |
|------|------|
| **推荐** | 对钉扎的 PG 16 **交叉/原生编译** pgvector Release，产出 `vector.dll` + `vector.control` + SQL 放入 `share/extension` 与 `lib` |
| 备选 | 使用已预编译的第三方 wheel/zip（**必须校验签名与 PG 次版本匹配**） |

`CREATE EXTENSION vector` 必须在首次 migrate / ensure 时成功，否则桌面检索不可用。

### 4.4 Redis 来源

| 选项 | 做法 | 注意 |
|------|------|------|
| **R1（已选）** | 钉版本社区/官方系 **Windows Redis 构建** | MIT/BSD 类优先；写入 THIRD_PARTY；禁止默默换成 SSPL 源而不审 |
| R2 | 捆绑 **Memurai Developer** | 许可与商用条款需法务 — **不选** |
| R3 | 改产品使 Redis 可选（内存 CachePort） | **行为变更**，另开 ADR — **本期不做** |

**落地**：单机 `redis-server.exe`（+ 可选 `redis-cli.exe`），AOF/RDB 落 AppData；**应用退出时 shutdown**。

### 4.5 明确不捆绑

- Docker Desktop、etcd、MinIO、Milvus  
- 完整 PG 文档、多余 locale、stack builder、pgAdmin  
- 开发头文件 / 静态库（仅运行时）

---

## 5. 安装布局（Windows）

### 5.1 程序目录（随 setup，可替换升级）

```text
%LOCALAPPDATA%\Programs\Context-OS Client\    # 或 NSIS 所选 INSTDIR
  Context-OS.exe
  avrag-api.exe
  avrag-worker.exe
  WebView2Loader.dll
  runtime\
    pgsql\                    # 便携 PG 根
      bin\postgres.exe
      bin\pg_ctl.exe
      bin\initdb.exe
      bin\psql.exe
      bin\pg_isready.exe
      lib\...
      share\extension\vector*
    redis\
      redis-server.exe
      redis-cli.exe           # 可选，便于排障
    bin\                      # 可选：migrate 小工具
      cos-migrate.exe         # 见 §6.3
    README.txt
```

NSIS：`resources` / 自定义拷贝 `runtime/**`（不要放进会炸路径长度的过深树；控制文件数）。

### 5.2 数据目录（用户数据，默认不随升级清空）

```text
%LOCALAPPDATA%\Context-OS Client\
  data\
    pg\                       # PGDATA（initdb 于此）
    redis\
  logs\
    postgres.log
    redis.log
    api.log
    worker.log
  client.env                  # DATABASE_URL / JWT / RETRIEVAL_BACKEND
  jwt.secret
  stack.mode                  # bundled | system | docker
  runtime.version             # 捆绑 runtime 版本戳，用于迁移策略
```

**开发 monorepo** 可继续用 `desktop/runtime/data/pg-native` 以兼容现脚本；安装态以 AppData 为准。

### 5.3 端口

| 服务 | 端口 | 冲突策略 |
|------|------|----------|
| PG | **5433** | 占用则顺序尝试 5434–5440，写回 `client.env` |
| Redis | **6380** | 占用则 6381–6385 |
| API | **18080** | 已有逻辑 |

---

## 6. 生命周期

### 6.1 首次启动（冷）

```text
Client 启动
  → license gate
  → ensure_local_stack (bundled)
       if PGDATA 不存在:
            initdb -U avrag --auth-local=trust --auth-host=scram-sha-256
            写 postgresql.conf (listen 127.0.0.1, port, unix_socket 禁用或本地)
            写 pg_hba.conf (仅 127.0.0.1 scram/trust 策略见 §8)
            pg_ctl start
            create database avrag_client
            CREATE EXTENSION vector
       else:
            pg_ctl start (若未运行)
       redis-server (若未运行)
       write client.env (随机 DB 密码首次生成)
       cos-migrate / sqlx migrate run
  → ensure_local_product (api+worker)
  → ensure_local_session
  → 工作区 UI
```

UI：设置页显示「本机数据库：捆绑 / 运行中 / 端口」；失败时中文错误 + 打开日志目录。

### 6.2 日常启动 / 退出

| 事件 | 行为（推荐默认） |
|------|------------------|
| 应用启动 | ensure（幂等，已运行则跳过） |
| 应用退出 | **一律 stop redis + pg**（已拍板；释放内存与端口，避免后台常驻） |
| 崩溃后重启 | ensure 再次拉起 |

### 6.3 迁移（migrate）如何进包

| 方案 | 说明 |
|------|------|
| **M1（推荐）** | 打一个很小的 `cos-migrate.exe`：内嵌 `sqlx` migrate runner 或调用已有 migrations 目录（migrations 作 resource 拷贝） |
| M2 | 继续依赖开发者本机 `sqlx`（**安装态不可接受**） |
| M3 | avrag-api 启动时 `AVRAG_RUN_MIGRATIONS=true`（已有开关）— **安装态可用**，需确认 release 二进制包含 migrate 逻辑且路径正确 |

**推荐落地**：安装态优先 **M3**（已有 `AVRAG_RUN_MIGRATIONS` + `AVRAG_MIGRATIONS_DIR`），migrations SQL 作为 `runtime/migrations/` 资源拷贝；`cos-migrate` 仅作排障备用。

### 6.4 版本升级

| 场景 | 策略 |
|------|------|
| 应用小版本升级 | 覆盖 `runtime/pgsql` 二进制；**不删** PGDATA；启动 migrate |
| PG minor 升级 | 同 major 可原地；测 `pg_upgrade` 仅 major |
| pgvector 升级 | 替换 extension 文件 + migrate/扩展脚本 |
| runtime.version 变化 | ensure 时检测，必要时提示「需要重启数据库」 |

---

## 7. 与现有代码的衔接

### 7.1 解析顺序（`native_stack` 扩展）

```text
1. $INSTDIR/runtime/pgsql/bin/pg_ctl.exe     # 捆绑（安装态）
2. CONTEXT_OS_RUNTIME / desktop/runtime/native/pgsql  # 开发态 stage
3. PG_BIN_DIR / 系统 PATH                     # 高级 / CI
```

Redis 同理：`runtime/redis/redis-server.exe` → PATH。

### 7.2 配置

`client.env` 保持现字段，并增加：

```text
STACK_MODE=bundled
COS_RUNTIME_ROOT=...
COS_PGDATA=...
COS_RUNTIME_VERSION=2026.08.04-pg16.4-vec0.8.0
```

产品进程加载顺序不变：先 monorepo `.env`（开发）再强制 `client.env` 覆盖（已有 product 脚本逻辑）。

### 7.3 NSIS / 构建流水线

| 步骤 | 脚本（拟） |
|------|------------|
| 下载/裁剪 PG + 编译/放置 pgvector + Redis | `scripts/stage-desktop-bundled-runtime.sh`（可在 Linux 交叉有限，**建议 Windows CI 或专用构建机**） |
| 产出目录 | `desktop/runtime/bundled/windows-x64/` |
| 打进 NSIS | `tauri.conf` `bundle.resources` 或 `build-windows.sh` 额外拷贝 |
| 签名 | 与 setup.exe 同流水线对 `runtime/**/*.exe` 签名 |

开发者本机无 Windows 时：从 **VPS `releases/desktop/runtime/`** 下预构建 zip 到本地 cache（不提交大二进制进 git）。

---

## 8. 安全

| 项 | 要求 |
|----|------|
| 监听 | 仅 127.0.0.1 |
| 密码 | 首次随机；存 `client.env`（ACL 限当前用户） |
| pg_hba | 拒绝非本机；生产感用 scram + 本地 trust 二选一（推荐 **scram + 密码**） |
| 防火墙 | 不主动添加入站规则 |
| 供应链 | 记录 PG/Redis/pgvector 版本与来源 URL、校验 sha256 |
| 卸载 | 默认不删用户文档库；明确勾选才删 AppData |

---

## 9. 许可与合规

| 组件 | 典型许可 | 动作 |
|------|----------|------|
| PostgreSQL | PostgreSQL License | THIRD_PARTY 声明 |
| pgvector | PostgreSQL License | 同上 |
| Redis | RSALv2 / SSPL 视版本 **或** 旧 BSD 构建 | **钉版本并审**；优先选许可清晰的 Windows 构建 |
| 我们的客户端 | 现有商业许可 | 安装许可页已有 |

必须在 `THIRD_PARTY_NOTICES` / 安装目录 `runtime/THIRD_PARTY.txt` 列出。

---

## 10. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| 体积超 250MB | 下载劝退 | 裁剪 locale/文档；可选「在线拉取 runtime」二期 |
| pgvector 与 PG 次版本不匹配 | CREATE EXTENSION 失败 | 构建时同版本矩阵；ensure 健康检查 |
| Windows Defender 误报 redis/pg | 启动失败 | 签名；文档加排除路径 |
| 杀软锁文件导致 pg_ctl 起不来 | 首启失败 | 重试 + 中文日志 |
| 双实例（系统 PG 占 5433） | 冲突 | 端口扫描；提示 |
| Redis 许可变更 | 合规 | 钉版本；评估无 Redis 模式作后备 ADR |
| 无 sqlx 导致 migrate 失败 | 库空 | 走 api `AVRAG_RUN_MIGRATIONS` |
| SmartScreen | 用户吓退 | EV 签名（已有自签≠生产） |

---

## 11. 分阶段落地

### Wave BR0 — 设计与钉版本（本文档）

- [x] 决策与布局  
- [x] 选定具体 PG 16.x / Redis URL 候选（`desktop/runtime/bundled/pins.env`；sha256 首次 assemble 后回填）  
- [ ] 体积试裁一次（人工/脚本），写入附录实测表  

### Wave BR1 — 运行时包 + 本地 stage

- [x] `scripts/stage-desktop-bundled-runtime.sh`（fetch/pack/verify/assemble/status）  
- [x] `scripts/publish-desktop-bundled-runtime.sh` → VPS `releases/desktop/runtime/`  
- [x] `native_stack` + `desktop-local-stack.sh`：优先捆绑/安装路径  
- [x] monorepo 路径：`desktop/runtime/bundled/{windows,linux}-x64/`（stage 后 ensure 可发现；Linux 便携二进制属 BR4）  
- [ ] 首次 assemble + 放入 pgvector + pack 发布到 VPS（需网络与 vector 预编译物）

### Wave BR2 — 装进 NSIS

- [ ] `build-windows.sh` 嵌入 `runtime/bundled`  
- [ ] 安装后路径探测  
- [ ] 卸载不删数据；可选删  
- [ ] 更新 `SMOKE_CHECKLIST`：干净机无系统 PG  

### Wave BR3 — 体验与硬化

- [ ] 设置页「数据库状态 / 打开数据目录 / 重启库」  
- [ ] 退出时 **强制 stop** 库（无「保持运行」开关，已拍板）  
- [ ] 自动端口避让  
- [ ] 体积与启动时间仪表（写进 release notes）  

### Wave BR4（可选，不含「首启在线下载 runtime」— 已否决）

- [ ] Linux 便携 tarball  
- [ ] Redis 可选化 ADR（与 R3 相关，另议）  

---

## 12. 验收清单（实现后）

| # | 场景 | 通过 |
|---|------|------|
| V1 | 干净 Win11，无 Docker/无系统 PG，仅 setup | 装完能进工作区并完成 migrate |
| V2 | `CREATE EXTENSION vector` 成功 | 是 |
| V3 | 上传文档 + 检索 | 是（BYOK 已配） |
| V4 | 杀进程后重启 Client | 自动 ensure 恢复 |
| V5 | 卸载默认 | 程序删除，AppData 数据仍在 |
| V6 | 安装包 ≤ 250MB | 是 |
| V7 | 仅 127.0.0.1 监听 | 是 |

---

## 13. 附录 A — 与「37MB」的关系（对用户话术）

| 包类型 | 体积量级 | 含义 |
|--------|----------|------|
| 当前发布 | ~37MB | 仅应用；库自备 |
| 本设计落地后 | ~100–180MB（目标） | 应用 + 便携库运行时 |
| 旧 Docker+Milvus 路线 | 数 GB | 已否决为默认 |

---

## 14. 附录 B — 版本钉扎

权威文件：`desktop/runtime/bundled/pins.env`（脚本 source）。摘要：

| 组件 | 候选版本 | 来源 | sha256 | 备注 |
|------|----------|------|--------|------|
| PostgreSQL | 16.14 | EDB Windows binaries zip | 首次 assemble 后回填 `PG_WIN_SHA256` | |
| pgvector | 0.8.0 | 预编译 DLL 入 cache / `PGVECTOR_WIN_ZIP` | 同上 | 与 PG 16 ABI |
| Redis Windows | 5.0.14.1 | tporadowski/redis release zip | 同上 | R1；升级前审许可 |
| 试裁未压缩 | _ MB | | | pack 后记 |
| 试裁 NSIS 增量 | _ MB | | | BR2 |

---

## 15. 决策记录

### 15.0 已拍板（见文首表）

1. Redis = **R1**  
2. 退出 = **停库**  
3. 大二进制 = **VPS** `/var/www/releases/desktop/runtime/`（构建时拉）  
4. 用户首启在线下 runtime = **不做**（安装包一次带齐）

### 15.1 大二进制存放：**已选 VPS**（对照三种选项）

便携 PG/Redis 约 **百 MB 级**，不宜进 git。三种常见选项：

| 选项 | 是什么 | 优点 | 缺点 |
|------|--------|------|------|
| A. Git LFS | 大文件进仓 | 和代码同版本 | clone 慢、仓胀 — **不选** |
| B. GitHub Release Asset | tag 附件 | 不污染 git | 多一套托管 — **不优先** |
| **C′. 自有 VPS 静态站（已选）** | 与现 **桌面 setup 发版同一 VPS** | 已有脚本/nginx/域名；无 GitHub 依赖 | 占 VPS 磁盘；要控权限与校验 |

#### 为何放 VPS 合适

你们**已经**把桌面安装包发到 VPS：

- 脚本：`scripts/publish-desktop-release.sh`
- 远端：`/var/www/releases/desktop/`（`v{version}/` + `latest.json`）
- 公网形态：`https://app.contextlm.top/releases/desktop/…`（见 `deploy-status` 探测）

便携 runtime 与 setup.exe **同类产物**（构建产物、要签名/校验、按版本存），挂在同一棵树下最省事，不必新开 MinIO bucket 或 GitHub Release。

#### 建议目录约定

```text
VPS: /var/www/releases/desktop/
  latest.json                          # 现有：指向 setup.exe
  v0.1.0/
    Context-OS-Client_0.1.0_x64-setup.exe
    runtime-sidecars/                  # 现有 companion（可选）
  runtime/                             # 新增：可复用的「库运行时」原料（给构建机下）
    manifest.json                      # 版本、组件、sha256、兼容的 Client 版本范围
    windows-x64/
      cos-runtime-pg16.x-redis-x-win-x64.zip
      cos-runtime-pg16.x-redis-x-win-x64.zip.sha256
```

- **给构建机 / 开发者**：从  
  `https://<公网>/releases/desktop/runtime/windows-x64/….zip`  
  下载 → 校验 sha256 → stage 进 NSIS。  
- **给终端用户**：**不**要求他们下 zip；下的仍是 **完整 setup.exe**（内含已打好的 runtime）。  
- 本地缓存：`~/.cache/context-osv6/bundled-runtime/`（stage 脚本写入，可离线复用）。

#### 和「用户不二次下载」的关系

| 谁下载 | 是否做 |
|--------|--------|
| **用户**装完客户端再下 runtime | **不做**（决策 4） |
| **构建 setup 时**从 VPS 拉 zip 打进包 | **要做**；用户拿到完整离线安装包 |
| **用户**下载 setup.exe | 仍走现有 `/releases/desktop/v*/`（体积变为 ~100–180MB） |

#### 上传方式（实现时）

- 复用 `VPS_MAIN_*` + rsync/sshpass（与 `publish-desktop-release.sh` 同凭证）。  
- 可增：`scripts/publish-desktop-bundled-runtime.sh`（只推 `runtime/` 树）。  
- nginx 已服务 `releases/desktop` 则 **几乎零额外配置**（注意大文件超时/磁盘配额）。

#### 不选「产品 MinIO」的原因（可后补）

应用内 MinIO 面向用户对象存储；runtime zip 是 **发版构件**，放 **静态 releases** 更清晰，也避免给匿名构建拉取开 bucket 密钥。

### 15.2 仍待实现时填写（非产品决策）

- 具体 Redis Windows 构建的 **URL + 版本号 + sha256**（R1 的实例化）  
- PG 16.x patch、pgvector tag、试裁体积实测（附录 B）

---

**一句话**：把 **钉版本的便携 PG16+pgvector+Redis（R1）** 放进安装目录 `runtime/`，数据放 **AppData**，退出 **停库**，安装包 **一次带齐**；大 zip **放 VPS `releases/desktop/runtime/`**，仅 **构建机/开发者** 拉取打进 setup（用户不二次下）；由 **`native_stack` 扩展捆绑路径** 落地。
