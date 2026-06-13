# Tauri 图标文件

此目录需要放置以下图标文件：

- `32x32.png` - 32x32 像素的 PNG 图标
- `128x128.png` - 128x128 像素的 PNG 图标
- `128x128@2x.png` - 256x256 像素的 PNG 图标（Retina）
- `icon.icns` - macOS 图标文件
- `icon.ico` - Windows 图标文件

## 生成图标

可以使用 Tauri CLI 自动生成所有尺寸的图标：

```bash
pnpm tauri icon <source-icon.png>
```

或者手动创建这些文件。

## 临时方案

在正式图标设计完成前，可以使用以下占位图标：

1. 创建一个 1024x1024 的 PNG 图标
2. 运行 `pnpm tauri icon icon.png` 自动生成所有尺寸
