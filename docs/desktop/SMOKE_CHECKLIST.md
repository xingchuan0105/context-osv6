# Context-OS Client — 安装与本机栈冒烟清单

**版本目标**: 含品牌 NSIS（Context-OS Client）+ native 栈 + `RETRIEVAL_BACKEND=pgvector`  
**日期**: 2026-08-04  
**用途**: 真机 / 本机验证；失败项记入文末表格

---

## 0. 构建产物（A）

```bash
# 全量（久）：
bash scripts/build-windows.sh

# 加速（已有 frontend out + Windows sidecars 时）：
SKIP_FRONTEND=1 SKIP_SIDECARS=1 bash scripts/build-windows.sh
```

期望：

| 检查 | 期望 |
|------|------|
| NSIS 路径 | `desktop/src-tauri/target/x86_64-pc-windows-gnu/release/bundle/nsis/*setup.exe` |
| 产品名 | **Context-OS Client**（文件名可能含空格） |
| 体积 | **含便携 runtime** 实测 ~**60MB**（2026-08-05 LZMA；`SKIP_BUNDLED_RUNTIME=1` 时约 37MB 壳） |

可选打包发布元数据：

```bash
bash scripts/package-desktop-release.sh
```

---

## 1. 安装向导（Windows）

| # | 步骤 | 通过标准 | 结果 |
|---|------|----------|------|
| I1 | 双击 setup | 无 SmartScreen 外致命错误（未签名可能有警告） | ☐ |
| I2 | 欢迎页 | 标题含「欢迎使用 Context-OS 客户端」或 Welcome to Context-OS Client；侧栏为深色 brand | ☐ |
| I3 | 许可页 | 出现协议；未勾选/未接受无法继续；按钮「我接受」/ I Agree | ☐ |
| I4 | 安装完成 | 默认勾选「立即启动 Context-OS 客户端」 | ☐ |
| I5 | 开始菜单 | 文件夹 **Context-OS** 下有快捷方式 | ☐ |
| I6 | 窗口标题 | **Context-OS Client** | ☐ |
| I7 | 图标 | 深 slate + 双弧 mark（非旧蓝六边形） | ☐ |

---

## 2. 许可 / 冷启动

| # | 步骤 | 通过标准 | 结果 |
|---|------|----------|------|
| L1 | 冷启动 | **不**跳转云 Login | ☐ |
| L2 | 未激活 | 进入 `/activate` 欢迎（试用 / 购买 / 输码） | ☐ |
| L3 | 试用 | 可本机签发；约 **21 天** | ☐ |
| L4 | 激活后 | 可进工作区 / dashboard | ☐ |

---

## 3. 本机数据面（native，无 Docker）

**Windows 干净机（BR2，默认安装包）**: **不**需系统 PostgreSQL / Redis / Docker。  
安装目录应有 `runtime/pgsql` + `runtime/redis`；数据在 `%LOCALAPPDATA%\Context-OS Client\`。

**Linux/WSL 开发机**可直接：

```bash
STACK_MODE=native bash scripts/desktop-local-stack.sh ensure
bash scripts/desktop-local-product.sh ensure
```

| # | 步骤 | 通过标准 | 结果 |
|---|------|----------|------|
| S0 | 安装树 | `…\runtime\pgsql\bin\pg_ctl.exe` + `redis\redis-server.exe` + `pgsql\lib\vector.dll` | ✅ 2026-08-05 silent（`%LocalAppData%\Context-OS Client`） |
| S1 | 设置 → 本机数据栈 | 展示 PG+pgvector / Redis；**不**要求 Docker 就绪才能点「启动并迁移」 | ☐ UI 待手测 |
| S2 | 启动并迁移 | native ensure 成功；`:5433` / `:6380` 通；`CREATE EXTENSION vector` OK | ✅ 脚本：`pg_ctl` 16.14 + vector **0.8.5**；redis listen OK（全量 ensure/UI 待手测） |
| S3 | client.env | 在 AppData；含 `RETRIEVAL_BACKEND=pgvector` | ☐ |
| S4 | 产品进程 | api `:18080` health OK；worker 有进程/日志 | ☐ |
| S5 | 本机会话 | 自动 `local@context-os.client` JWT；无云登录 | ☐ |
| S6 | 退出客户端 | 库进程停止（默认停库） | ☐ |

---

## 4. 知识路径（核心）

| # | 步骤 | 通过标准 | 结果 |
|---|------|----------|------|
| K1 | 建 workspace | 成功 | ☐ |
| K2 | 上传 1 文档 | 入库任务完成（Completed） | ☐ |
| K3 | 问答（本机 BYOK） | 有流式回答；非纯占位错误 | ☐ |
| K4 | 图/多跳题（若有） | 有检索/引用痕迹或合理答案（G3 另表） | ☐ |

---

## 5. 卸载

| # | 步骤 | 通过标准 | 结果 |
|---|------|----------|------|
| U1 | 卸载向导 | 卸载 header 为灰色条 + Uninstall 文案 | ☐ |
| U2 | 完成 | 程序目录移除；可选删 app data | ☐ |

---

## 失败记录

| 编号 | 现象 | 日志位置 | 结论 |
|------|------|----------|------|
| | | `desktop/runtime/logs/` · `%AppData%` | |

---

## 相关

- 运行时：`desktop/runtime/README.md`
- 便携 runtime stage：`desktop/runtime/bundled/README.md` · `scripts/stage-desktop-bundled-runtime.sh status`
- 安装素材：`desktop/src-tauri/nsis/README.md`
- G1/G2：`avrag-rs/docs/engineering/2026-08-04-pgvector-graph-hop-g1-spec.md`
