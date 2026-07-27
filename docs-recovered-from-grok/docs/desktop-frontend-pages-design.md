# 桌面端前端独立页面设计

> 日期：2026-07-08　状态：Proposed

---

## 1. 设计原则

桌面端的激活和配置页面**不与 SaaS workspace 混在一起**，使用独立的 route group `(desktop)`，有自己的布局和守卫。

核心原则：
1. **路由隔离**：`(desktop)` route group，不进入 `(app)` 的 `ProtectedRouteGate`
2. **环境守卫**：`DesktopOnlyGate`——非 Tauri 环境重定向到 `/dashboard`
3. **居中引导布局**：无侧边栏，纯引导流
4. **IPC 传输**：全部走 `invoke()` Tauri IPC，不碰 HTTP
5. **SaaS 页面分离**：购买/管理 license 在 SaaS 的 `(marketing)` / `(account)` group

---

## 2. 路由结构

现有 route groups：

```
frontend_next/app/
├── (app)/          # SaaS 工作区（ProtectedRouteGate）
├── (auth)/         # 登录注册
├── (marketing)/    # 落地页、定价
├── dashboard/      # SaaS dashboard
├── settings/       # SaaS 设置
└── ...
```

新增：

```
frontend_next/app/
├── (desktop)/                  # 新增：桌面端专用，独立 route group
│   ├── layout.tsx              # DesktopOnlyGate + DesktopCenterLayout
│   ├── activate/
│   │   └── page.tsx            # 激活引导（试用/输入key/购买链接）
│   └── setup/
│       └── page.tsx            # LLM 配置引导（选 provider → 填 key → 测试）
│
├── (account)/                  # 新增：SaaS 账户管理（浏览器访问）
│   └── licenses/
│       ├── page.tsx            # License 列表
│       └── [id]/
│           └── page.tsx        # 详情（已激活设备 + 解绑）
│
├── (marketing)/
│   ├── pricing/                # 现有
│   └── desktop/                # 新增
│       ├── page.tsx            # Desktop 产品介绍
│       └── buy/
│           └── page.tsx        # 购买页（带 ?device_id= 参数）
```

---

## 3. `(desktop)` Route Group

### 3.1 Layout

```tsx
// app/(desktop)/layout.tsx
"use client";

import { ReactNode } from "react";
import { DesktopOnlyGate } from "@/components/desktop/DesktopOnlyGate";
import { DesktopCenterLayout } from "@/components/desktop/DesktopCenterLayout";

export default function DesktopLayout({ children }: { children: ReactNode }) {
  return (
    <DesktopOnlyGate>
      <DesktopCenterLayout>
        {children}
      </DesktopCenterLayout>
    </DesktopOnlyGate>
  );
}
```

`DesktopOnlyGate` 逻辑：

```tsx
// components/desktop/DesktopOnlyGate.tsx
"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { isTauri } from "@/lib/runtime/tauri-ipc";

export function DesktopOnlyGate({ children }: { children: ReactNode }) {
  const router = useRouter();
  const [checked, setChecked] = useState(false);

  useEffect(() => {
    if (!isTauri()) {
      router.replace("/dashboard");
      return;
    }
    setChecked(true);
  }, [router]);

  if (!checked) return null;
  return <>{children}</>;
}
```

`DesktopCenterLayout`——居中卡片布局，无 workspace 侧边栏：

```tsx
// components/desktop/DesktopCenterLayout.tsx
export function DesktopCenterLayout({ children }: { children: ReactNode }) {
  return (
    <div className="flex min-h-screen items-center justify-center bg-warm-white p-8">
      <div className="w-full max-w-lg">
        {children}
      </div>
    </div>
  );
}
```

### 3.2 激活页 `/(desktop)/activate`

**入口条件**：桌面端启动时检测 `license_status`，非 `active`/`trial` 则重定向到此页。

**三个视图**：

#### 视图 1：选择（默认）

