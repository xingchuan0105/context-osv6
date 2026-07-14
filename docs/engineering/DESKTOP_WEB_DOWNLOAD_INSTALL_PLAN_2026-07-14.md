# AVRag Desktop：网页下载安装开发计划

**日期**: 2026-07-14  
**状态**: Plan（待实施）  
**目标**: 用户在公网网页上可下载 Windows 客户端并完成本地安装与激活  
**关联**: `desktop/`（Tauri 2）、`/desktop` 与 `/desktop/buy`、`scripts/build-windows.sh`、授权 deep link `avrag-desktop://activate`

---

## 1. 背景与现状

### 1.1 已有能力

| 能力 | 位置 | 状态 |
|------|------|------|
| Tauri 2 桌面壳 | `desktop/` | 工程存在，`productName=AVRag Desktop`，`version=0.1.0` |
| 前端桌面构建 | `frontend_next` + `BUILD_TARGET=desktop` / `pnpm build:desktop` | 配置在 `tauri.conf.json` `beforeBuildCommand` |
| Windows 交叉编脚本 | `scripts/build-windows.sh` | 产出指向 `desktop/src-tauri/target/.../bundle/` |
| 本机曾有产物 | `…/release/avrag-desktop.exe`、`bundle/nsis/` | **未发布** |
| 营销页 | `app.contextlm.top/desktop` | 仅「购买授权」「了解更多」 |
| 购买/激活 | `/desktop/buy` + Creem + `avrag-desktop://activate?key=` | 授权链路已接，**依赖客户端已安装** |
| GitHub Releases | `xingchuan0105/context-osv6` | **空** |

### 1.2 缺口（用户感知）

```text
想装 Windows 客户端
  → 打开 /desktop
  → 没有「下载安装包」
  → 无法完成「下载 → 安装 → 激活」闭环
```

### 1.3 成功标准（验收）

1. 未登录用户打开 `/desktop` 可看到 **下载 Windows** 主按钮，一点即下安装包（NSIS 或 EXE）。  
2. 安装包来自 **版本化、可校验** 的发布通道（含 SHA256）。  
3. 安装后 deep link / 授权码激活仍可用（与现 `/desktop/buy` 兼容）。  
4. 下载页展示版本号、系统要求、简要安装步骤。  
5. 发版流程可重复：构建 → 上传 → 更新网页元数据 → 冒烟。  
6. **不**把整仓源码当下载源；**不**要求用户装 Rust/pnpm。

---

## 2. 目标用户路径

```text
[路径 A · 先下载]
  /desktop → 下载 NSIS 安装包 → 本地安装
  → （可选）/desktop/buy 购买 → 复制 key 或 avrag-desktop://activate
  → 客户端激活

[路径 B · 先购买]
  /desktop/buy → 登录 → Creem 结账 → 展示 key + 打开客户端链接
  → 若未安装：同页/弹层提示「先下载客户端」→ 回路径 A

[路径 C · 已安装升级]
  /desktop 展示「当前最新 vX.Y.Z」+ 下载按钮
  → （后续波次）应用内检查更新
```

首期以 **路径 A + B 闭环** 为主；应用内自动更新为可选 Phase 2。

---

## 3. 架构方案

### 3.1 发布物

| 产物 | 用途 | 优先级 |
|------|------|--------|
| **NSIS 安装包** `AVRag Desktop_x.y.z_x64-setup.exe` | 主下载（Tauri Windows 默认 bundle） | **P0** |
| Portable `.exe`（若有） | 便携/二次镜像 | P2 |
| `SHA256SUMS` | 校验 | **P0** |
| `latest.json` | 网页读版本与下载 URL | **P0** |

版本源：**单一事实** = `desktop/package.json` / `tauri.conf.json` 的 `version`（发版时同步 bump）。

### 3.2 托管位置（二选一，推荐 A）

