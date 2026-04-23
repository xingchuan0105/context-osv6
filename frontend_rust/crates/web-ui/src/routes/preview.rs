//! Pixel-faithful preview pages mapped to Figma Hi-Fi nodes.

use leptos::prelude::*;
use leptos_router::components::{A, Redirect};

const FRAME_STYLE: &str = "transform: scale(min(1, min(calc(100vw / 1440), calc(100vh / 1024)))); transform-origin: center center; font-family: 'SF Pro Display','PingFang SC','Noto Sans SC','Inter','Segoe UI','Microsoft YaHei UI',sans-serif; letter-spacing: 0.01em;";

#[component]
fn CanvasFrame(children: Children) -> impl IntoView {
    view! {
        <div class="flex h-screen w-screen items-center justify-center overflow-hidden bg-[#fdfdfe]">
            <div class="relative h-[1024px] w-[1440px] overflow-hidden rounded-[16px] border-2 border-[#ccd6e5] bg-white text-[#1f2638] antialiased" style=FRAME_STYLE>
                {children()}
            </div>
        </div>
    }
}

#[component]
fn BrainLogo(size: &'static str) -> impl IntoView {
    view! {
        <div class={format!("inline-flex items-center justify-center rounded-[8px] bg-[#0b0f14] {}", size)}>
            <svg viewBox="0 0 64 64" class="h-[76%] w-[76%] text-white" fill="none" stroke="currentColor">
                <path d="M16 22c0-7.2 5.8-13 13-13h6c7.2 0 13 5.8 13 13v20c0 7.2-5.8 13-13 13h-6c-7.2 0-13-5.8-13-13z" stroke-width="2.8"/>
                <path d="M21 28c6.5 0 9 5.5 11 12 2-6.5 4.5-12 11-12" stroke-width="2.8" stroke-linecap="round"/>
                <path d="M24 44h16" stroke-width="2.8" stroke-linecap="round"/>
                <circle cx="32" cy="20" r="2.3" fill="currentColor" stroke="none"/>
            </svg>
        </div>
    }
}

#[component]
fn AvatarButton(href: &'static str) -> impl IntoView {
    view! {
        <A href=href attr:class="absolute left-[1406px] top-[19px] block size-[28px] rounded-[14px] border border-[#d1dbe8] bg-[#f5f7fc]">
            <span class="absolute left-[10px] top-[6px] block h-[8px] w-[8px] rounded-full bg-[#1c2638]"></span>
            <span class="absolute left-[8px] top-[15px] block h-[8px] w-[12px] rounded-[4px] bg-[#1c2638]"></span>
        </A>
    }
}

#[component]
fn PreviewDashboardRow(
    title: &'static str,
    sources: &'static str,
    date: &'static str,
    active: bool,
) -> impl IntoView {
    view! {
        <div
            class="grid grid-cols-12 items-center gap-4 border-b border-[#f3f4f6] px-4 py-3.5 transition-colors"
            class=("bg-[#fafafa]", active)
        >
            <div class="col-span-6 truncate pr-4 text-[14.5px] font-medium text-[#27272a]">
                {title}
            </div>
            <div class="col-span-2 text-[14px] text-[#71717a]">{sources}</div>
            <div class="col-span-2 text-[14px] text-[#71717a]">{date}</div>
            <div class="col-span-2 flex items-center justify-between">
                <span class="text-[14px] text-[#71717a]">{"Owner"}</span>
                <span class="text-[18px] leading-none text-[#a1a1aa]">{"⋮"}</span>
            </div>
        </div>
    }
}

#[component]
fn PreviewWorkspaceThreadRow(title: &'static str, active: bool) -> impl IntoView {
    view! {
        <button
            type="button"
            class="flex w-full items-center rounded-[10px] px-3 py-2 text-left text-[14px] font-medium text-[#334155] transition-colors"
            class=("bg-[#e8edf6] text-[#22324d]", active)
            class=("hover:bg-[#eef2f7]", !active)
        >
            <span class="truncate">{title}</span>
        </button>
    }
}

#[component]
fn PreviewWorkspaceSourceRow(title: &'static str, checked: bool) -> impl IntoView {
    view! {
        <div class="flex items-center gap-3 rounded-[10px] border border-[#e7ebf2] bg-white px-3 py-2.5">
            <span
                class="inline-flex h-[15px] w-[15px] shrink-0 items-center justify-center rounded-[3px] border border-[#6b7a90] bg-white text-[11px] font-semibold leading-none text-[#334155]"
            >
                {if checked { "✓" } else { "" }}
            </span>
            <span class="truncate text-[14px] font-medium text-[#314154]">{title}</span>
        </div>
    }
}

#[component]
fn PreviewWorkspaceNoteCard(title: &'static str, preview: &'static str) -> impl IntoView {
    view! {
        <div class="rounded-[12px] border border-[#e7ebf2] bg-white px-3.5 py-3 shadow-[0_1px_0_rgba(15,23,42,0.02)]">
            <div class="text-[13px] font-semibold leading-[1.35] text-[#1f2d3d]">{title}</div>
            <p class="mt-2 line-clamp-2 text-[12px] leading-[1.45] text-[#7a8796]">{preview}</p>
        </div>
    }
}

