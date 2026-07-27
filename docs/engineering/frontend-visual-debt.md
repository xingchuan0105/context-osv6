# 前端视觉技术债登记

视觉修复各轮完成后仍未处理的技术债，按优先级简列，供后续参考。
（最近更新：2026-07-22,Wave 1-5 + 导航/配色/布局批次之后）

## 已清零（2026-07-22 批次）

- ~~47 个文件约 489 处内联 `style={{...}}`~~ → 已批量迁移 ~380 处到 CSS Module(share 126 / admin 112 / settings 80 / desktop 37 / misc 23 + Wave 4 的 2 个文件）;保留的均为动态计算值或共享样式函数调用。
- ~~globals.css 2000+ 行堆全局~~ → 已拆分为 `app/styles/` 下 6 个 partial(`_app-shared/_marketing/_dashboard/_billing/_legal` + 瘦身后 globals.css 78 行）,`@import` 链,规则数 310=310 无丢失。
- ~~亮模式铜色小字链接对比度~~ → 色系已换靛蓝,链接统一 `--accent-text`(AA)。
- ~~caption/overline 11px 中文过小~~ → 已升 0.75rem。
- 字号刻度重复档位(overline == caption、meta == caption-strong == label)→ overline/caption 归并到 0.75rem 后实质上已同档,名义重复可接受,不再单列。

## 工程卫生

- 代码块无语法高亮（需引入高亮库，另行评估体积与 SSR 影响）。
- settings 各 panel 的 CSS Module 存在重复的通用类名（`.section`/`.mutedText` 等）,可抽共享 settings-ui 模块（低优先）。
- 字号收敛机会：离刻度字面量 `0.82rem`×14（≈`--font-size-control`)、`0.76rem`×6（≈`--font-size-meta`)、`0.92rem`×4（≈`--font-size-body-strong`)、`1.25rem`×5（刻度缺档，归 title-sm 或补令牌）。
- 间距收敛机会：只收近邻值 `0.8→--space-3(0.75)`、`0.95→--space-4(1)`，细 padding 微调值保留字面量（2026-07-22 样式体系审查结论）。
- 新拆分的 `app/styles/_dashboard.css` 内含疑似未使用的 `.brand-*`/`.placeholder-grid` 规则,待确认后清理。
- `workspace-shell.module.css` 有菜单删除后遗留的死 CSS(menuAnchor/menuPanel/menuChoice 等）。

## 交互

- 流式期间 composer 整体禁用，可考虑允许输入排队。
- 移动端 topBar 动作折行拥挤，建议收成 overflow 菜单。
- `.citationText` 移动端截断后无展开途径。
- `.thinkingIndicator` 三点动画样式已定义未使用（改用或删除）。
- 历史面板为无标题会话逐 session 拉取消息派生标题（N 会话 = N 请求）,建议后端列表接口直接返回派生标题或批量端点。
- 引用图片等异步内容加载撑高 `scrollHeight` 后无再吸底补偿（`chat-message-list.tsx`)。
- 桌面端消息操作按钮等触控目标约 30px,低于 44px 建议值。

## 排版

- 中文混排无 `text-autospace` / CJK 行高补偿。
- `codeBlock` 相关样式存在 magic 值（尺寸/间距硬编码，未走 token）。
- 暗色下禁用按钮（如 /desktop "安装包暂未发布"）为亮银色，比页面所有元素都亮——禁用态在暗色应更暗而非更亮，需调 disabled 色板（低优先）。
- 复查注意：曾有"`.choiceCard` 零引用"的误判（实际被 activate 页使用），静态死代码结论删除前必须二次 grep。