```
┌────────────────────────────────────────────────────────┐
│                    AVRag Desktop                        │
│                                                        │
│            ┌────────────────────────┐                  │
│            │     [应用 Logo]         │                  │
│            └────────────────────────┘                  │
│                                                        │
│        欢迎使用 AVRag Desktop                            │
│        请选择激活方式                                    │
│                                                        │
│   ┌──────────────────┐    ┌──────────────────┐         │
│   │  开始 7 天试用     │    │  我已有授权码     │         │
│   │  全功能免费体验    │    │                  │         │
│   │  无需信用卡       │    │                  │         │
│   └──────────────────┘    └──────────────────┘         │
│                                                        │
│         还没有授权？ [购买授权]  [查看帮助]               │
│                                                        │
└────────────────────────────────────────────────────────┘
```

#### 视图 2：输入授权码

```
┌────────────────────────────────────────────────────────┐
│              输入授权码                                  │
│                                                        │
│   ┌──────────────────────────────────────────────┐     │
│   │  AVRG-XXXX-XXXX-XXXX-XXXX                     │     │
│   └──────────────────────────────────────────────┘     │
│                                                        │
│              [取消]         [激活]                       │
│                                                        │
│   本机设备 ID: a1b2c3d4...（用于绑定）                    │
└────────────────────────────────────────────────────────┘
```

#### 视图 3：激活成功

```
┌────────────────────────────────────────────────────────┐
│              ✓ 激活成功                                  │
│                                                        │
│   产品: AVRag Desktop Pro                              │
│   授权: 永久（v1.x 终身免费升级）                          │
│   设备: 1/3 已激活                                       │
│                                                        │
│         [配置 LLM 模型]  [开始使用]                       │
│                                                        │
└────────────────────────────────────────────────────────┘
```

**实现**：

