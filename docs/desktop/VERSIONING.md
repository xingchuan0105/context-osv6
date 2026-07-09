# Desktop 版本线

> ADR 0006 §9：桌面与云端 **独立版本线**。本文定义桌面侧版本与（可选）云 API 兼容表达方式。

## 版本号

- 采用 **SemVer**：`MAJOR.MINOR.PATCH`（与应用商店/安装包展示一致）。  
- **MAJOR**：破坏性变更（协议、本地数据格式、最低 OS）。  
- **MINOR**：向后兼容功能。  
- **PATCH**：缺陷与安全修复。  

云端 SaaS **不共享** 该版本号序列。

## 发布产物

| 产物 | 位置 / 约定 |
|------|-------------|
| Desktop release notes | `docs/release/desktop/YYYY-MM-DD-vX.Y.Z.md`（启用后） |
| Cloud release notes | `docs/release/cloud/…`（启用后） |
| 安装包 checksum / 签名 | 发版流水线产物，不入库 |

## 最低兼容云 API（可选连云）

当桌面构建包含连云能力时，每个 **MINOR** 发版说明须声明：

```text
Min cloud API: v1   # 示例；以实际 OpenAPI / route 兼容策略为准
```

| Desktop | Min cloud API | 备注 |
|---------|---------------|------|
| 1.x | v1 | 初版矩阵占位；连云稳定后由发版 PR 更新 |

未连云 / 纯本地模式：本表不适用。

## Changelog 必备段

1. 用户可见变更  
2. 已知问题  
3. 最低 OS / 架构  
4. Min cloud API（若适用）  
5. 许可 / 激活变更（若有）  

## 变更

| 日期 | 说明 |
|------|------|
| 2026-07-09 | 初稿（ADR 0006 backlog #4） |
