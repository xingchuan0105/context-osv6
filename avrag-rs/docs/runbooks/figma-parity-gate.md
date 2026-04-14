# Figma Parity Gate (Design Drift 防护)

目标：把 Figma 定稿与 `frontend_rust` 预览页面做固定口径截图对比，量化漂移并在超阈值时直接失败。

## 目录约定

- Figma 期望图：`frontend_rust/.run/visual_compare/figma/*.png`
- 前端实拍图：`frontend_rust/.run/visual_compare/playwright/*.png`
- 分析报告：`frontend_rust/.run/visual_compare/analysis/{summary.json,report.md}`

页面与节点映射：

- [figma-parity-map.json](/home/chuan/context-osv6/avrag-rs/docs/runbooks/figma-parity-map.json)

## 1) 先更新 Figma 期望截图

按映射文件中的 `figmaNodeId` 导出以下文件名，放到 `frontend_rust/.run/visual_compare/figma/`：

- `login.png`
- `dashboard.png`
- `workspace.png`
- `account.png`
- `settings.png`
- `help.png`

说明：

- 固定视口 `1440x1024`。
- 导出时不要裁切到内容高度，保持与前端截图同一口径。

## 2) 一键执行 Gate

在 `avrag-rs` 下运行：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\run_figma_parity.ps1 -Threshold 0.02 -WriteDiffImages
```

阈值解释：

- `0.02` = 2% 像素差异上限
- 任意页面超过阈值，脚本返回非 0，适合接 CI 门禁

## 3) 单步执行（可调试）

```powershell
$env:PARITY_BASE_URL="http://127.0.0.1:4173"
python .\scripts\capture_preview_pages.py

python .\scripts\compare_figma_playwright.py `
  --expected-dir ..\frontend_rust\.run\visual_compare\figma `
  --actual-dir ..\frontend_rust\.run\visual_compare\playwright `
  --out-dir ..\frontend_rust\.run\visual_compare\analysis `
  --threshold 0.02 `
  --write-diff-images
```

## 4) 如何读报告

查看：

- [report.md](/home/chuan/context-osv6/frontend_rust/.run/visual_compare/analysis/report.md)
- [summary.json](/home/chuan/context-osv6/frontend_rust/.run/visual_compare/analysis/summary.json)

优先看 `report.md` 中 ratio 最高的页面，按顺序修：

1. 布局结构（容器宽度、间距、分栏比例）
2. 文本层级（字号、行高、字重）
3. 色彩 token（背景、边框、主按钮）
4. 交互状态（hover/focus/loading/empty/error）
