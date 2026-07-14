# Desktop 发版与网页下载 Runbook

配套计划：`docs/engineering/DESKTOP_WEB_DOWNLOAD_INSTALL_PLAN_2026-07-14.md`  
版本约定：`docs/desktop/VERSIONING.md`

## 1. 构建 Windows 产物

在 Windows 机（推荐）或已装 mingw 的 Linux：

```bash
# 可选：交叉编
bash scripts/build-windows.sh

# 或在 desktop 目录（Windows 本机）
cd frontend_next && pnpm build:desktop
cd ../desktop && pnpm tauri build
```

产物常见路径：

- NSIS：`desktop/src-tauri/target/*/release/bundle/nsis/*-setup.exe`
- 便携：`desktop/src-tauri/target/*/release/avrag-desktop.exe`

## 2. 打包 latest.json

```bash
bash scripts/package-desktop-release.sh
# 输出: dist/desktop-release/latest.json
#       dist/desktop-release/v{version}/…
```

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

## 4. 网页

- 介绍/下载：`/desktop`
- 购买/激活：`/desktop/buy`（成功页含下载入口）

前端读取 `latest.json` 渲染「下载 Windows 版」。

## 5. 验收

```bash
curl -sS https://app.contextlm.top/releases/desktop/latest.json | jq .
curl -sSI https://app.contextlm.top$(jq -r '.platforms["windows-x64"].url' <<< "$(curl -sS https://app.contextlm.top/releases/desktop/latest.json)")
# 浏览器打开 /desktop → 下载 → Win 机运行 → /desktop/buy 激活
```
