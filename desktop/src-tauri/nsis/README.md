# NSIS installer chrome (Context-OS Client)

Used by `tauri.conf.json` → `bundle.windows.nsis`.

| Path | Purpose |
|------|---------|
| `installer.nsi` | Custom template (Tauri upstream + welcome/finish/license LangString hooks) |
| `LICENSE.txt` | License agreement page (UTF-8 BOM, EN + 中文) |
| `languages/English.nsh` | Tauri + `COS_*` MUI strings (EN) |
| `languages/SimpChinese.nsh` | Tauri + `COS_*` MUI strings (中文) |
| `../icons/nsis/header.bmp` | 150×57 installer header |
| `../icons/nsis/sidebar.bmp` | 164×314 welcome/finish sidebar |
| `../icons/nsis/uninstaller-header.bmp` | 150×57 uninstaller header |
| `../icons/nsis/installer.ico` | Installer / uninstaller icon |

## Welcome / finish copy (中文)

- 欢迎标题：`欢迎使用 Context-OS 客户端`
- 欢迎正文：本机知识库与 Vector Graph RAG；许可优先，核心使用无需云登录
- 完成页：默认勾选 **「立即启动 Context-OS 客户端」**（`MUI_FINISHPAGE_RUN` + `COS_FINISH_RUN`）

## Rebuild installer

```bash
bash scripts/regen-desktop-icons.sh   # optional icon refresh
bash scripts/build-windows.sh
```

When upgrading Tauri major versions, re-diff `installer.nsi` against upstream:

`https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi`
