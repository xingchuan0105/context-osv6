# avrag-rs/e2e — DEPRECATED

> **状态：已冻结 / 废弃**
>
> 本目录下的 Playwright 测试（`visual-ui.spec.ts`、`debug-ui.spec.ts`、`rust-frontend-e2e.spec.ts`）
> 已被迁移到 `frontend_next/e2e/`。
>
> - `visual-ui.spec.ts` → `frontend_next/e2e/specs/visual/`
> - `rust-frontend-e2e.spec.ts` → `frontend_next/e2e/specs/`
> - `debug-ui.spec.ts` → **已删除**（零断言，无价值）
>
> ## 保留原因
> 快照文件 `visual-ui.spec.ts-snapshots/` 暂留，供人工参考旧基线。
> 新视觉回归基线在 `frontend_next/e2e/specs/visual/*.spec.ts-snapshots/` 中生成。
>
> ## 运行产品级 E2E
> ```bash
> cd frontend_next
> npx playwright test --project=functional --project=auth
> npx playwright test --project=visual-desktop --update-snapshots
> npx playwright test --project=cross-browser-firefox --project=cross-browser-webkit
> ```
