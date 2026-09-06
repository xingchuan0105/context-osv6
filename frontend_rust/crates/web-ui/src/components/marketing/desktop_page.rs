use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use serde::Deserialize;

/// 桌面发布清单（对齐 Next `lib/desktop/release-manifest.ts`）。
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DesktopPlatformManifest {
    pub url: String,
    pub sha256: String,
    pub size_bytes: i64,
    pub format: String,
    #[serde(default)]
    pub authenticode: Option<bool>,
    #[serde(default)]
    pub filename: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DesktopReleaseManifest {
    pub version: String,
    #[serde(default)]
    pub min_os: Option<String>,
    #[serde(default)]
    pub platforms: std::collections::HashMap<String, DesktopPlatformManifest>,
}

pub fn windows_download_from_manifest(
    manifest: &DesktopReleaseManifest,
) -> Option<&DesktopPlatformManifest> {
    manifest.platforms.get("windows-x64")
}

pub fn format_bytes(bytes: i64) -> String {
    if bytes <= 0 {
        return "—".to_string();
    }
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb < 1.0 {
        format!("{} KB", (bytes as f64 / 1024.0).max(1.0).round() as i64)
    } else if mb >= 10.0 {
        format!("{mb:.0} MB")
    } else {
        format!("{mb:.1} MB")
    }
}

pub const DESKTOP_LATEST_JSON_URL: &str = "/releases/desktop/latest.json";

pub struct DesktopCopy {
    product_title: &'static str,
    product_subtitle: &'static str,
    benefits_title: &'static str,
    features: &'static [&'static str],
    cta_title: &'static str,
    install_title: &'static str,
    install_steps: &'static [&'static str],
    smart_screen_hint: &'static str,
    buy_cta: &'static str,
    learn_more: &'static str,
    download_windows: &'static str,
    download_loading: &'static str,
    download_unavailable: &'static str,
    download_unavailable_hint: &'static str,
    version_label: &'static str,
    sha256_label: &'static str,
    portable_hint: &'static str,
    unsigned_hint: &'static str,
    signed_hint: &'static str,
    open_saas: &'static str,
    seo_publisher: &'static str,
}

const DESKTOP_ZH: DesktopCopy = DesktopCopy {
    product_title: "AI 知识库桌面客户端 · 完全免费",
    product_subtitle: "数据本地私有，开箱即用官方模型或配置自备 Key，可被 Claude / Codex 等桌面 Agent 以 MCP / CLI 调用。完全免费，无需买断许可。",
    benefits_title: "为什么下载客户端？",
    features: &[
        "数据本地私有：文档与索引默认保存在本机，核心检索无需数据上云",
        "开箱即用 + BYOK：内置官方托管模型免配置体验，同时支持自备主流大模型 Key",
        "桌面 Agent 友好：提供标准 MCP / CLI 接口，无缝接入 Claude Code、Codex 等本地编程助手",
        "完全免费使用客户端；需要上云分享时再升级云端名额",
        "自带 LLM Key（BYOK），本地栈一键启动",
        "可选与云端工作区互通。对外分享需先发布到云端（向量导入、不重灌库），仍走云端档位规则",
    ],
    cta_title: "下载与相关入口",
    install_title: "安装步骤",
    install_steps: &[
        "下载 Windows 客户端安装包",
        "运行安装程序（需 Windows 10+ 与 WebView2）",
        "启动后登录账号即可直接问答（开箱自带官方模型），亦可按需切换自备 Key 或连接桌面 Agent (MCP / CLI)",
    ],
    smart_screen_hint: "若 SmartScreen 提示未知应用，选择「仍要运行」（正式签名信誉积累前属正常现象）。",
    buy_cta: "云端定价（分享名额）",
    learn_more: "Agent 接入说明",
    download_windows: "下载 Windows 客户端",
    download_loading: "正在获取下载信息…",
    download_unavailable: "安装包暂未发布",
    download_unavailable_hint: "请稍后再试，或联系支持获取安装包。",
    version_label: "版本",
    sha256_label: "SHA256",
    portable_hint: "当前为便携版 EXE：下载后直接运行即可（建议固定安装目录）。",
    unsigned_hint: "安装包尚未使用商业代码签名证书；SmartScreen 可能提示未知发布者。",
    signed_hint: "安装包已 Authenticode 签名。若仍见 SmartScreen，选择「仍要运行」（自签/新发布者需积累信誉）。",
    open_saas: "打开云端应用",
    seo_publisher: "说明基于本站公开能力与定价文档（来源：帮助 / 定价页）。",
};