#[component]
fn WorkspaceChrome(#[prop(optional)] children: Option<Children>) -> impl IntoView {
    view! {
        <CanvasFrame>
            <div class="absolute left-[-2px] top-[-2px] h-[72px] w-[1440px] border border-[#e3ebf2] bg-white"></div>
            <div class="absolute left-[-2px] top-[70px] h-[952px] w-[276px] border border-[#e3ebf2] bg-[#f9f9fb]"></div>
            <div class="absolute left-[274px] top-[70px] h-[952px] w-[824px] border border-[#e3ebf2] bg-[#f9f9fb]"></div>
            <div class="absolute left-[1098px] top-[70px] h-[952px] w-[340px] border border-[#e3ebf2] bg-[#f9f9fb]"></div>

            <A href="/preview/dashboard" attr:class="absolute left-[46px] top-[22px] block size-[32px]">
                <BrainLogo size="h-[32px] w-[32px]" />
            </A>
            <A href="/preview/dashboard" attr:class="absolute left-[94px] top-[21px] text-[27px] font-semibold tracking-[-0.01em] text-[#12141c]">
                {"Context-OS"}
            </A>
            <p class="absolute left-[292px] top-[24px] text-[19px] font-medium leading-[1.25] text-[#1f2638]">{"Research Project Alpha"}</p>

            <button class="absolute left-[1068px] top-[16px] h-[36px] w-[96px] rounded-[18px] border border-[#d9e3f0] bg-white text-[15px] font-medium text-[#1f2638]">
                {"Share"}
            </button>
            <button class="absolute left-[1176px] top-[16px] h-[36px] w-[84px] rounded-[18px] border border-[#d9e3f0] bg-white text-[15px] font-medium text-[#1f2638]">
                {"API"}
            </button>
            <A href="/preview/settings" attr:class="absolute left-[1272px] top-[16px] inline-flex h-[36px] w-[130px] items-center justify-center rounded-[18px] border border-[#d9e3f0] bg-[#0a1f47] text-[15px] font-medium text-white">
                {"Settings"}
            </A>
            <AvatarButton href="/preview/account" />

            <button class="absolute left-[14px] top-[92px] h-[46px] w-[244px] rounded-[23px] border border-[#e3ebf2] bg-[#0a1f47] text-[15px] font-semibold text-white">
                {"+ New Thread"}
            </button>
            <button class="absolute left-[14px] top-[154px] h-[44px] w-[244px] rounded-[12px] border border-[#e0e0db] bg-[#f0f0eb] text-left">
                <span class="absolute left-[16px] top-[9px] inline-flex h-[22px] w-[22px] items-center justify-center rounded-full border border-[#2e2e29] text-[#2e2e29]">
                    <svg viewBox="0 0 24 24" class="h-[13px] w-[13px]" fill="none" stroke="currentColor">
                        <path d="M12 7v5l3 2" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                </span>
                <span class="absolute left-[62px] top-[8px] text-[28px] font-semibold leading-[1] text-[#2e2e29]">{"History"}</span>
            </button>

            <p class="absolute left-[18px] top-[220px] text-[15px] font-medium text-[#334054]">{"Thread 1: Scope Summary"}</p>
            <p class="absolute left-[18px] top-[276px] text-[15px] font-medium text-[#334054]">{"Thread 2: Risk Tracking"}</p>
            <p class="absolute left-[18px] top-[332px] text-[15px] font-medium text-[#334054]">{"Thread 3: Data Quality"}</p>
            <p class="absolute left-[18px] top-[388px] text-[15px] font-medium text-[#334054]">{"Thread 4: API Planning"}</p>
            <p class="absolute left-[18px] top-[444px] text-[15px] font-medium text-[#334054]">{"Thread 5: UX Review"}</p>
            <p class="absolute left-[18px] top-[500px] text-[15px] font-medium text-[#334054]">{"Thread 6: Launch Prep"}</p>

            <div class="absolute left-[298px] top-[86px] h-[94px] w-[776px] rounded-[14px] border border-[#e3ebf2] bg-white"></div>
            <p class="absolute left-[318px] top-[118px] text-[20px] font-normal leading-[1.3] text-[#1f2638]">
                {"Can you summarize the top findings from the uploaded scope document?"}
            </p>

            <div class="absolute left-[298px] top-[208px] h-[420px] w-[776px] rounded-[14px] border border-[#e3ebf2] bg-[#f9f9fb]"></div>
            <p class="absolute left-[318px] top-[236px] text-[16px] font-normal leading-[1.65] text-[#1f2638]">
                {"Primary findings: timeline risk is moderate, API readiness is the main dependency, and user research supports a phased rollout with weekly checkpoints."}
            </p>

            <div class="absolute left-[298px] top-[840px] h-[152px] w-[776px] rounded-[18px] border border-[#e3ebf2] bg-white"></div>
            <p class="absolute left-[322px] top-[872px] text-[16px] font-normal leading-[1.45] text-[#1f2638]">{"Ask a question about your selected sources..."}</p>
            <button class="absolute left-[1022px] top-[930px] h-[36px] w-[36px] rounded-[18px] border border-[#e3ebf2] bg-[#0a1f47]"></button>

            <p class="absolute left-[1124px] top-[100px] text-[30px] font-semibold leading-[1.1] text-[#1f2638]">{"Sources"}</p>
            <div class="absolute left-[1118px] top-[136px] h-[44px] w-[300px] rounded-[22px] border border-[#e3ebf2] bg-white"></div>
            <div class="absolute left-[1118px] top-[208px] h-[54px] w-[300px] rounded-[12px] border border-[#e3ebf2] bg-white"></div>
            <div class="absolute left-[1118px] top-[272px] h-[54px] w-[300px] rounded-[12px] border border-[#e3ebf2] bg-white"></div>
            <div class="absolute left-[1118px] top-[336px] h-[54px] w-[300px] rounded-[12px] border border-[#e3ebf2] bg-white"></div>
            <div class="absolute left-[1118px] top-[400px] h-[54px] w-[300px] rounded-[12px] border border-[#e3ebf2] bg-white"></div>

            <p class="absolute left-[1124px] top-[484px] text-[30px] font-semibold leading-[1.1] text-[#1f2638]">{"Notes"}</p>
            <button class="absolute left-[1118px] top-[524px] h-[46px] w-[300px] rounded-[23px] border border-[#e3ebf2] bg-[#0a1f47] text-[15px] font-semibold text-white">
                {"+ New Note"}
            </button>
            <div class="absolute left-[1118px] top-[590px] h-[122px] w-[300px] rounded-[14px] border border-[#e3ebf2] bg-white"></div>
            <div class="absolute left-[1118px] top-[728px] h-[122px] w-[300px] rounded-[14px] border border-[#e3ebf2] bg-white"></div>

            {children.map(|slot| slot())}
        </CanvasFrame>
    }
}

