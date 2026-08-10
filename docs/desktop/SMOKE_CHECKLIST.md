# Context-OS Client — 安装与本机栈冒烟清单

**版本目标**: **v0.2.0** — 免费客户端（ADR-0010）+ 品牌 NSIS + native 栈 + `RETRIEVAL_BACKEND=pgvector`  
**日期**: 2026-08-10（自 2026-08-04 清单修订：去掉激活墙）  
**用途**: 真机 / 本机验证；失败项记入文末表格  
**设计**: `docs/desktop/2026-08-10-v0.2.0-free-client-release.md`

---

## 0. 构建产物（A）

```bash
# 全量（久）：
bash scripts/build-windows.sh

# 加速（已有 frontend out + Windows sidecars 时）：
SKIP_FRONTEND=1 SKIP_SIDECARS=1 bash scripts/build-windows.sh

# 打包元数据：
SIGN_WINDOWS=0 bash scripts/package-desktop-release.sh
```

期望：

| 检查 | 期望 |
|------|------|
| NSIS 路径 | `desktop/src-tauri/target/x86_64-pc-windows-gnu/release/bundle/nsis/*setup.exe` |
| 产品名 | **Context-OS Client** |
| 版本 | **0.2.0**（文件名 / `latest.json`） |
| 体积 | **含便携 runtime** 量级 ~60MB+（LZMA；`SKIP_BUNDLED_RUNTIME=1` 时约 37MB 壳） |

---

## 1. 安装向导（Windows）

| # | 步骤 | 通过标准 | 结果 |
|---|------|----------|------|
| I1 | 双击 setup | 无 SmartScreen 外致命错误（未签名可能有警告） | ☐ |
| I2 | 欢迎页 | 标题含「欢迎使用 Context-OS 客户端」；文案含**免费 / 无需激活**（非「许可优先」） | ☐ |
| I3 | 许可页 | 出现 EULA；未勾选/未接受无法继续；按钮「我接受」/ I Agree | ☐ |
| I4 | 安装完成 | 默认勾选「立即启动 Context-OS 客户端」 | ☐ |
| I5 | 开始菜单 | 文件夹 **Context-OS** 下有快捷方式 | ☐ |
| I6 | 窗口标题 | **Context-OS Client** | ☐ |
| I7 | 图标 | 深 slate + 双弧 mark（非旧蓝六边形） | ☐ |

---

## 2. 冷启动（免费客户端）

| # | 步骤 | 通过标准 | 结果 |
|---|------|----------|------|
| L1 | 冷启动 | **不**跳转云 Login | ☐ |
| L2 | 无激活墙 | **不**被强制带到 `/activate` 才能用主路径 | ☐ |
| L3 | 进入产品 | 可到工作台 / 本机 setup（数据栈） | ☐ |
| L4 | 遗留入口 | `/activate`、`/desktop/buy` **重定向到 `/desktop`**，无结账 | ☐ |

> v0.1.x 的「试用 21 天 / 购买 / 输码」**不再**作为通过标准。

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
| S0 | 安装树 | `…\runtime\pgsql\bin\pg_ctl.exe` + `redis\redis-server.exe` + `pgsql\lib\vector.dll` | ☐ |
| S1 | 设置 → 本机数据栈 | 展示 PG+pgvector / Redis；**不**要求 Docker 就绪才能点「启动并迁移」 | ☐ |
| S2 | 启动并迁移 | native ensure 成功；`:5433` / `:6380` 通；`CREATE EXTENSION vector` OK | ☐ |
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
| K4 | 图/多跳题（若有） | 有检索/引用痕迹或合理答案 | ☐ |

---

## 5. 可选连云（非阻断）

| # | 步骤 | 通过标准 | 结果 |
|---|------|----------|------|
| C1 | 登录云账号 | 成功 | ☐ |
| C2 | 打开 `/pricing` 心智 | 分享名额 + 钱包；**不是**客户端买断 | ☐ |
| C3 | 云端 BYOK（若测） | 设置 → 自己的模型 Key；对话可走用户 Key | ☐ |

---

## 6. 卸载

| # | 步骤 | 通过标准 | 结果 |
|---|------|----------|------|
| U1 | 卸载向导 | 卸载 header 为灰色条 + Uninstall 文案 | ☐ |
| U2 | 完成 | 程序目录移除；可选删 app data | ☐ |

---

## 失败记录

| # | 现象 | 根因 | 处置 |
|---|------|------|------|
| | | | |
