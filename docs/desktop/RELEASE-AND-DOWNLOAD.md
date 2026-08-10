# Desktop 发版与网页下载 Runbook

配套：

- 版本约定：`docs/desktop/VERSIONING.md`
- 冒烟：`docs/desktop/SMOKE_CHECKLIST.md`
- **v0.2.0 设计**：`docs/desktop/2026-08-10-v0.2.0-free-client-release.md`
- 商业模式：`docs/adr/0010-share-service-business-model.md`（客户端免费）

历史计划（已部分 SUPERSEDED）：`docs/engineering/DESKTOP_WEB_DOWNLOAD_INSTALL_PLAN_2026-07-14.md`

## 1. 构建 Windows 产物（主格式 = NSIS setup.exe）

**推荐入口（本机 Ubuntu/WSL 可交叉编）：**

```bash
# 依赖：mingw-w64、nsis、Rust（desktop/src-tauri/rust-toolchain.toml）
# sudo apt-get install -y mingw-w64 nsis
# 默认嵌入便携 PG+pgvector+Redis（需先有 desktop/runtime/bundled/windows-x64，否则自动 fetch）
bash scripts/build-windows.sh
# 已有 frontend_next/out 时可：SKIP_FRONTEND=1 bash scripts/build-windows.sh
# 仅壳（~37MB，不嵌库）：SKIP_BUNDLED_RUNTIME=1 bash scripts/build-windows.sh
```

或 Windows 本机：

```bash
cd frontend_next && pnpm build:desktop
cd ../desktop && pnpm tauri build --bundles nsis
```

产物：

- **主下载**：`desktop/src-tauri/target/*/release/bundle/nsis/*-setup.exe`
- 便携 exe（仅应急）：`…/release/Context-OS.exe`（`ALLOW_PORTABLE=1` 才允许打包）

**Authenticode 签名**（Linux 交叉编可用 `osslsigncode`）：

```bash
# 生产 OV/EV 证书
export WINDOWS_CERTIFICATE_FILE=/secure/path/codesign.pfx
export WINDOWS_CERTIFICATE_PASSWORD='…'
# 可选时间戳
# export WINDOWS_TIMESTAMP_URL=http://timestamp.digicert.com

bash scripts/package-desktop-release.sh   # SIGN_WINDOWS=1 默认会签
# 或单独： bash scripts/sign-windows-release.sh path/to/setup.exe

# 无商业证书时开发自签（SmartScreen 仍可能提示未知发布者）
SIGN_ALLOW_SELF_SIGNED=1 bash scripts/package-desktop-release.sh
# 跳过签名：
SIGN_WINDOWS=0 bash scripts/package-desktop-release.sh
```

证书与口令**永不入库**；自签材料落在 gitignored 的 `desktop/signing/`。  
`latest.json` 字段 `platforms.windows-x64.authenticode` 为 true/false。

## 2. 打包 latest.json

```bash
bash scripts/package-desktop-release.sh
# 输出: dist/desktop-release/latest.json
#       dist/desktop-release/v{version}/…
```

期望文件名：`Context-OS-Client_{version}_x64-setup.exe`。

## 3. 发布到 VPS

需 `avrag-rs/.env` 中 `VPS_MAIN_*`：

```bash
bash scripts/publish-desktop-release.sh
```

目标目录：`/var/www/releases/desktop/`  
公网：

- `https://app.contextlm.top/releases/desktop/latest.json`
- `https://app.contextlm.top/releases/desktop/v{version}/…`

nginx：`app.contextlm.top` 的 `location /releases/desktop/`（见 `deploy/nginx/app-releases-desktop.snippet.conf`）。

### 3.1 便携 runtime 原料（构建机用，非终端用户）

设计：`docs/desktop/2026-08-04-portable-runtime-design.md` §15.1。  
**用户不二次下载**；zip 仅供 stage 进 NSIS。

```bash
# 本地已有 windows-x64 树时
bash scripts/stage-desktop-bundled-runtime.sh pack
bash scripts/publish-desktop-bundled-runtime.sh

# 其它构建机：从 VPS 拉回
bash scripts/stage-desktop-bundled-runtime.sh fetch
```

VPS：`/var/www/releases/desktop/runtime/`  
公网：`https://app.contextlm.top/releases/desktop/runtime/manifest.json`

## 4. 网页与用户旅程

- 介绍/下载：**`/desktop`**（主路径）
- 云端会员/充值：`/pricing`（分享名额 + 钱包；**不是**客户端买断）
- 历史页 `/desktop/buy`：仅降权说明，**不得**作为安装验收必经步骤

前端读取 `latest.json` 渲染「下载 Windows 版」。

## 5. 验收

```bash
# 本地 dist（打包后）
jq . dist/desktop-release/latest.json
# 公网（publish 后）
curl -sS https://app.contextlm.top/releases/desktop/latest.json | jq .
curl -sSI "https://app.contextlm.top$(jq -r '.platforms["windows-x64"].url' <<< "$(curl -sS https://app.contextlm.top/releases/desktop/latest.json)")"
```

手动：

1. 浏览器打开 `/desktop` → 下载 setup  
2. Win 机安装（EULA 接受即可；**无需**激活码）  
3. 冷启动 → 本机栈 → 本机 BYOK 问答  
4. **不要**把「必须走 /desktop/buy 激活」当作通过条件  

详见 `docs/desktop/SMOKE_CHECKLIST.md`。