#[component]
pub fn PreviewEntryPage() -> impl IntoView {
    view! { <Redirect path="/preview/login" /> }
}

#[component]
pub fn PreviewLoginPage() -> impl IntoView {
    view! {
        <CanvasFrame>
            <div class="absolute left-[-2px] top-[-2px] h-[1024px] w-[1440px] border border-[#e3ebf2] bg-[#fdfdfe]"></div>
            <div class="absolute left-[138px] top-[38px] h-[420px] w-[420px] rounded-[210px] bg-[#f9f9fb] blur-[60px]"></div>
            <div class="absolute left-[498px] top-[168px] h-[680px] w-[440px] rounded-[24px] border border-[#e3ebf2] bg-[#f9f9fb] shadow-[0px_14px_34px_0px_rgba(15,20,31,0.08)]"></div>

            <div class="absolute left-[678px] top-[228px]">
                <BrainLogo size="h-[80px] w-[80px]" />
            </div>
            <p class="absolute left-[592px] top-[334px] text-[36px] font-semibold leading-[1.15] text-[#141a24]">{"Welcome Back"}</p>
            <p class="absolute left-[566px] top-[382px] text-[15px] font-normal leading-[1.45] text-[#737d8f]">{"Sign in to continue your second-brain workflow."}</p>

            <p class="absolute left-[578px] top-[448px] text-[15px] font-medium leading-[1.3] text-[#525c6e]">{"Email"}</p>
            <div class="absolute left-[578px] top-[472px] h-[50px] w-[280px] rounded-[12px] border border-[#e3ebf2] bg-[#fbfcfd]"></div>
            <p class="absolute left-[596px] top-[488px] text-[15px] font-normal leading-[1.3] text-[#9ea8ba]">{"name@example.com"}</p>

            <p class="absolute left-[578px] top-[540px] text-[15px] font-medium leading-[1.3] text-[#525c6e]">{"Password"}</p>
            <div class="absolute left-[578px] top-[564px] h-[50px] w-[280px] rounded-[12px] border border-[#e3ebf2] bg-[#fbfcfd]"></div>
            <p class="absolute left-[596px] top-[580px] text-[15px] font-normal leading-[1.3] text-[#9ea8ba]">{"***********"}</p>

            <A href="/preview/dashboard" attr:class="absolute left-[578px] top-[642px] inline-flex h-[52px] w-[280px] items-center justify-center rounded-[14px] border border-[#e3ebf2] bg-[#0a1f47] text-[15px] font-semibold text-white">
                {"Continue"}
            </A>
            <p class="absolute left-[574px] top-[724px] text-[15px] font-normal leading-[1.35] text-[#7a8799]">{"No account yet?"}</p>
            <A href="/preview/dashboard" attr:class="absolute left-[683px] top-[724px] text-[15px] font-semibold leading-[1.35] text-[#141a24]">
                {"Create one"}
            </A>
        </CanvasFrame>
    }
}