const DESKTOP_EN: DesktopCopy = DesktopCopy {
    product_title: "AI knowledge base desktop client · free",
    product_subtitle: "Private by default, official models out of the box or BYOK, callable by Claude / Codex via MCP / CLI. 100% free with no license fee.",
    benefits_title: "Why download the client?",
    features: &[
        "Private by default: docs and indexes stay on this machine",
        "Out of the box + BYOK: official models ready to use, or bring your own LLM keys",
        "Desktop-agent ready: standard MCP & CLI for Claude Code, Codex, and local agents",
        "Client is free; upgrade cloud share slots only when you publish online",
        "Bring your own LLM key; one-click local stack",
        "Optional cloud workspace sync. Public sharing requires publish-to-cloud (vector import, no re-ingest) and still follows cloud plan limits",
    ],
    cta_title: "Download & related",
    install_title: "Install steps",
    install_steps: &[
        "Download the Windows client installer",
        "Run the installer (Windows 10+ with WebView2)",
        "Sign in to chat immediately with official models, or switch to your own key and connect agents (MCP / CLI)",
    ],
    smart_screen_hint: "If SmartScreen warns about an unknown app, choose Run anyway (normal until publisher reputation builds).",
    buy_cta: "Cloud pricing (share slots)",
    learn_more: "Agent access guide",
    download_windows: "Download for Windows",
    download_loading: "Checking download…",
    download_unavailable: "Installer not published yet",
    download_unavailable_hint: "Please try again later or contact support.",
    version_label: "Version",
    sha256_label: "SHA256",
    portable_hint: "Portable EXE: run after download (keep a stable folder).",
    unsigned_hint: "Installer is not yet signed with a commercial code-signing certificate; SmartScreen may warn.",
    signed_hint: "Installer is Authenticode-signed. SmartScreen may still warn for new/self-signed publishers — use Run anyway if needed.",
    open_saas: "Open cloud app",
    seo_publisher: "说明基于本站公开能力与定价文档（来源：帮助 / 定价页）。",
};

pub fn desktop_copy(locale: &str) -> &'static DesktopCopy {
    if locale == "en" {
        &DESKTOP_EN
    } else {
        &DESKTOP_ZH
    }
}

