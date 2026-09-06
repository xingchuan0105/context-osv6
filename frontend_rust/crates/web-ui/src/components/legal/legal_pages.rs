use super::legal_markdown::{render_legal_markdown, TocEntry};
use crate::components::help::json_ld::{OrganizationJsonLd, SoftwareApplicationJsonLd};
use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};

struct LegalShellCopy {
    updated_prefix: &'static str,
    version_prefix: &'static str,
    toc_title: &'static str,
    toc_aria: &'static str,
    back_label: &'static str,
    back_href: &'static str,
}

fn shell_copy(locale: &str) -> LegalShellCopy {
    if locale == "en" {
        LegalShellCopy {
            updated_prefix: "Last updated: ",
            version_prefix: "Version: ",
            toc_title: "Contents",
            toc_aria: "Document contents",
            back_label: "Back to legal center",
            back_href: "/en/legal",
        }
    } else {
        LegalShellCopy {
            updated_prefix: "最后更新：",
            version_prefix: "版本：",
            toc_title: "目录",
            toc_aria: "文档目录",
            back_label: "返回法律中心",
            back_href: "/legal",
        }
    }
}

/// 法务文档通用壳（标题 / 最后更新 / 版本 / 目录 / 返回法律中心），对齐 Next LegalLayout。
#[component]
pub fn LegalShell(
    locale: &'static str,
    title: &'static str,
    updated: Option<&'static str>,
    version: Option<&'static str>,
    toc: Option<Vec<TocEntry>>,
    children: Children,
) -> impl IntoView {
    let c = shell_copy(locale);
    view! {
        <OrganizationJsonLd locale=locale/>
        <SoftwareApplicationJsonLd locale=locale/>
        <div class="legal-layout" data-testid="legal-doc-layout">
            <header class="legal-header">
                <h1 class="legal-title">{title}</h1>
                {updated.map(|u| view! { <p class="legal-updated">{format!("{}{}", c.updated_prefix, u)}</p> })}
                {version.map(|v| view! { <p class="legal-version">{format!("{}{}", c.version_prefix, v)}</p> })}
            </header>
            <div class="legal-body">
                {toc.filter(|entries| !entries.is_empty()).map(|entries| {
                    view! {
                        <nav class="legal-toc" aria-label=c.toc_aria>
                            <p class="legal-toc-title">{c.toc_title}</p>
                            <ul class="legal-toc-list">
                                {entries.iter().map(|entry| {
                                    let depth_class = format!("legal-toc-item legal-toc-depth-{}", entry.depth);
                                    let href = format!("#{}", entry.id);
                                    view! {
                                        <li class=depth_class>
                                            <a href=href>{entry.text.clone()}</a>
                                        </li>
                                    }
                                }).collect_view()}
                            </ul>
                        </nav>
                    }
                })}
                <div class="legal-content">{children()}</div>
            </div>
            <footer class="legal-footer">
                <a href=c.back_href>{c.back_label}</a>
            </footer>
        </div>
    }
}

/// 公开页脚：法务三链 + 版权（home/legal 公共）。en 指向 /en/legal/*。
#[component]
pub fn LegalFooterLinks(locale: &'static str) -> impl IntoView {
    let (terms_href, privacy_href, licenses_href, terms_label, privacy_label, licenses_label) =
        if locale == "en" {
            (
                "/en/legal/terms",
                "/en/legal/privacy",
                "/en/legal/licenses",
                "Terms",
                "Privacy",
                "Open source",
            )
        } else {
            (
                "/legal/terms",
                "/legal/privacy",
                "/legal/licenses",
                "用户协议",
                "隐私政策",
                "开源声明",
            )
        };
    view! {
        <footer class="legal-footer-links" data-testid="legal-footer-links">
            <div class="legal-footer-content">
                <a href=terms_href>{terms_label}</a>
                <span class="legal-footer-separator">"·"</span>
                <a href=privacy_href>{privacy_label}</a>
                <span class="legal-footer-separator">"·"</span>
                <a href=licenses_href>{licenses_label}</a>
            </div>
            <div class="legal-footer-copyright">"© 2026 Context-OS"</div>
        </footer>
    }
}