#[component]
pub fn PreviewDashboardPage() -> impl IntoView {
    view! {
        <div class="min-h-screen overflow-hidden bg-white text-[#18181b] antialiased" style="font-family:'Inter','SF Pro Display','PingFang SC','Noto Sans SC','Segoe UI',sans-serif;">
            <header class="border-b border-[#f0f1f3] bg-white">
                <div class="mx-auto flex h-[72px] w-full max-w-[1280px] items-center justify-between px-7">
                    <A href="/preview/dashboard" attr:class="inline-flex items-center gap-2.5 text-[#111827]">
                        <svg viewBox="0 0 24 24" class="h-7 w-7" fill="none" stroke="currentColor">
                            <path d="M4 6.5A1.5 1.5 0 015.5 5h13A1.5 1.5 0 0120 6.5v11A1.5 1.5 0 0118.5 19h-13A1.5 1.5 0 014 17.5z" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>
                            <path d="M8 9h8M8 13h5" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>
                        </svg>
                        <span class="text-[30px] font-semibold tracking-[-0.03em]">{"NotebookLM"}</span>
                    </A>

                    <div class="flex items-center gap-4">
                        <A href="/preview/settings" attr:class="inline-flex h-[34px] items-center gap-1.5 rounded-full border border-[#e5e7eb] bg-white px-3 text-[14px] font-medium text-[#71717a] shadow-[0_1px_2px_rgba(24,24,27,0.04)]">
                            <svg class="h-[15px] w-[15px]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15a3 3 0 100-6 3 3 0 000 6z"/>
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-2 2 2 2 0 01-2-2v-.09a1.65 1.65 0 00-1-1.51 1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06a1.65 1.65 0 00.33-1.82 1.65 1.65 0 00-1.51-1H3a2 2 0 01-2-2 2 2 0 012-2h.09a1.65 1.65 0 001.51-1 1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06a1.65 1.65 0 001.82.33h.01A1.65 1.65 0 009 3.09V3a2 2 0 012-2 2 2 0 012 2v.09a1.65 1.65 0 001 1.51h.01a1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06a1.65 1.65 0 00-.33 1.82v.01a1.65 1.65 0 001.51 1H21a2 2 0 012 2 2 2 0 01-2 2h-.09a1.65 1.65 0 00-1.51 1z"/>
                            </svg>
                            <span>{"设置"}</span>
                        </A>
                        <A href="/preview/account" attr:class="inline-flex h-[34px] w-[34px] items-center justify-center rounded-full bg-[#f4f4f5] text-[#71717a]">
                            <svg class="h-[18px] w-[18px]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M12 12a4 4 0 100-8 4 4 0 000 8z"/>
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M4 20a8 8 0 0116 0"/>
                            </svg>
                        </A>
                    </div>
                </div>
            </header>

            <main class="mx-auto w-full max-w-[1280px] px-7 py-8">
                <div class="mb-8 flex items-center justify-between gap-4">
                    <div class="flex items-center gap-2">
                        <button class="rounded-full px-5 py-2 text-[14px] font-medium text-[#71717a] hover:bg-[#fafafa]">{"全部"}</button>
                        <button class="rounded-full bg-[#f4f4f5] px-5 py-2 text-[14px] font-medium text-[#18181b]">{"我的笔记本"}</button>
                    </div>

                    <div class="flex items-center gap-3">
                        <button class="inline-flex h-[38px] w-[38px] items-center justify-center rounded-full border border-[#e5e7eb] text-[#71717a] shadow-[0_1px_2px_rgba(24,24,27,0.05)]">
                            <svg class="h-[18px] w-[18px]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.25" d="M21 21l-4.35-4.35m1.85-5.15a7 7 0 11-14 0 7 7 0 0114 0z"/>
                            </svg>
                        </button>
                        <div class="flex items-center gap-[2px] rounded-full border border-[#e4e4e7] bg-[#f4f4f5]/80 p-1 shadow-[0_1px_2px_rgba(24,24,27,0.04)]">
                            <button class="inline-flex h-[30px] w-[30px] items-center justify-center rounded-full text-[#9ca3af]">
                                <svg class="h-[15px] w-[15px]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M4 5h6v6H4V5zm10 0h6v6h-6V5zM4 13h6v6H4v-6zm10 0h6v6h-6v-6z"/>
                                </svg>
                            </button>
                            <button class="inline-flex h-[30px] w-[30px] items-center justify-center rounded-full bg-white text-[#18181b] shadow-[0_1px_2px_rgba(24,24,27,0.08)]">
                                <svg class="h-[16px] w-[16px]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M5 7h14M5 12h14M5 17h14"/>
                                </svg>
                            </button>
                        </div>
                        <button class="inline-flex h-[38px] items-center gap-2 rounded-full border border-[#e5e7eb] px-5 text-[14px] font-medium text-[#52525b] shadow-[0_1px_2px_rgba(24,24,27,0.05)]">
                            <span>{"最近"}</span>
                            <svg class="h-[14px] w-[14px] text-[#9ca3af]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M19 9l-7 7-7-7"/>
                            </svg>
                        </button>
                        <A href="/preview/workspace" attr:class="inline-flex h-[38px] items-center gap-2 rounded-full bg-black px-6 text-[14px] font-medium text-white shadow-[0_1px_2px_rgba(24,24,27,0.08)]">
                            <svg class="h-[15px] w-[15px]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.4" d="M12 4v16m8-8H4"/>
                            </svg>
                            <span>{"新建"}</span>
                        </A>
                    </div>
                </div>

                <h1 class="mb-6 px-1 text-[38px] font-medium tracking-[-0.03em] text-[#18181b]">
                    {"我的笔记本"}
                </h1>

                <div class="grid grid-cols-12 gap-4 border-b border-[#eceef1] px-4 pb-3 text-[13px] font-medium text-[#a1a1aa]">
                    <div class="col-span-6">{"标题"}</div>
                    <div class="col-span-2">{"来源"}</div>
                    <div class="col-span-2">{"创建日期"}</div>
                    <div class="col-span-2">{"角色"}</div>
                </div>

                <div class="pb-10">
                    <A href="/preview/workspace" attr:class="contents">
                        <PreviewDashboardRow
                            title="中核集团市场开发现状分析与战略调研模板"
                            sources="71 个来源"
                            date="2026年3月30日"
                            active=false
                        />
                    </A>
                    <PreviewDashboardRow
                        title="The Expert Interview Guide: Insight-Driven Research and Best Practices"
                        sources="20 个来源"
                        date="2026年3月20日"
                        active=false
                    />
                    <PreviewDashboardRow
                        title="China 2030 Power Market Outlook: Demand, Structure, and Costs"
                        sources="1 个来源"
                        date="2026年3月17日"
                        active=false
                    />
                    <PreviewDashboardRow
                        title="CNNC Power Trading Operational Framework and Execution Flow"
                        sources="29 个来源"
                        date="2026年3月16日"
                        active=true
                    />
                    <PreviewDashboardRow
                        title="CNNP Electricity Sales and Value-Added Services Management Standards"
                        sources="25 个来源"
                        date="2026年3月13日"
                        active=false
                    />
                    <PreviewDashboardRow
                        title="The Dual Nature of Power: Commodity and System Logistics"
                        sources="3 个来源"
                        date="2026年3月13日"
                        active=false
                    />
                    <PreviewDashboardRow
                        title="China National Nuclear Power Market Development Strategy 2026-2030"
                        sources="2 个来源"
                        date="2026年3月13日"
                        active=false
                    />
                    <PreviewDashboardRow
                        title="Strategic Framework for Prospectus and S-1 Analysis"
                        sources="3 个来源"
                        date="2026年3月10日"
                        active=false
                    />
                    <PreviewDashboardRow
                        title="China's Unified Power Market and Energy Storage Evolution"
                        sources="46 个来源"
                        date="2026年3月9日"
                        active=false
                    />
                    <PreviewDashboardRow
                        title="Beyond Naive RAG: Hybrid Search and Collaborative AI Tutoring"
                        sources="21 个来源"
                        date="2026年3月6日"
                        active=false
                    />
                </div>
            </main>
        </div>
    }
}

