# Desktop shell icons & NSIS chrome

Canonical mark is the **Full ContextOsMark** (dual-arc + spine + nodes), matching
`frontend_next/app/icon.svg` / `components/context-os-mark.tsx`.

| File | Use |
|------|-----|
| `icon.svg` | Source mark (slate plate) |
| `32x32.png` / `128x128.png` / `128x128@2x.png` | Tauri bundle icons |
| `icon.ico` | Windows app / shell |
| `nsis/installer.ico` | NSIS installer + uninstaller icon |
| `nsis/header.bmp` | 150×57 installer page header |
| `nsis/sidebar.bmp` | 164×314 welcome/finish sidebar |
| `nsis/uninstaller-header.bmp` | 150×57 uninstaller header (muted bar) |
| `nsis/preview-install-ui.png` | Design QA only (not bundled) |

Installer license + language strings: see `desktop/src-tauri/nsis/`.

## Regenerate

```bash
# From repo root (needs rsvg-convert + pillow)
rsvg-convert -w 256 -h 256 desktop/src-tauri/icons/icon.svg -o /tmp/cos-256.png
# Or re-run the generator used in the branding wave (see git history / scripts).
```

After changing `icon.svg`, re-export PNG/ICO/BMP and rebuild the Windows installer:

```bash
bash scripts/build-windows.sh
```

Product display name: **Context-OS Client** (`tauri.conf.json` → `productName` / window title).