| 方案 | 做法 | 优点 | 缺点 |
|------|------|------|------|
| **A. 对象存储 + CDN 友好静态 URL（推荐）** | 上传到 MinIO/S3 或云厂商 COS；公网 `https://downloads.contextlm.top/...` 或现有域名下 `/releases/desktop/` | 可控、不绑 GitHub 额度 | 需配置 bucket/CORS/缓存头 |
| **B. GitHub Releases** | `gh release create vX.Y.Z` 挂附件 | 免费、审计清晰 | 国内下载慢；与 solo 仓库权限绑定 |

**建议 Phase 1：A**，在主站同域或子路径暴露，避免新域名证书拖延：

```text
https://app.contextlm.top/releases/desktop/v0.1.0/AVRag-Desktop_0.1.0_x64-setup.exe
https://app.contextlm.top/releases/desktop/v0.1.0/SHA256SUMS
https://app.contextlm.top/releases/desktop/latest.json
```

实现方式任选其一：

1. **nginx 静态目录**：`/var/www/releases/desktop/`（与 landing 类似，运维简单）  
2. **MinIO + nginx reverse proxy** 到 bucket  

`latest.json` 示例：

```json
{
  "product": "AVRag Desktop",
  "version": "0.1.0",
  "published_at": "2026-07-14T00:00:00Z",
  "platforms": {
    "windows-x64": {
      "url": "/releases/desktop/v0.1.0/AVRag-Desktop_0.1.0_x64-setup.exe",
      "sha256": "…",
      "size_bytes": 12345678,
      "format": "nsis"
    }
  },
  "min_os": "Windows 10 64-bit",
  "notes_url": "/desktop#changelog"
}
```

网页 **只读 `latest.json`** 渲染按钮，避免每次发版改前端代码硬编码 URL（允许 fallback 常量）。

### 3.3 网页信息架构

#### `/desktop`（介绍 + 下载）

- 主 CTA：**下载 Windows 版**（直链 `latest.json` 中 url，或服务端渲染注入）  
- 次 CTA：购买授权 → `/desktop/buy`  
- 元信息：版本、大小、SHA256（可折叠）、系统要求、简要 3 步安装  
- 次要：macOS/Linux「即将推出」灰态（若无包则不链死链）  
- 视觉：跟 Monochrome Ink（已有 marketing 壳）

#### `/desktop/buy`（购买）

- 结账成功区：除 key / deep link 外，增加 **「尚未安装？下载客户端」** → 同 latest 安装包  
- 保持 `avrag-desktop://activate?key=`  

#### 可选 `/desktop/download`

- 若希望统计跳转：中转页 `GET /desktop/download?platform=windows-x64` → 302 到真实 URL + 埋点  
- Phase 1 可省略，按钮直链即可

### 3.4 鉴权与商业策略

| 决策 | 建议 |
|------|------|
| 安装包是否免费下载 | **是**（买断的是 license，不是下载墙） |
| 是否必须登录才下载 | **否**（降低安装漏斗阻力） |
| 未激活是否可启动 | 保持现客户端逻辑（试用/锁功能以现实现为准，本计划不改授权模型） |

---

## 4. 工作分解（Waves）

### Wave 0 — 发布门禁与产物约定（0.5 天）

| ID | 任务 | 产出 | 验收 |
|----|------|------|------|
| D0-1 | 锁定 Windows 目标：`x86_64-pc-windows-msvc`（原生 Win 机构建）为主；gnu 交叉为备选 | ADR 一小节写入本文件 §6 | 构建说明无歧义 |
| D0-2 | 约定安装包命名与版本 bump 清单 | `VERSIONING` 补充或本文件 | 命名表稳定 |
| D0-3 | 系统要求文案 | Win10+、WebView2、磁盘约 N MB | 写进 `/desktop` |

**版本 bump 清单（每次发版）**：`desktop/package.json`、`desktop/src-tauri/tauri.conf.json`、（若有）Cargo package version。

### Wave 1 — 可重复 Windows 构建（1–2 天）