#[component]
pub fn PreviewWorkspacePage() -> impl IntoView {
    view! {
        <div class="flex h-[100dvh] min-h-[100dvh] flex-col overflow-hidden bg-[#fafaf9] text-[#1f2937] antialiased" style="font-family:'Inter','SF Pro Text','PingFang SC','Noto Sans SC','Segoe UI',sans-serif;">
            <div class="flex h-[44px] items-center border-b border-[#eceff4] bg-[#fbfbfa] px-3.5">
                <div class="flex items-center gap-4">
                    <div class="flex items-center gap-2">
                        <span class="inline-flex h-6 w-6 items-center justify-center rounded-[7px] bg-[#0f172a] text-[11px] font-semibold text-white">
                            {"⟠"}
                        </span>
                        <span class="text-[14px] font-semibold text-[#101828]">{"Context-OS"}</span>
                    </div>
                    <span class="h-6 w-px bg-[#dbe2ea]"></span>
                    <span class="text-[14px] font-medium text-[#39507a]">{"Research Project Alpha"}</span>
                </div>
                <div class="ml-auto flex items-center gap-5 text-[14px] font-medium text-[#40506a]">
                    <button class="inline-flex items-center gap-1.5 hover:text-[#111827]">
                        <span class="text-[18px] leading-none">{"+"}</span>
                        <span>{"New Notebook"}</span>
                    </button>
                    <button class="inline-flex items-center gap-1.5 hover:text-[#111827]">
                        <span>{"∿"}</span>
                        <span>{"Analyze"}</span>
                    </button>
                    <button class="hover:text-[#111827]">{"Share"}</button>
                    <button class="hover:text-[#111827]">{"API"}</button>
                    <button class="hover:text-[#111827]">{"⚙"}</button>
                    <button class="inline-flex h-6 w-6 items-center justify-center rounded-full border border-[#bcc7d3] text-[12px] text-[#425466]">
                        {"◔"}
                    </button>
                </div>
            </div>

            <div class="flex h-[calc(100dvh-44px)] min-h-0">
                <aside class="flex w-[210px] shrink-0 flex-col border-r border-[#eceff4] bg-[#fbfbfa] px-3 py-3">
                    <button class="inline-flex h-[31px] items-center justify-center gap-2 rounded-full bg-[#152544] text-[13.5px] font-semibold text-white">
                        <span class="text-[16px] leading-none">{"+"}</span>
                        <span>{"New Thread"}</span>
                    </button>

                    <div class="mt-12">
                        <div class="flex h-[30px] items-center rounded-full border border-[#dde3ea] bg-white px-3 text-[12.5px] text-[#94a3b8]">
                            <span class="mr-2">{"⌕"}</span>
                            <span>{"Search threads"}</span>
                        </div>
                    </div>

                    <div class="mt-5 text-[12px] font-semibold uppercase tracking-[0.04em] text-[#718096]">
                        {"Threads"}
                    </div>

                    <div class="mt-2 space-y-1.5">
                        <PreviewWorkspaceThreadRow title="Generative AI trends 2024" active=true />
                        <PreviewWorkspaceThreadRow title="React Performance Opti..." active=false />
                        <PreviewWorkspaceThreadRow title="Vite build configurations" active=false />
                        <PreviewWorkspaceThreadRow title="Kubernetes vs Docker Sw..." active=false />
                        <PreviewWorkspaceThreadRow title="Figma to Code plugin fea..." active=false />
                        <PreviewWorkspaceThreadRow title="Tailwind grid system layo..." active=false />
                    </div>
                </aside>

                <main class="flex min-w-0 flex-1 flex-col bg-white">
                    <div class="mx-auto grid h-full w-full max-w-[744px] grid-rows-[1fr_auto] px-7 pt-5">
                        <div class="min-h-0">
                            <div class="flex justify-center">
                                <div class="w-[456px] rounded-[14px] bg-[#f3f5f8] px-5 py-4 text-[15px] leading-[1.42] text-[#314154] shadow-[inset_0_0_0_1px_rgba(226,232,240,0.55)]">
                                    {"Can you summarize the main findings from the uploaded Project Scope document?"}
                                </div>
                            </div>

                            <div class="mt-2 flex justify-end gap-4 pr-5 text-[12px] text-[#94a3b8]">
                                <span>{"Copy"}</span>
                                <span>{"Edit"}</span>
                            </div>

                            <div class="mt-7 max-w-[612px] text-[15px] leading-[1.55] text-[#314154]">
                                <p>{"Based on the Project Scope document, the main findings are:"}</p>
                                <p class="mt-5">
                                    {"1. *Core Objective*: The primary goal is to launch a unified dashboard by Q4 that integrates marketing and sales data."}
                                </p>
                                <p class="mt-2">
                                    {"2. *Budget & Timeline*: The allocated budget is $250,000, with a strict deadline of November 15th for the beta release."}
                                </p>
                                <p class="mt-2">
                                    {"3. *Key Dependencies*: The project heavily relies on the new API endpoints being delivered by the backend team by early September."}
                                </p>
                                <p class="mt-7">
                                    {"Would you like me to elaborate on the risks outlined in section 4?"}
                                </p>
                            </div>

                            <div class="mt-5 flex items-center gap-2 text-[12px] text-[#8b98aa]">
                                <span>{"Sources:"}</span>
                                <span class="inline-flex h-5 min-w-5 items-center justify-center rounded-full border border-[#e1e6ee] bg-[#f8fafc] px-1.5 text-[11px] font-semibold text-[#607086]">
                                    {"1"}
                                </span>
                                <span class="inline-flex h-5 min-w-5 items-center justify-center rounded-full border border-[#e1e6ee] bg-[#f8fafc] px-1.5 text-[11px] font-semibold text-[#607086]">
                                    {"2"}
                                </span>
                            </div>

                            <div class="mt-3 flex items-center gap-5 text-[12px] text-[#94a3b8]">
                                <span>{"Copy"}</span>
                                <span>{"Add to note"}</span>
                                <span>{"Regenerate"}</span>
                            </div>
                        </div>

                        <div class="pb-6 pt-8">
                            <div class="mx-auto max-w-[612px] rounded-[16px] border border-[#dde3ea] bg-white px-4 py-3 shadow-[0_10px_30px_-18px_rgba(15,23,42,0.25)]">
                                <div class="text-[14px] text-[#a0aec0]">
                                    {"Ask a question about your sources..."}
                                </div>
                                <div class="mt-6 flex items-center justify-between">
                                    <button class="inline-flex h-7 w-7 items-center justify-center rounded-full text-[20px] leading-none text-[#64748b]">
                                        {"+"}
                                    </button>
                                    <button class="inline-flex h-7 w-7 items-center justify-center rounded-full bg-[#152544] text-[13px] text-white">
                                        {"↑"}
                                    </button>
                                </div>
                            </div>
                        </div>
                    </div>
                </main>

                <aside class="flex w-[304px] shrink-0 flex-col border-l border-[#eceff4] bg-[#fbfbfa]">
                    <div class="border-b border-[#eceff4] px-4 py-4">
                        <div class="flex items-center gap-2">
                            <h2 class="text-[16px] font-semibold text-[#1f2d3d]">{"Sources"}</h2>
                            <span class="inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-[#eef2f7] px-1.5 text-[11px] font-semibold text-[#66768d]">
                                {"4"}
                            </span>
                        </div>

                        <button class="mt-5 flex h-[36px] w-full items-center justify-center gap-2 rounded-full border border-[#dde3ea] bg-white text-[14px] font-semibold text-[#22324d]">
                            <span class="text-[18px] leading-none">{"+"}</span>
                            <span>{"New Source"}</span>
                        </button>

                        <div class="mt-4 flex items-center justify-between text-[13px] text-[#617287]">
                            <span>{"Select all"}</span>
                            <span class="inline-flex h-[15px] w-[15px] rounded-[3px] border border-[#9ba7b6] bg-white"></span>
                        </div>

                        <div class="mt-4 space-y-2">
                            <PreviewWorkspaceSourceRow title="Q3_Financial_Report.pdf" checked=true />
                            <PreviewWorkspaceSourceRow title="Project_Scope_v2.docx" checked=true />
                            <PreviewWorkspaceSourceRow title="Competitor Analysis - Wikipedia" checked=false />
                            <PreviewWorkspaceSourceRow title="User_Research_Interviews.pdf" checked=true />
                        </div>
                    </div>

                    <div class="flex-1 px-4 py-4">
                        <h2 class="text-[16px] font-semibold text-[#1f2d3d]">{"Notes"}</h2>

                        <button class="mt-6 flex h-[36px] w-full items-center justify-center gap-2 rounded-full bg-[#152544] text-[14px] font-semibold text-white">
                            <span class="text-[18px] leading-none">{"+"}</span>
                            <span>{"New Note"}</span>
                        </button>

                        <div class="mt-4 text-[12px] font-semibold uppercase tracking-[0.04em] text-[#718096]">
                            {"Saved Notes"}
                        </div>

                        <div class="mt-3 space-y-3">
                            <PreviewWorkspaceNoteCard
                                title="Summary of Q3 Goals"
                                preview="The main goal for Q3 is to increase user retention by 15% through..."
                            />
                            <PreviewWorkspaceNoteCard
                                title="Key Risks Identified"
                                preview="Technical debt in the legacy payment system poses a significant risk to..."
                            />
                        </div>
                    </div>
                </aside>
            </div>
        </div>
    }
}