/// 剥离 YAML frontmatter（`---` 围栏）与文档级 `#` 标题（LegalShell 已渲染 H1）。
pub fn strip_frontmatter(md: &str) -> String {
    let body = match md.strip_prefix("---\n") {
        Some(rest) => match rest.find("\n---") {
            Some(idx) => rest[idx + 4..].trim_start_matches('\n'),
            None => md,
        },
        None => md,
    };
    body.lines()
        .skip_while(|line| line.trim_start().starts_with('#') || line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_doc(md: &'static str) -> (String, Vec<TocEntry>) {
    render_legal_markdown(&strip_frontmatter(md))
}

const TERMS_ZH_MD: &str = include_str!("../../../../../assets/legal/zh-CN/terms.md");
const PRIVACY_ZH_MD: &str = include_str!("../../../../../assets/legal/zh-CN/privacy.md");
const TERMS_EN_MD: &str = include_str!("../../../../../assets/legal/en/terms.md");
const PRIVACY_EN_MD: &str = include_str!("../../../../../assets/legal/en/privacy.md");
const THIRD_PARTY_MD: &str = include_str!("../../../../../assets/legal/third-party-notices.md");
const PROJECT_LICENSE: &str = include_str!("../../../../../assets/legal/LICENSE");

/// 统计 `### ` 组件条目数（对齐 Next ThirdPartyNotices）。
fn count_third_party_entries(md: &str) -> usize {
    md.lines().filter(|line| line.starts_with("### ")).count()
}

/// 从声明文件提取生成日期（`Generated: YYYY-MM-DD`）。
fn third_party_generated_date(md: &str) -> String {
    md.lines()
        .find_map(|line| {
            let idx = line.to_ascii_lowercase().find("generated:")?;
            let rest = line[idx + "generated:".len()..].trim();
            let date: String = rest.chars().take(10).collect();
            if date.len() == 10 && date.as_bytes()[4] == b'-' {
                Some(date)
            } else {
                None
            }
        })
        .unwrap_or_else(|| "2026-08-05".to_string())
}

const LEGAL_TERMS_VERSION: &str = "2026-06-13";
const LEGAL_PRIVACY_VERSION: &str = "2026-06-13";

fn legal_seo(locale: &str) -> (&'static str, &'static str, &'static str) {
    if locale == "en" {
        (
            "Legal center",
            "Context-OS legal center: terms of service, privacy policy, and open-source notices.",
            "/en/legal",
        )
    } else {
        (
            "法律中心 - Context OS",
            "Context OS 法律文档中心：用户服务协议、隐私政策与开源声明。",
            "/legal",
        )
    }
}

#[component]
pub fn LegalCenterPageView(locale: &'static str) -> impl IntoView {
    let (seo_title, seo_description, canonical) = legal_seo(locale);
    let en = locale == "en";
    let (title, lead, updated_label, contact) = if en {
        ("Legal center", "Please read the following documents before using Context-OS", "Last updated", "For legal questions, contact")
    } else {
        ("法律中心", "使用Context-OS前请阅读以下文档", "最后更新", "如有法律问题，请联系")
    };
    let (terms_title, terms_desc, privacy_title, privacy_desc, licenses_title, licenses_desc) = if en {
        (
            "Terms of service",
            "Read these terms before using Context-OS",
            "Privacy policy",
            "How we collect, use, and protect your personal information",
            "Open-source notices",
            "Open-source components we use and their licenses",
        )
    } else {
        (
            "用户服务协议",
            "使用Context-OS服务前请阅读本协议",
            "隐私政策",
            "了解我们如何收集、使用和保护您的个人信息",
            "开源声明",
            "查看我们使用的开源组件及其许可证",
        )
    };
    let prefix = if en { "/en/legal" } else { "/legal" };
    let terms_href = format!("{prefix}/terms");
    let privacy_href = format!("{prefix}/privacy");
    let licenses_href = format!("{prefix}/licenses");

    view! {
        <OrganizationJsonLd locale=locale/>
        <SoftwareApplicationJsonLd locale=locale/>
        <Title text=seo_title/>
        <Meta name="description" content=seo_description/>
        <Link rel="canonical" href=canonical/>
        <Link rel="alternate" hreflang="zh-CN" href="/legal"/>
        <Link rel="alternate" hreflang="en" href="/en/legal"/>
        <Link rel="alternate" hreflang="x-default" href="/legal"/>
        <main class="pub-shell" data-testid=if en { "legal-center-page-en" } else { "legal-center-page" }>
            <div class="pub-center">
                <div class="legal-center-header">
                    <h1 class="pub-title">{title}</h1>
                    <p class="pub-subtitle">{lead}</p>
                </div>
                <div class="legal-cards" data-testid="legal-cards">
                    <a href=terms_href class="pub-card pub-card-link" data-testid="legal-card-terms">
                        <h2 class="pub-h2">{terms_title}</h2>
                        <p class="pub-card-desc">{terms_desc}</p>
                        <span class="pub-muted">{format!("{updated_label}: {LEGAL_TERMS_VERSION}")}</span>
                    </a>
                    <a href=privacy_href class="pub-card pub-card-link" data-testid="legal-card-privacy">
                        <h2 class="pub-h2">{privacy_title}</h2>
                        <p class="pub-card-desc">{privacy_desc}</p>
                        <span class="pub-muted">{format!("{updated_label}: {LEGAL_PRIVACY_VERSION}")}</span>
                    </a>
                    <a href=licenses_href class="pub-card pub-card-link" data-testid="legal-card-licenses">
                        <h2 class="pub-h2">{licenses_title}</h2>
                        <p class="pub-card-desc">{licenses_desc}</p>
                        <span class="pub-muted">{format!("{updated_label}: {LEGAL_TERMS_VERSION}")}</span>
                    </a>
                </div>
                <div class="legal-contact">
                    <p class="pub-muted">
                        {format!("{contact}: ")}
                        <a href="mailto:legal@context-os.com">"legal@context-os.com"</a>
                    </p>
                </div>
                <LegalFooterLinks locale=locale/>
            </div>
        </main>
    }
}

#[component]
pub fn LegalTermsPageView(locale: &'static str) -> impl IntoView {
    let en = locale == "en";
    let (html, toc) = render_doc(if en { TERMS_EN_MD } else { TERMS_ZH_MD });
    let (seo_title, seo_description, canonical) = if en {
        (
            "Terms of Service",
            "Full text of the Context-OS Terms of Service (Chinese version prevails).",
            "/en/legal/terms",
        )
    } else {
        (
            "用户服务协议 - Context OS",
            "Context-OS 用户服务协议，了解使用我们服务的条款与条件。",
            "/legal/terms",
        )
    };
    view! {
        <Title text=seo_title/>
        <Meta name="description" content=seo_description/>
        <Link rel="canonical" href=canonical/>
        <Link rel="alternate" hreflang="zh-CN" href="/legal/terms"/>
        <Link rel="alternate" hreflang="en" href="/en/legal/terms"/>
        <Link rel="alternate" hreflang="x-default" href="/legal/terms"/>
        <main class="pub-shell" data-testid=if en { "legal-terms-page-en" } else { "legal-terms-page" }>
            <div class="pub-center">
                <LegalShell locale=locale title=if en { "Terms of Service" } else { "用户服务协议" } updated=Some(LEGAL_TERMS_VERSION) version=Some(LEGAL_TERMS_VERSION) toc=Some(toc)>
                    <div class="legal-document" inner_html=html></div>
                </LegalShell>
            </div>
        </main>
    }
}

#[component]
pub fn LegalPrivacyPageView(locale: &'static str) -> impl IntoView {
    let en = locale == "en";
    let (html, toc) = render_doc(if en { PRIVACY_EN_MD } else { PRIVACY_ZH_MD });
    let (seo_title, seo_description, canonical) = if en {
        (
            "Privacy Policy",
            "Context-OS Privacy Policy: how we collect, use, and protect your personal information.",
            "/en/legal/privacy",
        )
    } else {
        (
            "隐私政策 - Context OS",
            "Context-OS 隐私政策，了解我们如何收集、使用和保护您的个人信息。",
            "/legal/privacy",
        )
    };
    view! {
        <Title text=seo_title/>
        <Meta name="description" content=seo_description/>
        <Link rel="canonical" href=canonical/>
        <Link rel="alternate" hreflang="zh-CN" href="/legal/privacy"/>
        <Link rel="alternate" hreflang="en" href="/en/legal/privacy"/>
        <Link rel="alternate" hreflang="x-default" href="/legal/privacy"/>
        <main class="pub-shell" data-testid=if en { "legal-privacy-page-en" } else { "legal-privacy-page" }>
            <div class="pub-center">
                <LegalShell locale=locale title=if en { "Privacy Policy" } else { "隐私政策" } updated=Some(LEGAL_PRIVACY_VERSION) version=Some(LEGAL_PRIVACY_VERSION) toc=Some(toc)>
                    <div class="legal-document" inner_html=html></div>
                </LegalShell>
            </div>
        </main>
    }
}

#[component]
pub fn LegalLicensesPageView(locale: &'static str) -> impl IntoView {
    let en = locale == "en";
    let (seo_title, seo_description, canonical) = if en {
        (
            "Open-source notices",
            "Open-source components used by Context-OS and a summary of their licenses.",
            "/en/legal/licenses",
        )
    } else {
        (
            "开源软件说明 - Context OS",
            "Context-OS 使用的开源组件及其许可证摘要。",
            "/legal/licenses",
        )
    };
    let prefix = if en { "/en/legal" } else { "/legal" };
    let project_href = format!("{prefix}/licenses/project");
    let third_party_href = format!("{prefix}/licenses/third-party");
    view! {
        <Title text=seo_title/>
        <Meta name="description" content=seo_description/>
        <Link rel="canonical" href=canonical/>
        <Link rel="alternate" hreflang="zh-CN" href="/legal/licenses"/>
        <Link rel="alternate" hreflang="en" href="/en/legal/licenses"/>
        <Link rel="alternate" hreflang="x-default" href="/legal/licenses"/>
        <main class="pub-shell" data-testid=if en { "legal-licenses-page-en" } else { "legal-licenses-page" }>
            <div class="pub-center">
                <LegalShell locale=locale title=if en { "Open-source notices" } else { "开源软件说明" } updated=Some("2026-08-05") version=None toc=None>
                    <div class="licenses-summary">
                        <section>
                            <h2 class="pub-h2">{if en { "Our product" } else { "我们的产品" }}</h2>
                            <p class="pub-p">
                                {if en {
                                    "The Context-OS server and web client are primarily built in-house; distribution follows the MIT license."
                                } else {
                                    "Context-OS服务端与Web客户端以自研为主；整体分发遵守MIT许可证。"
                                }}
                            </p>
                            <a href=project_href class="pub-btn">
                                {if en { "View the full MIT license" } else { "查看MIT许可证全文" }}
                            </a>
                        </section>
                        <section>
                            <h2 class="pub-h2">{if en { "Major open-source components" } else { "主要开源组件" }}</h2>
                            <div class="pub-table-wrap">
                                <table class="pub-table" data-testid="licenses-table">
                                    <thead>
                                        <tr>
                                            <th class="pub-th">{if en { "Category" } else { "类别" }}</th>
                                            <th class="pub-th">{if en { "Components" } else { "代表组件" }}</th>
                                            <th class="pub-th">{if en { "License" } else { "许可证" }}</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {if en {
                                            view! {
                                                <tr><td class="pub-td">"Web framework"</td><td class="pub-td">"Next.js, React"</td><td class="pub-td"><span class="license-badge">"MIT"</span></td></tr>
                                                <tr><td class="pub-td">"Backend runtime"</td><td class="pub-td">"Tokio, Axum"</td><td class="pub-td"><span class="license-badge">"MIT / Apache-2.0"</span></td></tr>
                                                <tr><td class="pub-td">"Vector search"</td><td class="pub-td">"Milvus · pgvector"</td><td class="pub-td"><span class="license-badge">"Apache-2.0 / PostgreSQL"</span></td></tr>
                                                <tr><td class="pub-td">"Document parsing"</td><td class="pub-td">"markitdown · firecrawl-anydoc"</td><td class="pub-td"><span class="license-badge">"MIT"</span></td></tr>
                                                <tr><td class="pub-td">"Client shell"</td><td class="pub-td">"Tauri 2"</td><td class="pub-td"><span class="license-badge">"MIT / Apache-2.0"</span></td></tr>
                                                <tr><td class="pub-td">"AI inference"</td><td class="pub-td">"DeepSeek, DashScope, etc."</td><td class="pub-td"><span class="license-badge">"Commercial API"</span></td></tr>
                                            }.into_any()
                                        } else {
                                            view! {
                                                <tr><td class="pub-td">"Web框架"</td><td class="pub-td">"Next.js, React"</td><td class="pub-td"><span class="license-badge">"MIT"</span></td></tr>
                                                <tr><td class="pub-td">"后端运行时"</td><td class="pub-td">"Tokio, Axum"</td><td class="pub-td"><span class="license-badge">"MIT / Apache-2.0"</span></td></tr>
                                                <tr><td class="pub-td">"向量检索"</td><td class="pub-td">"Milvus · pgvector"</td><td class="pub-td"><span class="license-badge">"Apache-2.0 / PostgreSQL"</span></td></tr>
                                                <tr><td class="pub-td">"文档解析"</td><td class="pub-td">"markitdown · firecrawl-anydoc"</td><td class="pub-td"><span class="license-badge">"MIT"</span></td></tr>
                                                <tr><td class="pub-td">"客户端壳"</td><td class="pub-td">"Tauri 2"</td><td class="pub-td"><span class="license-badge">"MIT / Apache-2.0"</span></td></tr>
                                                <tr><td class="pub-td">"AI推理"</td><td class="pub-td">"DeepSeek, DashScope 等"</td><td class="pub-td"><span class="license-badge">"商业API"</span></td></tr>
                                            }.into_any()
                                        }}
                                    </tbody>
                                </table>
                            </div>
                        </section>
                        <section>
                            <h2 class="pub-h2">{if en { "Weak-copyleft notes" } else { "弱copyleft说明" }}</h2>
                            <ul class="pub-list">
                                {if en {
                                    view! {
                                        <li><strong>"dompurify"</strong>": Use the Apache-2.0 build"</li>
                                        <li><strong>"cssparser"</strong>": MPL; unmodified use only requires a NOTICE"</li>
                                        <li><strong>"MinIO / Redis 7.4+ (server)"</strong>": See the commercial list in third-party notices: prefer cloud S3/OSS; use Valkey or Redis ≤7.2"</li>
                                    }.into_any()
                                } else {
                                    view! {
                                        <li><strong>"dompurify"</strong>": 选择Apache-2.0版本"</li>
                                        <li><strong>"cssparser"</strong>": MPL，未修改则仅需NOTICE"</li>
                                        <li><strong>"MinIO / Redis 7.4+（服务端）"</strong>": 见第三方声明商业清单：优先云 S3/OSS；Redis 用 Valkey 或 ≤7.2"</li>
                                    }.into_any()
                                }}
                            </ul>
                        </section>
                        <section>
                            <h2 class="pub-h2">{if en { "Full list" } else { "完整清单" }}</h2>
                            <div class="pub-btn-row">
                                <a href=third_party_href class="pub-btn">
                                    {if en { "View full third-party notices" } else { "查看完整第三方声明" }}
                                </a>
                                <a href="/legal/third-party-notices.md" download class="pub-btn">
                                    {if en { "Download Markdown" } else { "下载Markdown" }}
                                </a>
                            </div>
                        </section>
                        <section>
                            <h2 class="pub-h2">{if en { "Desktop client" } else { "客户端" }}</h2>
                            <p class="pub-p">
                                {if en {
                                    "The client shell uses Tauri 2 (MIT / Apache-2.0). Full installers may bundle portable PostgreSQL, pgvector, and Redis Windows ports (BSD-3-Clause historical ports, not SSPL); see runtime/THIRD_PARTY.txt in the install directory and the Desktop section of the full third-party notices. The About dialog also shows a summary."
                                } else {
                                    "客户端壳层使用 Tauri 2（MIT / Apache-2.0）。完整安装包可捆绑便携 PostgreSQL、pgvector 与 Redis Windows 端口（BSD-3-Clause 历史端口，非 SSPL），声明见安装目录 runtime/THIRD_PARTY.txt，以及完整第三方声明中的 Desktop 章节。About 对话框亦可查看摘要。"
                                }}
                            </p>
                        </section>
                    </div>
                </LegalShell>
            </div>
        </main>
    }
}

#[component]
pub fn LegalProjectLicensePageView(locale: &'static str) -> impl IntoView {
    let en = locale == "en";
    let (seo_title, seo_description, canonical) = if en {
        (
            "MIT License",
            "Full text of the MIT license used by the Context-OS project.",
            "/en/legal/licenses/project",
        )
    } else {
        (
            "MIT 许可证 - Context OS",
            "Context-OS 项目使用的 MIT 许可证全文。",
            "/legal/licenses/project",
        )
    };
    view! {
        <Title text=seo_title/>
        <Meta name="description" content=seo_description/>
        <Link rel="canonical" href=canonical/>
        <Link rel="alternate" hreflang="zh-CN" href="/legal/licenses/project"/>
        <Link rel="alternate" hreflang="en" href="/en/legal/licenses/project"/>
        <Link rel="alternate" hreflang="x-default" href="/legal/licenses/project"/>
        <main class="pub-shell" data-testid=if en { "legal-project-license-page-en" } else { "legal-project-license-page" }>
            <div class="pub-center">
                <LegalShell locale=locale title=if en { "MIT License" } else { "MIT许可证" } updated=None version=None toc=None>
                    <div class="license-content">
                        <pre class="pub-code license-text"><code>{PROJECT_LICENSE}</code></pre>
                    </div>
                </LegalShell>
            </div>
        </main>
    }
}

#[component]
pub fn LegalThirdPartyPageView(locale: &'static str) -> impl IntoView {
    let en = locale == "en";
    let (html, toc) = render_legal_markdown(THIRD_PARTY_MD);
    let total = count_third_party_entries(THIRD_PARTY_MD);
    let generated = third_party_generated_date(THIRD_PARTY_MD);
    let (seo_title, seo_description, canonical) = if en {
        (
            "Full third-party notices",
            "Complete list of all third-party open-source components used by Context-OS and their licenses.",
            "/en/legal/licenses/third-party",
        )
    } else {
        (
            "完整第三方组件声明 - Context OS",
            "Context-OS 使用的所有第三方开源组件及其许可证完整列表。",
            "/legal/licenses/third-party",
        )
    };
    view! {
        <Title text=seo_title/>
        <Meta name="description" content=seo_description/>
        <Link rel="canonical" href=canonical/>
        <Link rel="alternate" hreflang="zh-CN" href="/legal/licenses/third-party"/>
        <Link rel="alternate" hreflang="en" href="/en/legal/licenses/third-party"/>
        <Link rel="alternate" hreflang="x-default" href="/legal/licenses/third-party"/>
        <main class="pub-shell" data-testid=if en { "legal-third-party-page-en" } else { "legal-third-party-page" }>
            <div class="pub-center">
                <LegalShell locale=locale title=if en { "Full third-party notices" } else { "完整第三方组件声明" } updated=None version=None toc=Some(toc)>
                    <div class="third-party-notices">
                        <div class="notices-header">
                            <div class="notices-stats">
                                <p class="pub-muted">
                                    {format!("{}: {generated}", if en { "Generated" } else { "生成日期" })}
                                </p>
                                <p class="pub-muted" data-testid="third-party-total">
                                    {format!("{}: {total}+", if en { "Total components" } else { "组件总数" })}
                                </p>
                            </div>
                            <div class="pub-btn-row">
                                <a href="/legal/third-party-notices.md" download class="pub-btn">
                                    {if en { "Download .md" } else { "下载 .md" }}
                                </a>
                            </div>
                        </div>
                        <div class="legal-document" inner_html=html></div>
                        <div class="notices-footer">
                            <a href=if en { "/en/legal/licenses" } else { "/legal/licenses" }>
                                {if en { "Back to open-source summary" } else { "返回开源摘要" }}
                            </a>
                        </div>
                    </div>
                </LegalShell>
            </div>
        </main>
    }
}

#[component]
pub fn LegalCenterPage() -> impl IntoView {
    view! { <LegalCenterPageView locale="zh"/> }
}

#[component]
pub fn EnLegalCenterPage() -> impl IntoView {
    view! { <LegalCenterPageView locale="en"/> }
}

#[component]
pub fn LegalTermsPage() -> impl IntoView {
    view! { <LegalTermsPageView locale="zh"/> }
}

#[component]
pub fn EnLegalTermsPage() -> impl IntoView {
    view! { <LegalTermsPageView locale="en"/> }
}

#[component]
pub fn LegalPrivacyPage() -> impl IntoView {
    view! { <LegalPrivacyPageView locale="zh"/> }
}

#[component]
pub fn EnLegalPrivacyPage() -> impl IntoView {
    view! { <LegalPrivacyPageView locale="en"/> }
}

#[component]
pub fn LegalLicensesPage() -> impl IntoView {
    view! { <LegalLicensesPageView locale="zh"/> }
}

#[component]
pub fn EnLegalLicensesPage() -> impl IntoView {
    view! { <LegalLicensesPageView locale="en"/> }
}

#[component]
pub fn LegalProjectLicensePage() -> impl IntoView {
    view! { <LegalProjectLicensePageView locale="zh"/> }
}

#[component]
pub fn EnLegalProjectLicensePage() -> impl IntoView {
    view! { <LegalProjectLicensePageView locale="en"/> }
}

#[component]
pub fn LegalThirdPartyPage() -> impl IntoView {
    view! { <LegalThirdPartyPageView locale="zh"/> }
}

#[component]
pub fn EnLegalThirdPartyPage() -> impl IntoView {
    view! { <LegalThirdPartyPageView locale="en"/> }
}