| ID | 任务 | 产出 | 验收 |
|----|------|------|------|
| D1-1 | 整理 `scripts/build-windows.sh`：固定 NSIS target、输出路径、失败即退出 | 脚本可本地/CI 跑 | 产出 setup.exe |
| D1-2 | 优先 **Windows runner 或自有 Win 机** 构建 msvc（签名可后置） | 构建 runbook | 干净机安装成功 |
| D1-3 | 生成 `SHA256SUMS` + `latest.json` 的脚本步骤 | `scripts/package-desktop-release.sh` | 一键生成三件套 |
| D1-4 | （可选）代码签名 Authenticode | 证书 + signtool | SmartScreen 误报下降 |

**依赖**：WebView2 运行时（Win10/11 通常自带；页上注明缺失时安装）。

### Wave 2 — 发布托管（0.5–1 天）

| ID | 任务 | 产出 | 验收 |
|----|------|------|------|
| D2-1 | VPS：`/var/www/releases/desktop/` + nginx `location /releases/desktop/` | 静态可下 | curl 200 + Content-Length |
| D2-2 | `Cache-Control`：`latest.json` 短缓存（如 60s）；版本目录 `immutable` 长缓存 | nginx 片段 | 发版后 latest 更新快 |
| D2-3 | 上传脚本：`scripts/publish-desktop-release.sh`（scp/rsync 到 VPS） | 一键发布 | 新版本 URL 可访问 |
| D2-4 | 备份：旧版本目录保留至少 N 个 minor | 目录策略 | 可回滚下载 |

### Wave 3 — 网页下载 UX（1 天）

| ID | 任务 | 产出 | 验收 |
|----|------|------|------|
| D3-1 | `/desktop` 增加主按钮「下载 Windows」 | `page.tsx` + i18n | 可见可点 |
| D3-2 | 客户端组件拉取 `/releases/desktop/latest.json`（失败 fallback 文案） | `DesktopDownloadButton` | 展示版本/大小 |
| D3-3 | `/desktop/buy` 成功区增加下载入口 | buy page | 购买后可装 |
| D3-4 | 安装说明 3 步 + 故障：SmartScreen、WebView2 | 文案块 | 支持自助 |
| D3-5 | 单元测试：latest.json 解析 / fallback | vitest | CI/本地绿 |
| D3-6 | 部署 frontend standalone 到 VPS | 与现 app 部署一致 | 公网可见 |

### Wave 4 — 端到端与运维硬化（0.5–1 天）

| ID | 任务 | 产出 | 验收 |
|----|------|------|------|
| D4-1 | 人工 E2E：下载 → 安装 → 启动 → buy 激活 deep link | checklist | 全绿 |
| D4-2 | 下载可用性监控（可选）：cron curl latest + setup URL | 告警或日志 | 挂了能发现 |
| D4-3 | 安全：安装包仅 HTTPS；禁止目录列表；校验 SHA 展示 | 配置 | 抽查 |
| D4-4 | 文档：`docs/desktop/` 增加「发版与下载」runbook | md | 可照做发版 |

### Wave 5（可选）— 自动更新

| ID | 任务 | 说明 |
|----|------|------|
| D5-1 | Tauri updater 插件 + 签名更新清单 | 与 `latest.json` 或独立 update manifest 对齐 |
| D5-2 | 应用内「检查更新」 | 非网页闭环必需 |

---

## 5. 前端实现要点（Wave 3 技术设计）

### 5.1 组件草图

```tsx
// DesktopDownloadButton（client）
// 1. useEffect fetch('/releases/desktop/latest.json')
// 2. 成功：按钮 href = platform.url，副文案 v{version} · {size}
// 3. 失败：显示「安装包暂未发布」或 fallback 常量（仅预发环境）
```

### 5.2 同域 vs 跨域

- 安装包与 `latest.json` 放在 **app 同域** `/releases/...`，无 CORS 问题。  
- 若改 CDN 跨域：给 `latest.json` 配 CORS GET。

### 5.3 SEO / 爬虫

- 下载链用真实 `<a href>`，勿仅 JS blob，便于校验与右键另存。

### 5.4 i18n

- 新增 key：`desktop.downloadWindows`、`desktop.versionLabel`、`desktop.sha256`、`desktop.requirements`、`desktop.installSteps*` 等（`lib/i18n/messages`）。

---

## 6. 构建与目标约定