#[component]
pub fn PreviewAccountPage() -> impl IntoView {
    view! {
        <WorkspaceChrome>
            <p class="absolute left-[318px] top-[104px] text-[34px] font-semibold leading-[1.15] text-[#1f2638]">{"Account & Quota"}</p>

            <div class="absolute left-[318px] top-[168px] h-[220px] w-[480px] rounded-[16px] border border-[#e0e8f2] bg-white"></div>
            <div class="absolute left-[818px] top-[168px] h-[220px] w-[620px] rounded-[16px] border border-[#e0e8f2] bg-white"></div>

            <p class="absolute left-[342px] top-[198px] text-[16px] font-medium leading-[1.35] text-[#303b4f]">{"Name: Chuan"}</p>
            <p class="absolute left-[342px] top-[232px] text-[16px] font-medium leading-[1.35] text-[#303b4f]">{"Email: chuan@context-os.ai"}</p>
            <p class="absolute left-[342px] top-[266px] text-[16px] font-medium leading-[1.35] text-[#303b4f]">{"Organization: Context-OS"}</p>
            <p class="absolute left-[342px] top-[300px] text-[16px] font-medium leading-[1.35] text-[#303b4f]">{"Role: Owner"}</p>

            <p class="absolute left-[846px] top-[196px] text-[22px] font-semibold leading-[1.2] text-[#1f2638]">{"Usage Overview"}</p>
            <p class="absolute left-[846px] top-[232px] text-[15px] font-medium leading-[1.35] text-[#667385]">{"Monthly request cap"}</p>
            <p class="absolute left-[1088px] top-[232px] text-[15px] font-semibold leading-[1.35] text-[#242b3d]">{"12,000"}</p>
            <p class="absolute left-[846px] top-[266px] text-[15px] font-medium leading-[1.35] text-[#667385]">{"Used this month"}</p>
            <p class="absolute left-[1088px] top-[266px] text-[15px] font-semibold leading-[1.35] text-[#242b3d]">{"4,820"}</p>
            <p class="absolute left-[846px] top-[300px] text-[15px] font-medium leading-[1.35] text-[#667385]">{"Remaining"}</p>
            <p class="absolute left-[1088px] top-[300px] text-[15px] font-semibold leading-[1.35] text-[#242b3d]">{"7,180"}</p>
            <p class="absolute left-[846px] top-[334px] text-[15px] font-medium leading-[1.35] text-[#667385]">{"Reset window"}</p>
            <p class="absolute left-[1088px] top-[334px] text-[15px] font-semibold leading-[1.35] text-[#242b3d]">{"Every month, 01:00 UTC+8"}</p>
        </WorkspaceChrome>
    }
}