```tsx
// app/(desktop)/activate/page.tsx
"use client";

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useState, useEffect } from "react";
import { useRouter } from "next/navigation";

type View = "choice" | "input" | "success" | "error";

export default function ActivatePage() {
  const router = useRouter();
  const [view, setView] = useState<View>("choice");
  const [licenseKey, setLicenseKey] = useState("");
  const [deviceId, setDeviceId] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    invoke<string>("get_device_id").then(setDeviceId).catch(() => {});

    // 监听深链（从浏览器购买后跳回）
    const unlisten = listen<string>("deep-link-activate", (e) => {
      setLicenseKey(e.payload);
      setView("input");
    });
    return () => { unlisten.then((f) => f()); };
  }, []);

  async function startTrial() {
    setLoading(true);
    try {
      await invoke("start_trial");
      router.push("/setup");
    } catch (e) {
      setError(String(e));
      setView("error");
    } finally {
      setLoading(false);
    }
  }

  async function activate() {
    setLoading(true);
    try {
      const result = await invoke("activate_license", { licenseKey });
      setView("success");
    } catch (e) {
      setError(String(e));
      setView("error");
    } finally {
      setLoading(false);
    }
  }

  async function openBuyPage() {
    await invoke("open_in_browser", {
      url: `https://app.avrag.com/desktop/buy?device_id=${deviceId}`,
    });
  }

  async function openHelp() {
    await invoke("open_in_browser", {
      url: "https://app.avrag.com/help/desktop-activation",
    });
  }

  // ... render based on view
}
```

### 3.3 LLM 配置引导页 `/(desktop)/setup`

激活成功后引导配置 LLM（首次必经路径，降低配置门槛）：

#### 步骤 1：选择 Provider

```
┌────────────────────────────────────────────────────────┐
│              配置 AI 模型                       1/3     │
│                                                        │
│   选择你的 AI 服务商:                                    │
│                                                        │
│   ┌────────────┐ ┌────────────┐ ┌────────────┐         │
│   │  智谱 GLM   │ │  Anthropic │ │  DeepSeek  │         │
│   │ Coding Plan │ │  Claude    │ │            │         │
│   │  ¥20/月     │ │ $3-15/M    │ │ ¥1-8/M     │         │
│   └────────────┘ └────────────┘ └────────────┘         │
│   ┌────────────┐ ┌────────────┐ ┌────────────┐         │
│   │  OpenAI    │ │  Gemini    │ │ SiliconFlow│         │
│   │ $2.5-15/M  │ │ 免费额度    │ │ ¥1-4/M     │         │
│   └────────────┘ └────────────┘ └────────────┘         │
│   ┌────────────┐ ┌────────────┐ ┌────────────┐         │
│   │  通义千问   │ │  本地Ollama │ │  自定义    │         │
│   │ ¥0.8-4/M   │ │ 免费       │ │            │         │
│   └────────────┘ └────────────┘ └────────────┘         │
│                                                        │
│                              [跳过, 稍后配置]            │
└────────────────────────────────────────────────────────┘
```

选了某个 provider 后，自动填充 base_url 和 model（从 `LLM_PRESETS`）。

#### 步骤 2：填写 API Key

```
┌────────────────────────────────────────────────────────┐
│              配置 AI 模型                       2/3     │
│                                                        │
│   智谱 GLM（含 Coding Plan）                             │
│                                                        │
│   API Key:  [•••••••••••••••••••••]  [申请 →]          │
│   Model:    [glm-4.6 ▼]                                │
│   Base URL: https://open.bigmodel.cn/api/paas/v4       │
│                                                        │
│              [上一步]  [测试连接]                        │
└────────────────────────────────────────────────────────┘
```

"申请 →" 按钮调 `invoke("open_in_browser", { url: preset.api_key_url })`。

#### 步骤 3：测试连接

```
┌────────────────────────────────────────────────────────┐
│              配置 AI 模型                       3/3     │
│                                                        │
│   ┌──────────────────────────────────────────┐         │
│   │ ✓ 连接成功！延迟 120ms，模型可用            │         │
│   └──────────────────────────────────────────┘         │
│   或                                                   │
│   ┌──────────────────────────────────────────┐         │
│   │ ✗ 连接失败                                │         │
│   │ [运行诊断] ← 打开完整诊断面板               │         │
│   └──────────────────────────────────────────┘         │
│                                                        │
│              [完成配置]                                 │
└────────────────────────────────────────────────────────┘
```

---

## 4. 桌面端运行时入口（顶栏）

激活和配置完成后，在桌面端顶栏加独立入口——**不是 `/settings` 子路由，而是 Tauri 窗口内弹出的面板（modal/drawer）**：

```
桌面端顶栏:
┌────────────────────────────────────────────────────────┐
│ AVRag Desktop    [workspace 下拉]    [⚙️] [⚡状态]      │
│                                        │     │          │
│                          点击 ⚙️ 弹出:  │     │          │
│                          ┌──────────┐ │     │          │
│                          │ AI 模型   │ │     │          │
│                          │ 授权管理   │ │     │          │
│                          │ 诊断工具   │ │     │          │
│                          └──────────┘       │          │
│                                              │          │
│                          点击 ⚡ 显示状态:    │          │
│                          ┌──────────┐        │          │
│                          │✓ 已激活   │        │          │
│                          │ Pro 永久  │        │          │
│                          │ 1/3 设备 │        │          │
│                          └──────────┘        │          │
└────────────────────────────────────────────────────────┘
```

### 4.1 状态徽章组件

```tsx
// components/desktop/DesktopStatusBadge.tsx
"use client";

import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

type LicenseStatus = {
  kind: "active" | "trial" | "expired" | "revoked" | "unactivated" | "upgrade_required";
  expires_at?: number;
  max_seats?: number;
  days_remaining?: number;
};

