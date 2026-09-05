use leptos::prelude::*;

/// 长文与提示词编写建议（应用内静态长文）。
#[component]
pub fn HelpWritePage() -> impl IntoView {
    view! {
        <div class="help-shell" data-testid="help-write-page">
            <header class="help-header">
                <a href="/help" class="settings-back-link">"← 返回帮助中心"</a>
                <h1 class="help-title">"长文与提示词编写建议"</h1>
            </header>
            <main class="help-content help-article">
                <section class="help-article-section">
                    <h2>"长文写作：分段与结构"</h2>
                    <p>
                        "长文投喂给知识库时，切分质量直接决定检索效果。建议按语义单元分段："
                        "一个段落只讲一件事，段首先给结论，再展开论证与细节。"
                    </p>
                    <ul>
                        <li>"优先使用标题层级组织内容，标题会被切片器用作语义边界。"</li>
                        <li>"避免在一段中混杂多个主题；表格与清单尽量保持原子化。"</li>
                        <li>"跨章节引用时写明出处章节名，便于回答时回溯。"</li>
                    </ul>
                </section>
                <section class="help-article-section">
                    <h2>"上下文投喂方式"</h2>
                    <p>
                        "对话中粘贴长文时，建议先给一句摘要再贴全文；"
                        "长期使用的资料应上传到工作区资料库，而不是反复粘贴在会话里。"
                    </p>
                    <ul>
                        <li>"会话内粘贴适合一次性讨论；资料库适合反复检索的事实性内容。"</li>
                        <li>"上传后可用工作区右侧资料与笔记轨确认切片是否就绪。"</li>
                    </ul>
                </section>
                <section class="help-article-section">
                    <h2>"提示词编写建议"</h2>
                    <p>"稳定的提示词通常包含四个部分，顺序建议如下："</p>
                    <ul>
                        <li>"角色与目标：说明助手扮演的角色与本次任务要达成的结果。"</li>
                        <li>"约束与边界：明确不要做什么、输出长度与语言等硬约束。"</li>
                        <li>"素材与出处：粘贴或指明需要依据的资料片段。"</li>
                        <li>"输出格式：指定结构（如小标题、列表、表格）便于直接使用。"</li>
                    </ul>
                    <p>
                        "让模型先复述关键约束再作答，可以显著减少跑题；"
                        "对事实性答案，要求标注来源并区分「资料中提到」与「模型推断」。"
                    </p>
                </section>
                <section class="help-article-section">
                    <h2>"检索与回答质量自查"</h2>
                    <ul>
                        <li>"答案缺少依据时，检查资料是否已上传并完成索引。"</li>
                        <li>"检索结果偏题时，尝试在问题中加入资料中的关键术语。"</li>
                        <li>"需要精确数字时，把原始表格一并上传，避免模型凭印象补全。"</li>
                    </ul>
                </section>
            </main>
        </div>
    }
}