#[component]
pub fn PreviewSettingsPage() -> impl IntoView {
    view! {
        <WorkspaceChrome>
            <p class="absolute left-[318px] top-[104px] text-[34px] font-semibold leading-[1.15] text-[#1f2638]">{"Settings"}</p>

            <div class="absolute left-[318px] top-[168px] h-[160px] w-[1120px] rounded-[16px] border border-[#e0e8f2] bg-white"></div>
            <p class="absolute left-[346px] top-[192px] text-[22px] font-semibold leading-[1.2] text-[#242e40]">{"Help & Docs"}</p>
            <p class="absolute left-[346px] top-[228px] text-[15px] font-medium leading-[1.45] text-[#707a8c]">{"Open the product manual, FAQ, and API guidance."}</p>
            <A href="/preview/help" attr:class="absolute left-[1226px] top-[228px] inline-flex h-[42px] w-[156px] items-center justify-center rounded-[21px] border border-[#d1dbe8] bg-[#0a1f47] text-[15px] font-medium text-white">
                {"Open Help"}
            </A>

            <div class="absolute left-[318px] top-[344px] h-[160px] w-[1120px] rounded-[16px] border border-[#e0e8f2] bg-white"></div>
            <p class="absolute left-[346px] top-[368px] text-[22px] font-semibold leading-[1.2] text-[#242e40]">{"Visual Theme"}</p>
            <p class="absolute left-[346px] top-[404px] text-[15px] font-medium leading-[1.45] text-[#707a8c]">{"Switch between light and dark modes."}</p>
            <button class="absolute left-[346px] top-[438px] h-[42px] w-[156px] rounded-[21px] border border-[#d1dbe8] bg-[#0a1f47] text-[15px] font-medium text-white">
                {"Light (Current)"}
            </button>
            <button class="absolute left-[514px] top-[438px] h-[42px] w-[156px] rounded-[21px] border border-[#d1dbe8] bg-white text-[15px] font-medium text-[#333d4f]">
                {"Dark"}
            </button>

            <div class="absolute left-[318px] top-[520px] h-[160px] w-[1120px] rounded-[16px] border border-[#e0e8f2] bg-white"></div>
            <p class="absolute left-[346px] top-[544px] text-[22px] font-semibold leading-[1.2] text-[#242e40]">{"Language"}</p>
            <p class="absolute left-[346px] top-[580px] text-[15px] font-medium leading-[1.45] text-[#707a8c]">{"Choose one UI language for the entire product."}</p>
            <button class="absolute left-[346px] top-[614px] h-[42px] w-[156px] rounded-[21px] border border-[#d1dbe8] bg-[#0a1f47] text-[15px] font-medium text-white">
                {"English (Current)"}
            </button>
            <button class="absolute left-[514px] top-[614px] h-[42px] w-[156px] rounded-[21px] border border-[#d1dbe8] bg-white text-[15px] font-medium text-[#333d4f]">
                {"Chinese"}
            </button>
        </WorkspaceChrome>
    }
}

