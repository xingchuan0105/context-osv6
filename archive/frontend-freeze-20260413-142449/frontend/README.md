## Status

这个 `frontend/` 目录是 legacy/reference Next.js 前端。

当前正式前端为 `../frontend_rust/`，新功能和 API 对齐优先在那里落地。

## Frontend Runtime Requirements

- Node.js: `20.x`, `22.x`, or `24.x` (recommended: `24.x`)
- npm: `10+`
- This repo provides `.nvmrc` pinned to `24`.

## Getting Started

```bash
nvm use
npm install
```

Then run the development server:

```bash
npm run dev
```

Open [http://localhost:3000](http://localhost:3000) with your browser to see the result.

For the Rust `avrag-rs` backend in `context-osv6`, the default local target is:

- `NEXT_PUBLIC_API_URL=http://127.0.0.1:8080`
- `BACKEND_URL=http://127.0.0.1:8080`

For the integrated local stack (JWT + backend on `38080` + frontend on `3000`), use:

```bash
cd ..
./scripts/dev-stack-up.sh
```

This flow writes `frontend/.env.local` with:
- `NEXT_PUBLIC_API_URL=http://127.0.0.1:38080`
- `BACKEND_URL=http://127.0.0.1:38080`
- 登录态通过 `/api/auth/register` 或 `/api/auth/login` 获取，不再由前端脚本注入 dev bearer token

To stop the local stack:

```bash
cd ..
./scripts/dev-stack-down.sh
```

## Build

```bash
npm run build
```

If you see Node version errors, switch to `24.x` and reinstall dependencies.

## Notes

- CI validates both Node `22` and `24` for regression prevention.
- API requests are proxied by `next.config.ts` to backend targets for `/api`, `/v1`, `/mcp`.
- Rust M1+M2 alignment notes live in `frontend/docs/rust-api-alignment.md`.
- `frontend/.env.example` contains both generic local values and the Rust `avrag-rs` defaults for Wave 0/Wave 1 development.
- If build fails with `EACCES` under `~/.cache/next-swc`, run with project-local cache:
  `XDG_CACHE_HOME=../.cache npm run build`