| 环境 | 推荐 |
|------|------|
| 正式发布包 | **Windows 主机 + msvc**（或 GitHub `windows-latest` workflow_dispatch） |
| Linux 交叉 gnu | 仅 dev/smoke；SmartScreen/依赖差异大，不作为唯一正式源 |
| 签名 | Phase 1 可无；公开推广前尽量上 |

`scripts/build-windows.sh` 与正式发版脚本职责分离：

- `build-windows.sh`：编译  
- `package-desktop-release.sh`：收集 bundle、算 hash、写 `latest.json`  
- `publish-desktop-release.sh`：上传 VPS `/var/www/releases/desktop/`

---

## 7. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| 未签名 EXE 被 SmartScreen 拦截 | 用户不敢装 | 页上写明「仍要运行」；尽快代码签名 |
| 安装包过大 | 带宽/存储 | strip release；版本目录清理策略 |
| `latest.json` 缓存过久 | 用户下到旧包 | 短 max-age + 发版后 purge CF（若启用） |
| deep link 无客户端 | 激活失败 | buy 页强提示先下载 |
| 误把 debug 包发布 | 体积/安全 | 发布脚本只收 release bundle |
| Solo 无 Win 机构建 | 发不出包 | 租用临时 Win CI 或自有 Win 机 checklist |

---

## 8. 明确不做（本计划范围外）

- macOS / Linux 正式包（可在 UI 占位）  
- 改动 SaaS 计费模型（仍 Creem 买断 desktop tier）  
- 强制登录才允许下载  
- 用 GitHub Actions 每次 push 自动发版（避免 CI 额度；仅用 `workflow_dispatch` 可选）  
- 把桌面核心逻辑重写为 Electron  

---

## 9. 建议实施顺序（Solo，约 3–5 天）

```text
Day 1     Wave 0 + Wave 1：打出可安装 NSIS + SHA256 + latest.json
Day 1–2   Wave 2：VPS /releases/desktop 托管 + 上传脚本
Day 2–3   Wave 3：/desktop 与 /desktop/buy UI + 部署 app 前端
Day 3     Wave 4：真机安装 + 激活 deep link 验收
之后      Wave 5 自动更新（可选）
```

---

## 10. 验收清单（发布日）

- [ ] `GET /releases/desktop/latest.json` 返回正确 version 与 sha256  
- [ ] 主按钮下载的 setup.exe 可在干净 Win10/11 安装并启动  
- [ ] SHA256 与页面展示一致  
- [ ] `/desktop/buy` 购买成功后可复制 key，deep link 在已安装机可激活  
- [ ] 未安装时 deep link 失败有文案引导回下载  
- [ ] app / blog / canju 等其他站点无回归（仅增静态目录与前端文案）  
- [ ] 发版 runbook 可被第二次发版复用  

---

## 11. 关键文件（实施时）

| 区域 | 文件 |
|------|------|
| 构建 | `scripts/build-windows.sh`、`desktop/src-tauri/tauri.conf.json`、`desktop/package.json` |
| 发布脚本 | 新建 `scripts/package-desktop-release.sh`、`scripts/publish-desktop-release.sh` |
| 网页 | `frontend_next/app/(marketing)/desktop/page.tsx`、`…/buy/page.tsx`、`components/desktop/*`、i18n |
| 运维 | VPS nginx + `/var/www/releases/desktop/` |
| 文档 | `docs/desktop/VERSIONING.md` 或本文件 + 发版 runbook |

---

## 12. 决策记录（默认，实施前可改）

| # | 决策 | 默认 |
|---|------|------|
| 1 | 安装包免费下、授权另购 | 是 |
| 2 | 托管 | 同域 `/releases/desktop/`（VPS 静态） |
| 3 | 主格式 | NSIS setup.exe |
| 4 | 版本发现 | `latest.json` |
| 5 | 自动更新 | 二期 |
| 6 | 代码签名 | 二期（公开推广前尽量上） |

---

**文档结束。** 确认 §12 决策后，可从 Wave 0–1 开干（先打出可安装包并上传，再改网页按钮）。