#[component]
pub fn PreviewHelpPage() -> impl IntoView {
    view! {
        <CanvasFrame>
            <div class="absolute left-[-2px] top-[-2px] h-[72px] w-[1440px] border border-[#e3ebf2] bg-white"></div>
            <div class="absolute left-[-2px] top-[70px] h-[952px] w-[1440px] border border-[#e5ebf2] bg-[#f6f8fb]"></div>

            <A href="/preview/dashboard" attr:class="absolute left-[46px] top-[22px] block size-[32px]">
                <BrainLogo size="h-[32px] w-[32px]" />
            </A>
            <A href="/preview/dashboard" attr:class="absolute left-[94px] top-[21px] text-[27px] font-semibold tracking-[-0.01em] text-[#12141c]">
                {"Context-OS"}
            </A>
            <p class="absolute left-[292px] top-[24px] text-[19px] font-medium leading-[1.25] text-[#1f2638]">{"Research Project Alpha"}</p>
            <A href="/preview/workspace" attr:class="absolute left-[1238px] top-[16px] inline-flex h-[36px] w-[160px] items-center justify-center rounded-[18px] bg-[#0f2e66] text-[15px] font-semibold text-white">
                {"Back to Workspace"}
            </A>
            <AvatarButton href="/preview/account" />

            <div class="absolute left-[38px] top-[102px] h-[888px] w-[320px] rounded-[14px] border border-[#d9e0ed] bg-white"></div>
            <div class="absolute left-[378px] top-[102px] h-[888px] w-[1020px] rounded-[14px] border border-[#d9e0ed] bg-white"></div>

            <p class="absolute left-[62px] top-[134px] text-[22px] font-semibold leading-[1.2] text-[#21293b]">{"Help Index"}</p>
            <p class="absolute left-[62px] top-[182px] text-[15px] font-medium leading-[1.35] text-[#424f63]">{"Quick Start"}</p>
            <p class="absolute left-[62px] top-[222px] text-[15px] font-medium leading-[1.35] text-[#424f63]">{"Auth & Security"}</p>
            <p class="absolute left-[62px] top-[262px] text-[15px] font-medium leading-[1.35] text-[#424f63]">{"Notebooks & Sources"}</p>
            <p class="absolute left-[62px] top-[302px] text-[15px] font-medium leading-[1.35] text-[#424f63]">{"Threads & History"}</p>
            <p class="absolute left-[62px] top-[342px] text-[15px] font-medium leading-[1.35] text-[#424f63]">{"Sharing & Invites"}</p>
            <p class="absolute left-[62px] top-[382px] text-[15px] font-medium leading-[1.35] text-[#424f63]">{"API Access"}</p>
            <p class="absolute left-[62px] top-[422px] text-[15px] font-medium leading-[1.35] text-[#424f63]">{"Notifications"}</p>
            <p class="absolute left-[62px] top-[462px] text-[15px] font-medium leading-[1.35] text-[#424f63]">{"Quota & Limits"}</p>

            <p class="absolute left-[410px] top-[134px] text-[34px] font-semibold leading-[1.18] text-[#1f2638]">{"Context-OS Help Center (Wiki)"}</p>
            <div class="absolute left-[410px] top-[196px] w-[860px] whitespace-pre-wrap text-[15px] font-medium leading-[1.7] text-[#384257]">
                {"1) Account & Auth: sign up, sign in, sign out, password reset, and verification flows.\n\n2) Notebook Management: create, rename, delete notebooks, upload documents, and import URLs.\n\n3) Conversation System: start sessions, rename/remove threads, and inspect cited source snippets.\n\n4) Thread History Search: open the History panel in Workspace and jump directly to matched sessions.\n\n5) Sharing: generate share links, manage access policy, and review analytics or audit logs.\n\n6) Collaboration: invite teammates, accept or decline invites, and remove members safely.\n\n7) API Access: create, list, and revoke notebook-level API keys for controlled integrations.\n\n8) Preferences: manage notifications, language, and display theme at the profile level.\n\n9) Usage Limits: read current monthly cap, consumed units, and remaining capacity in Account.\n\n10) Ops Endpoints: health, readiness, metrics, openapi, and docs are available for operations."}
            </div>
        </CanvasFrame>
    }
}