fn desktop_seo(locale: &str) -> (&'static str, &'static str, &'static str) {
    if locale == "en" {
        (
            "AI knowledge base desktop client · free download",
            "Context OS Windows desktop client: free to download, data stays local, MCP / CLI for desktop agents.",
            "/en/desktop",
        )
    } else {
        (
            "AI 知识库桌面客户端 · 免费下载",
            "Context OS Windows 桌面客户端：免费下载使用，数据留在本机，支持 MCP / CLI 供桌面 Agent 调用。",
            "/desktop",
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
enum DownloadState {
    Loading,
    Ready(DesktopReleaseManifest, DesktopPlatformManifest),
    Unavailable,
}

/// `/desktop`（zh）与 `/en/desktop`（en）公开桌面下载页。
/// 下载信息来自 `/releases/desktop/latest.json`；未发布时呈现 Unavailable 态。
#[component]
pub fn DesktopProductPage(locale: &'static str) -> impl IntoView {
    let t = desktop_copy(locale);
    let (seo_title, seo_description, canonical) = desktop_seo(locale);
    let state = RwSignal::new(DownloadState::Loading);

    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            let client = web_sdk::BrowserRestClient::new("", None);
            match client.get_bytes(DESKTOP_LATEST_JSON_URL).await {
                Ok(bytes) => match serde_json::from_slice::<DesktopReleaseManifest>(&bytes) {
                    Ok(manifest) => {
                        let win = windows_download_from_manifest(&manifest).cloned();
                        match win {
                            Some(win) => state.set(DownloadState::Ready(manifest, win)),
                            None => state.set(DownloadState::Unavailable),
                        }
                    }
                    Err(_) => state.set(DownloadState::Unavailable),
                },
                Err(_) => state.set(DownloadState::Unavailable),
            }
        });
    });

    let en_link = (locale == "en").then(|| {
        view! { <Link rel="alternate" hreflang="en" href="/en/desktop"/> }.into_any()
    });
    view! {
        <Title text=seo_title/>
        <Meta name="description" content=seo_description/>
        <Link rel="canonical" href=canonical/>
        <Link rel="alternate" hreflang="zh-CN" href="/desktop"/>
        {en_link}
        <Link rel="alternate" hreflang="x-default" href="/desktop"/>
        <main class="pub-shell" data-testid="desktop-product-page">
            <div class="pub-center pub-desktop-grid">
                <header class="pub-header">
                    <h1 class="pub-title">{t.product_title}</h1>
                    <p class="pub-subtitle">{t.product_subtitle}</p>
                    <p class="pub-muted">{t.seo_publisher}</p>
                </header>

                <div class="pub-desktop-columns">
                    <section class="pub-card">
                        <h2 class="pub-h2">{t.benefits_title}</h2>
                        <ul class="pub-list">
                            {t.features.iter().map(|f| view! { <li>{*f}</li> }).collect_view()}
                        </ul>
                    </section>

                    <div class="pub-desktop-aside">
                        <section class="pub-card">
                            <h2 class="pub-h2">{t.cta_title}</h2>
                            <div class="pub-btn-row" data-testid="desktop-cta-row">
                                {move || match state.get() {
                                    DownloadState::Loading => view! {
                                        <button type="button" class="pub-btn" disabled>{t.download_loading}</button>
                                    }.into_any(),
                                    DownloadState::Unavailable => view! {
                                        <button type="button" class="pub-btn" disabled data-testid="desktop-download-unavailable">{t.download_unavailable}</button>
                                    }.into_any(),
                                    DownloadState::Ready(manifest, win) => {
                                        let meta = format!("v{} · {}", manifest.version, format_bytes(win.size_bytes));
                                        view! {
                                            <a
                                                class="pub-btn"
                                                data-testid="desktop-download-windows"
                                                download=win.filename.clone().unwrap_or_else(|| "context-os-setup".to_string())
                                                href=win.url.clone()
                                            >
                                                {t.download_windows}
                                            </a>
                                            <span class="pub-muted">{meta}</span>
                                            {manifest.min_os.as_ref().map(|os| view! { <span class="pub-muted">{os.clone()}</span> })}
                                        }.into_any()
                                    }
                                }}
                            </div>
                            {move || match state.get() {
                                DownloadState::Ready(manifest, win) => {
                                    Some(view! {
                                        <dl class="pub-stat-list" data-testid="desktop-release-details">
                                            <div class="admin-stat-row">
                                                <span class="admin-stat-label">{t.version_label}</span>
                                                <span class="admin-stat-value">{format!("v{}", manifest.version)}</span>
                                            </div>
                                            <div class="admin-stat-row">
                                                <span class="admin-stat-label">{t.sha256_label}</span>
                                                <span class="admin-stat-value admin-table-mono">{win.sha256}</span>
                                            </div>
                                            {(win.format == "portable").then(|| {
                                                view! { <p class="pub-muted">{t.portable_hint}</p> }
                                            })}
                                            {(win.authenticode == Some(false)).then(|| {
                                                view! { <p class="pub-muted" data-testid="desktop-unsigned-hint">{t.unsigned_hint}</p> }
                                            })}
                                            {(win.authenticode == Some(true)).then(|| {
                                                view! { <p class="pub-muted" data-testid="desktop-signed-hint">{t.signed_hint}</p> }
                                            })}
                                        </dl>
                                    })
                                        .into_any()
                                }
                                DownloadState::Unavailable => Some(view! {
                                    <p class="pub-muted" data-testid="desktop-download-hint">{t.download_unavailable_hint}</p>
                                })
                                    .into_any(),
                                DownloadState::Loading => None::<AnyView>.into_any(),
                            }}
                        </section>

                        <section class="pub-card">
                            <h2 class="pub-h2">{t.install_title}</h2>
                            <ol class="pub-list">
                                {t.install_steps.iter().map(|s| view! { <li>{*s}</li> }).collect_view()}
                            </ol>
                            <p class="pub-muted">{t.smart_screen_hint}</p>
                        </section>
                    </div>
                </div>

                <div class="pub-btn-row">
                    <a href="/pricing" class="pub-btn">{t.buy_cta}</a>
                    <a href="/help/api-access" class="pub-btn">{t.learn_more}</a>
                    <a href="/dashboard" class="pub-btn">{t.open_saas}</a>
                </div>
                <crate::components::legal::legal_pages::LegalFooterLinks/>
            </div>
        </main>
    }
}

#[component]
pub fn DesktopPage() -> impl IntoView {
    view! { <DesktopProductPage locale="zh"/> }
}

/// `/activate`：客户端免费、无需激活（ADR-0010）；旧深链重定向到下载页。
#[component]
pub fn DesktopActivatePage() -> impl IntoView {
    let navigate = use_navigate();
    Effect::new(move |_| {
        navigate("/desktop", NavigateOptions::default());
    });
    view! {
        <Title text="客户端激活 - Context OS"/>
        <Meta name="robots" content="noindex, nofollow"/>
        <main class="pub-shell" data-testid="desktop-activate-page">
            <div class="pub-center">
                <section class="pub-card">
                    <p class="pub-muted">"客户端免费，无需激活。正在前往下载页…"</p>
                    <a href="/desktop" class="pub-btn">"前往下载页"</a>
                </section>
            </div>
        </main>
    }
}

/// `/setup` 已退役：BYOK 配置只保留在 `/settings?tab=providers`（PRODUCT_IA §2）。
#[component]
pub fn DesktopSetupPage() -> impl IntoView {
    let navigate = use_navigate();
    Effect::new(move |_| {
        navigate("/settings?tab=providers", NavigateOptions::default());
    });
    view! {
        <Title text="客户端设置 - Context OS"/>
        <Meta name="robots" content="noindex, nofollow"/>
        <main class="pub-shell" data-testid="desktop-setup-page">
            <div class="pub-center">
                <section class="pub-card">
                    <p class="pub-muted">"该页已并入设置页的模型 Provider 标签，正在跳转…"</p>
                    <a href="/settings?tab=providers" class="pub-btn">"前往模型 Provider 设置"</a>
                </section>
            </div>
        </main>
    }
}
