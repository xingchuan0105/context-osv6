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
fn NotebookCard(left: i32, top: i32, title: &'static str) -> impl IntoView {
    view! {
        <div class="absolute h-[186px] w-[420px] rounded-[20px] border border-[#e3ebf2] bg-[#f9f9fb] shadow-[0px_8px_20px_0px_rgba(13,20,31,0.05)]" style={format!("left:{left}px;top:{top}px;")}>
            <div class="absolute left-[22px] top-[22px] h-[46px] w-[46px] rounded-[23px] border border-[#e3ebf2] bg-[#f2f5fa]"></div>
            <p class="absolute left-[22px] top-[82px] text-[17px] font-semibold leading-[1.3] text-[#1f2638]">{title}</p>
            <p class="absolute left-[22px] top-[152px] text-[14px] font-medium leading-[1.3] text-[#738094]">{"Updated Apr 13, 2026"}</p>
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
        <CanvasFrame>
            <div class="absolute left-[-2px] top-[-2px] h-[1024px] w-[1440px] border border-[#e3ebf2] bg-[#fdfdfe]"></div>
            <div class="absolute left-[-2px] top-[-2px] h-[76px] w-[1440px] border border-[#e3ebf2] bg-white"></div>

            <A href="/preview/dashboard" attr:class="absolute left-[22px] top-[24px] block size-[32px]">
                <BrainLogo size="h-[32px] w-[32px]" />
            </A>
            <A href="/preview/dashboard" attr:class="absolute left-[70px] top-[23px] text-[27px] font-semibold tracking-[-0.01em] text-[#12141c]">
                {"Context-OS"}
            </A>

            <button class="absolute left-[938px] top-[17px] h-[38px] w-[96px] rounded-[19px] border border-[#e3ebf2] bg-white text-[15px] font-medium text-[#4d596b]">
                {"Sort"}
            </button>
            <A href="/preview/workspace" attr:class="absolute left-[1050px] top-[15px] inline-flex h-[42px] w-[114px] items-center justify-center rounded-[21px] border border-[#e3ebf2] bg-[#0a1f47] text-[15px] font-semibold text-white">
                {"New"}
            </A>
            <AvatarButton href="/preview/account" />

            <p class="absolute left-[70px] top-[118px] text-[34px] font-semibold leading-[1.15] text-[#141a24]">{"My Notebooks"}</p>

            <A href="/preview/workspace" attr:class="contents">
                <NotebookCard left=70 top=188 title="Notebook 1" />
            </A>
            <NotebookCard left=514 top=188 title="Notebook 2" />
            <NotebookCard left=958 top=188 title="Notebook 3" />
            <NotebookCard left=70 top=402 title="Notebook 4" />
            <NotebookCard left=514 top=402 title="Notebook 5" />
            <NotebookCard left=958 top=402 title="Notebook 6" />
            <NotebookCard left=70 top=616 title="Notebook 7" />
            <NotebookCard left=514 top=616 title="Notebook 8" />
            <NotebookCard left=958 top=616 title="Notebook 9" />
        </CanvasFrame>
    }
}

#[component]
pub fn PreviewWorkspacePage() -> impl IntoView {
    view! { <WorkspaceChrome /> }
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