export function DesktopStatusBadge() {
  const [status, setStatus] = useState<LicenseStatus | null>(null);

  useEffect(() => {
    const check = () => invoke<LicenseStatus>("get_license_status").then(setStatus);
    check();
    const interval = setInterval(check, 60_000);  // 每分钟刷新
    return () => clearInterval(interval);
  }, []);

  if (!status) return null;

  const config = {
    active: { icon: "✓", color: "text-green-600", label: "已激活" },
    trial: { icon: "⏱", color: "text-orange-600", label: `试用 ${status.days_remaining}d` },
    expired: { icon: "⚠", color: "text-red-600", label: "已过期" },
    revoked: { icon: "✗", color: "text-red-600", label: "已吊销" },
    unactivated: { icon: "○", color: "text-gray-400", label: "未激活" },
    upgrade_required: { icon: "↑", color: "text-blue-600", label: "可升级" },
  }[status.kind];

  return (
    <button onClick={openStatusPanel} className={`...`}>
      <span>{config.icon}</span>
      <span className={config.color}>{config.label}</span>
    </button>
  );
}
```

### 4.2 设置弹出面板

点击 ⚙️ 弹出的面板有三个入口：

- **AI 模型** → 打开 drawer，内含 LLM 配置表单 + 诊断面板（复用 `(desktop)/setup` 的组件）
- **授权管理** → 打开 drawer，显示 license 详情 + 解绑按钮 + 跳转 SaaS 管理页
- **诊断工具** → 打开 drawer，运行完整 6 步诊断

这些面板是 Tauri 窗口内的 React 组件（modal/drawer），不走路由跳转。

---

## 5. SaaS 侧页面

### 5.1 `(marketing)/desktop/buy` — 购买页

浏览器访问，桌面端通过系统浏览器跳转到此页。

```
┌────────────────────────────────────────────────────────┐
│              AVRag Desktop                              │
│              本地 AI 知识助手                             │
│                                                        │
│   ┌──────────────────┐    ┌──────────────────┐         │
│   │  Standard         │    │  Pro              │         │
│   │  $39 / ¥299       │    │  $99 / ¥699       │         │
│   │  1 台设备         │    │  3 台设备          │         │
│   │  v1 终身免费升级   │    │  v1 终身免费升级    │         │
│   │                   │    │  优先支持          │         │
│   │  [购买]           │    │  [购买]           │         │
│   └──────────────────┘    └──────────────────┘         │
│                                                        │
│   支付方式：信用卡（Creem）/ 支付宝                       │
└────────────────────────────────────────────────────────┘
```

购买成功后显示：

```
┌────────────────────────────────────────────────────────┐
│              ✓ 购买成功                                  │
│                                                        │
│   你的授权码:                                           │
│   ┌──────────────────────────────────────────────┐     │
│   │  AVRG-XXXX-XXXX-XXXX-XXXX           [复制]    │     │
│   └──────────────────────────────────────────────┘     │
│                                                        │
│   [在 AVRag Desktop 中激活]  ← 深链按钮                  │
│                                                        │
│   或手动输入授权码激活                                    │
└────────────────────────────────────────────────────────┘
```

深链按钮：`<a href="avrag-desktop://activate?key=AVRG-XXXX-...">在 AVRag Desktop 中激活</a>`

### 5.2 `(account)/licenses` — License 管理

用户在 SaaS 管理自己的 license：

```
┌────────────────────────────────────────────────────────┐
│  我的授权                                                │
│                                                        │
│  AVRag Desktop Pro (AVRG-XXXX)                          │
│  ┌──────────────────────────────────────────────┐      │
│  │ 已激活设备 (2/3)                               │      │
│  ├──────────────────────────────────────────────┤      │
│  │ ● MBP-2023-Chuan      最后心跳: 2小时前    [解绑] │      │
│  │ ● Desktop-Office      最后心跳: 3天前      [解绑] │      │
│  │ ○ 空位                                        │      │
│  └──────────────────────────────────────────────┘      │
│                                                        │
│  授权信息                                                │
│  产品: AVRag Desktop Pro                               │
│  类型: 永久（v1.x 终身免费升级）                          │
│  购买日: 2026-07-08                                     │
└────────────────────────────────────────────────────────┘
```

解绑设备调 `DELETE /api/v1/licenses/{id}/machines/{mid}`，被解绑的设备下次心跳收到 403 自动锁定。

---

## 6. 设计风格

遵循现有 Notion-inspired design system（`DESIGN.md`）：
- Warm neutral 色板（`#f6f5f4` 背景、`rgba(0,0,0,0.95)` 文字）
- Notion Blue（`#0075de`）作为主 CTA
- Ultra-thin borders（`1px solid rgba(0,0,0,0.1)`）
- 居中卡片布局，最大宽度 `max-w-lg`
- NotionInter 字体

---

## 7. 关联

- `docs/adr/0004-desktop-hybrid-business-model.md` — 混合商业模式
- `docs/desktop-license-activation-design.md` — 授权激活（跳转闭环设计）
- `docs/desktop-llm-provider-design.md` — LLM 诊断（诊断面板 UI）
- `docs/desktop-execution-plan.md` — 总执行计划（WP5 覆盖本设计）