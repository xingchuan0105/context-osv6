# Desktop 版本线

> ADR 0006 §9：桌面与云端 **独立版本线**。本文定义桌面侧版本与（可选）云 API 兼容表达方式。  
> 商业叙事以 **ADR-0010** 为准（客户端免费；收费在云端分享名额 / 钱包）。

## 版本号

- 采用 **SemVer**：`MAJOR.MINOR.PATCH`（与应用商店/安装包展示一致）。  
- **MAJOR**：破坏性变更（协议、本地数据格式、最低 OS）。  
- **MINOR**：向后兼容功能（含商业叙事与壳能力）。  
- **PATCH**：缺陷与安全修复。  

云端 SaaS **不共享** 该版本号序列。

## 发布产物

| 产物 | 位置 / 约定 |
|------|-------------|
| Desktop release notes | `docs/release/desktop/YYYY-MM-DD-vX.Y.Z.md` |
| 发版设计短文（可选） | `docs/desktop/YYYY-MM-DD-vX.Y.Z-*.md` |
| 安装包 checksum / 签名 | 发版流水线产物，不入库（`dist/desktop-release/` gitignored） |

### 网页下载

| 产物 | 命名 / URL |
|------|------------|
| 版本单一事实 | `desktop/package.json` 与 `desktop/src-tauri/tauri.conf.json`（及 `Cargo.toml`）的 `version`（须同步） |
| Windows 安装包（优先 NSIS） | `Context-OS-Client_{version}_x64-setup.exe` |
| Windows 便携（无 NSIS 时） | `Context-OS_{version}_x64.exe`（`ALLOW_PORTABLE=1`） |
| 校验 | `SHA256SUMS`（同目录） |
| 发现 | 公网 `GET /releases/desktop/latest.json` |
| 版本化目录 | `/releases/desktop/v{version}/…`（长缓存） |
| 发版脚本 | `scripts/package-desktop-release.sh`、`scripts/publish-desktop-release.sh` |
| Runbook | `docs/desktop/RELEASE-AND-DOWNLOAD.md` |

`latest.json` 由打包脚本生成，网页只读该文件渲染「下载 Windows」按钮。

## 最低兼容云 API（可选连云）

当桌面构建包含连云能力时，每个 **MINOR** 发版说明须声明：

```text
Min cloud API: v1
```

| Desktop | Min cloud API | 备注 |
|---------|---------------|------|
| 0.1.x | v1 | 许可/Keygen 叙事时代（已退役） |
| **0.2.x** | **v1** | **免费客户端**；云端分享名额 + 钱包 + BYOK（ADR-0010） |

未连云 / 纯本地模式：本表不适用。

## Changelog 必备段

1. 用户可见变更  
2. 已知问题  
3. 最低 OS / 架构  
4. Min cloud API（若适用）  
5. 许可 / 激活变更（**0.2+：声明免费、无激活闸**）  

## 变更

| 日期 | 说明 |
|------|------|
| 2026-07-09 | 初稿（ADR 0006 backlog #4） |
| 2026-07-14 | 增加网页下载产物命名、`latest.json` 与发版脚本约定 |
| 2026-08-10 | v0.2.0 矩阵；命名 Context-OS-Client；ADR-0010 免费客户端 |
