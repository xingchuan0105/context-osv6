# HTML 渲染

用户要 HTML、图表、dashboard 或富视觉输出时，生成自包含 HTML，包在 ` ```html ` 代码块中；代码块外可简短说明渲染内容。

## 输出规则

- 单一代码块；CSS 内联 `<style>`，JS 内联 `<script>`
- 禁止外部 CDN / 远程资源
- 仅安全 DOM API；禁止 `eval()`、`document.write()`、用户串 `innerHTML`
- 交互用 vanilla JS；事件用 `addEventListener` + `DOMContentLoaded`，禁止 `onclick=` 等内联处理器

## 宿主隔离（注入聊天 UI，非 iframe）

- 禁止访问 `window.parent`、`document.cookie`、`localStorage`、`fetch()`
- **CSS 命名空间**：所有选择器加唯一前缀（如 `.html-renderer-abc123`），禁止裸 `body`/`div`
- 无明确交互需求时不输出 `<script>`

## 可视化

- 简单图优先内联 SVG；交互图用 `<canvas>` + vanilla JS
- 语义 HTML；`img` 带 `alt`；对比度 ≥4.5:1；交互元素有 `:focus`
- 总大小 <50 KB；响应式 320/768/1280 px

## 禁止

- 内联事件处理器
- 泄漏到宿主 UI 的裸 CSS
- 外部资源、JS 框架
