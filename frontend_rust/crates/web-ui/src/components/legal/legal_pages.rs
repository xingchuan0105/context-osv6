use super::legal_markdown::{render_legal_markdown, TocEntry};
use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};

/// 法务文档通用壳（标题 / 最后更新 / 版本 / 目录 / 返回法律中心），对齐 Next LegalLayout。
#[component]
pub fn LegalShell(
    title: &'static str,
    updated: Option<&'static str>,
    version: Option<&'static str>,
    toc: Option<Vec<TocEntry>>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="legal-layout" data-testid="legal-doc-layout">
            <header class="legal-header">
                <h1 class="legal-title">{title}</h1>
                {updated.map(|u| view! { <p class="legal-updated">{format!("最后更新：{u}")}</p> })}
                {version.map(|v| view! { <p class="legal-version">{format!("版本：{v}")}</p> })}
            </header>
            <div class="legal-body">
                {toc.filter(|entries| !entries.is_empty()).map(|entries| {
                    view! {
                        <nav class="legal-toc" aria-label="文档目录">
                            <p class="legal-toc-title">"目录"</p>
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
                <a href="/legal">"返回法律中心"</a>
            </footer>
        </div>
    }
}

/// 公开页脚：法务三链 + 版权（home/legal 公共）。
#[component]
pub fn LegalFooterLinks() -> impl IntoView {
    view! {
        <footer class="legal-footer-links" data-testid="legal-footer-links">
            <div class="legal-footer-content">
                <a href="/legal/terms">"用户协议"</a>
                <span class="legal-footer-separator">"·"</span>
                <a href="/legal/privacy">"隐私政策"</a>
                <span class="legal-footer-separator">"·"</span>
                <a href="/legal/licenses">"开源声明"</a>
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

#[component]
pub fn LegalCenterPage() -> impl IntoView {
    view! {
        <Title text="法律中心 - Context OS"/>
        <Meta name="description" content="Context OS 法律文档中心：用户服务协议、隐私政策与开源声明。"/>
        <Link rel="canonical" href="/legal"/>
        <Link rel="alternate" hreflang="zh-CN" href="/legal"/>
        <Link rel="alternate" hreflang="en" href="/en/legal"/>
        <Link rel="alternate" hreflang="x-default" href="/legal"/>
        <main class="pub-shell" data-testid="legal-center-page">
            <div class="pub-center">
                <div class="legal-center-header">
                    <h1 class="pub-title">"法律中心"</h1>
                    <p class="pub-subtitle">"使用Context-OS前请阅读以下文档"</p>
                </div>
                <div class="legal-cards" data-testid="legal-cards">
                    <a href="/legal/terms" class="pub-card pub-card-link" data-testid="legal-card-terms">
                        <h2 class="pub-h2">"用户服务协议"</h2>
                        <p class="pub-card-desc">"使用Context-OS服务前请阅读本协议"</p>
                        <span class="pub-muted">{format!("最后更新: {}", LEGAL_TERMS_VERSION)}</span>
                    </a>
                    <a href="/legal/privacy" class="pub-card pub-card-link" data-testid="legal-card-privacy">
                        <h2 class="pub-h2">"隐私政策"</h2>
                        <p class="pub-card-desc">"了解我们如何收集、使用和保护您的个人信息"</p>
                        <span class="pub-muted">{format!("最后更新: {}", LEGAL_PRIVACY_VERSION)}</span>
                    </a>
                    <a href="/legal/licenses" class="pub-card pub-card-link" data-testid="legal-card-licenses">
                        <h2 class="pub-h2">"开源声明"</h2>
                        <p class="pub-card-desc">"查看我们使用的开源组件及其许可证"</p>
                        <span class="pub-muted">{format!("最后更新: {}", LEGAL_TERMS_VERSION)}</span>
                    </a>
                </div>
                <div class="legal-contact">
                    <p class="pub-muted">
                        "如有法律问题，请联系: "
                        <a href="mailto:legal@context-os.com">"legal@context-os.com"</a>
                    </p>
                </div>
                <LegalFooterLinks/>
            </div>
        </main>
    }
}

pub const LEGAL_TERMS_VERSION: &str = "2026-06-13";
pub const LEGAL_PRIVACY_VERSION: &str = "2026-06-13";

#[component]
pub fn LegalTermsPage() -> impl IntoView {
    let (html, toc) = render_doc(TERMS_ZH_MD);
    view! {
        <Title text="用户服务协议 - Context OS"/>
        <Meta name="description" content="Context-OS 用户服务协议，了解使用我们服务的条款与条件。"/>
        <Link rel="canonical" href="/legal/terms"/>
        <Link rel="alternate" hreflang="zh-CN" href="/legal/terms"/>
        <Link rel="alternate" hreflang="en" href="/en/legal/terms"/>
        <Link rel="alternate" hreflang="x-default" href="/legal/terms"/>
        <main class="pub-shell" data-testid="legal-terms-page">
            <div class="pub-center">
                <LegalShell title="用户服务协议" updated=Some(LEGAL_TERMS_VERSION) version=Some(LEGAL_TERMS_VERSION) toc=Some(toc)>
                    <div class="legal-document" inner_html=html></div>
                </LegalShell>
            </div>
        </main>
    }
}

#[component]
pub fn LegalPrivacyPage() -> impl IntoView {
    let (html, toc) = render_doc(PRIVACY_ZH_MD);
    view! {
        <Title text="隐私政策 - Context OS"/>
        <Meta name="description" content="Context-OS 隐私政策，了解我们如何收集、使用和保护您的个人信息。"/>
        <Link rel="canonical" href="/legal/privacy"/>
        <Link rel="alternate" hreflang="zh-CN" href="/legal/privacy"/>
        <Link rel="alternate" hreflang="en" href="/en/legal/privacy"/>
        <Link rel="alternate" hreflang="x-default" href="/legal/privacy"/>
        <main class="pub-shell" data-testid="legal-privacy-page">
            <div class="pub-center">
                <LegalShell title="隐私政策" updated=Some(LEGAL_PRIVACY_VERSION) version=Some(LEGAL_PRIVACY_VERSION) toc=Some(toc)>
                    <div class="legal-document" inner_html=html></div>
                </LegalShell>
            </div>
        </main>
    }
}

#[component]
pub fn LegalLicensesPage() -> impl IntoView {
    view! {
        <Title text="开源软件说明 - Context OS"/>
        <Meta name="description" content="Context-OS 使用的开源组件及其许可证摘要。"/>
        <Link rel="canonical" href="/legal/licenses"/>
        <Link rel="alternate" hreflang="zh-CN" href="/legal/licenses"/>
        <Link rel="alternate" hreflang="en" href="/en/legal/licenses"/>
        <Link rel="alternate" hreflang="x-default" href="/legal/licenses"/>
        <main class="pub-shell" data-testid="legal-licenses-page">
            <div class="pub-center">
                <LegalShell title="开源软件说明" updated=Some("2026-08-05") version=None toc=None>
                    <div class="licenses-summary">
                        <section>
                            <h2 class="pub-h2">"我们的产品"</h2>
                            <p class="pub-p">"Context-OS服务端与Web客户端以自研为主；整体分发遵守MIT许可证。"</p>
                            <a href="/legal/licenses/project" class="pub-btn">"查看MIT许可证全文"</a>
                        </section>
                        <section>
                            <h2 class="pub-h2">"主要开源组件"</h2>
                            <div class="pub-table-wrap">
                                <table class="pub-table" data-testid="licenses-table">
                                    <thead>
                                        <tr>
                                            <th class="pub-th">"类别"</th>
                                            <th class="pub-th">"代表组件"</th>
                                            <th class="pub-th">"许可证"</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        <tr><td class="pub-td">"Web框架"</td><td class="pub-td">"Next.js, React"</td><td class="pub-td">"MIT"</td></tr>
                                        <tr><td class="pub-td">"后端运行时"</td><td class="pub-td">"Tokio, Axum"</td><td class="pub-td">"MIT / Apache-2.0"</td></tr>
                                        <tr><td class="pub-td">"向量检索"</td><td class="pub-td">"Milvus · pgvector"</td><td class="pub-td">"Apache-2.0 / PostgreSQL"</td></tr>
                                        <tr><td class="pub-td">"文档解析"</td><td class="pub-td">"markitdown · firecrawl-anydoc"</td><td class="pub-td">"MIT"</td></tr>
                                        <tr><td class="pub-td">"客户端壳"</td><td class="pub-td">"Tauri 2"</td><td class="pub-td">"MIT / Apache-2.0"</td></tr>
                                        <tr><td class="pub-td">"AI推理"</td><td class="pub-td">"DeepSeek, DashScope 等"</td><td class="pub-td">"商业API"</td></tr>
                                    </tbody>
                                </table>
                            </div>
                        </section>
                        <section>
                            <h2 class="pub-h2">"弱copyleft说明"</h2>
                            <ul class="pub-list">
                                <li><strong>"dompurify"</strong>": 选择Apache-2.0版本"</li>
                                <li><strong>"cssparser"</strong>": MPL，未修改则仅需NOTICE"</li>
                                <li><strong>"MinIO / Redis 7.4+（服务端）"</strong>": 见第三方声明商业清单：优先云 S3/OSS；Redis 用 Valkey 或 ≤7.2"</li>
                            </ul>
                        </section>
                        <section>
                            <h2 class="pub-h2">"完整清单"</h2>
                            <div class="pub-btn-row">
                                <a href="/legal/licenses/third-party" class="pub-btn">"查看完整第三方声明"</a>
                                <a href="/legal/third-party-notices.md" download class="pub-btn">"下载Markdown"</a>
                            </div>
                        </section>
                        <section>
                            <h2 class="pub-h2">"客户端"</h2>
                            <p class="pub-p">"客户端壳层使用 Tauri 2（MIT / Apache-2.0）。完整安装包可捆绑便携 PostgreSQL、pgvector 与 Redis Windows 端口（BSD-3-Clause 历史端口，非 SSPL），声明见安装目录 runtime/THIRD_PARTY.txt，以及完整第三方声明中的 Desktop 章节。About 对话框亦可查看摘要。"</p>
                        </section>
                    </div>
                </LegalShell>
            </div>
        </main>
    }
}

#[component]
pub fn LegalProjectLicensePage() -> impl IntoView {
    view! {
        <Title text="MIT 许可证 - Context OS"/>
        <Meta name="description" content="Context-OS 项目使用的 MIT 许可证全文。"/>
        <Link rel="canonical" href="/legal/licenses/project"/>
        <Link rel="alternate" hreflang="zh-CN" href="/legal/licenses/project"/>
        <Link rel="alternate" hreflang="en" href="/en/legal/licenses/project"/>
        <Link rel="alternate" hreflang="x-default" href="/legal/licenses/project"/>
        <main class="pub-shell" data-testid="legal-project-license-page">
            <div class="pub-center">
                <LegalShell title="MIT许可证" updated=None version=None toc=None>
                    <div class="license-content">
                        <pre class="pub-code license-text"><code>{PROJECT_LICENSE}</code></pre>
                    </div>
                </LegalShell>
            </div>
        </main>
    }
}

#[component]
pub fn LegalThirdPartyPage() -> impl IntoView {
    let (html, toc) = render_legal_markdown(THIRD_PARTY_MD);
    let total = count_third_party_entries(THIRD_PARTY_MD);
    let generated = third_party_generated_date(THIRD_PARTY_MD);
    view! {
        <Title text="完整第三方组件声明 - Context OS"/>
        <Meta name="description" content="Context-OS 使用的所有第三方开源组件及其许可证完整列表。"/>
        <Link rel="canonical" href="/legal/licenses/third-party"/>
        <Link rel="alternate" hreflang="zh-CN" href="/legal/licenses/third-party"/>
        <Link rel="alternate" hreflang="en" href="/en/legal/licenses/third-party"/>
        <Link rel="alternate" hreflang="x-default" href="/legal/licenses/third-party"/>
        <main class="pub-shell" data-testid="legal-third-party-page">
            <div class="pub-center">
                <LegalShell title="完整第三方组件声明" updated=None version=None toc=Some(toc)>
                    <div class="third-party-notices">
                        <div class="notices-header">
                            <div class="notices-stats">
                                <p class="pub-muted">{format!("生成日期: {generated}")}</p>
                                <p class="pub-muted" data-testid="third-party-total">{format!("组件总数: {total}+")}</p>
                            </div>
                            <div class="pub-btn-row">
                                <a href="/legal/third-party-notices.md" download class="pub-btn">"下载 .md"</a>
                            </div>
                        </div>
                        <div class="legal-document" inner_html=html></div>
                        <div class="notices-footer">
                            <a href="/legal/licenses">"返回开源摘要"</a>
                        </div>
                    </div>
                </LegalShell>
            </div>
        </main>
    }
}
