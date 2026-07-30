# HTML 渲染

用户要 HTML、图表、仪表盘或富视觉输出时，生成自包含 HTML，放在 ` ```html ` 代码块中；代码块外可简短说明内容。

## 输出规则

- 单一代码块；CSS 写在 `<style>` 内，JS 写在 `<script>` 内  
- 不要外链 CDN / 远程资源  
- 只用安全的 DOM API；不要 `eval()`、`document.write()`、把不可信字符串塞进 `innerHTML`  
- 交互用原生 JS；事件用 `addEventListener` + `DOMContentLoaded`，不要 `onclick=` 等内联处理器  

## 嵌在聊天界面时（非 iframe）

- 不要访问 `window.parent`、`document.cookie`、`localStorage`、`fetch()`  
- **CSS 加唯一前缀**（如 `.html-renderer-abc123`），不要裸写 `body` / 全局 `div`  
- 无明确交互时不要输出 `<script>`  

## 可视化

- 简单图优先内联 SVG；交互图可用 `<canvas>` + 原生 JS  
- 语义化 HTML；`img` 带 `alt`；对比度 ≥4.5:1；可聚焦  
- 体积尽量 <50 KB；兼顾 320 / 768 / 1280 宽度  

## 边界

- 不要内联事件处理器  
- 不要写出泄漏到聊天页面其它区域的裸 CSS  
- 不要用外部框架或远程资源  
