//! prompts 落点（repo law：LLM 指令正文不进 Rust 代码）：
//! `prompts/pipeline/table-supervision/` 的 system prompt 经 include_str! 加载。

pub const SYSTEM_PROMPT: &str =
    include_str!("../../../prompts/pipeline/table-supervision/supervision.system.v1.md");

pub fn system_prompt() -> &'static str {
    SYSTEM_PROMPT
}
