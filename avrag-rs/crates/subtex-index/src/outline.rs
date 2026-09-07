use anyhow::Result;
use subtex_store_sqlite::SubtexStore;

const SUMMARY_MAX_CHARS: usize = 120;
const INDENT: &str = "  ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outline {
    pub text: String,
    pub token_count: usize,
    pub files: usize,
    pub truncated: bool,
}

/// Token-budgeted global outline (Aider repo-map spirit): file names, heading
/// trees, one-line summaries. Files render in path order; when the budget is
/// reached the remainder is dropped and reported.
pub fn render_outline(store: &SubtexStore, token_budget: usize) -> Result<Outline> {
    let files = store.outline_files()?;
    let mut parts: Vec<String> = Vec::new();
    let mut tokens = 0usize;
    let mut truncated = false;

    for (file_id, path, chunk_count) in &files {
        let mut entry = format!("- {path} ({chunk_count} chunks)\n");
        let mut rendered: Vec<String> = Vec::new();
        for heading in store.file_outline(file_id)? {
            for (depth, segment) in heading.split(" > ").enumerate() {
                if rendered.get(depth).map(String::as_str) == Some(segment) {
                    continue;
                }
                rendered.truncate(depth);
                rendered.push(segment.to_string());
                entry.push_str(&INDENT.repeat(depth + 1));
                entry.push_str("- ");
                entry.push_str(segment);
                entry.push('\n');
            }
        }
        if let Some(summary) = store
            .file_first_chunk_content(file_id)?
            .as_deref()
            .map(first_line_summary)
        {
            entry.push_str(INDENT);
            entry.push_str("- summary: ");
            entry.push_str(&summary);
            entry.push('\n');
        }

        let entry_tokens = avrag_llm::count_tokens(&entry);
        if tokens + entry_tokens > token_budget {
            truncated = true;
            break;
        }
        tokens += entry_tokens;
        parts.push(entry);
    }

    let mut text = parts.join("");
    if truncated {
        text.push_str(&format!("… outline truncated at token budget {token_budget}\n"));
    }
    Ok(Outline {
        text,
        token_count: tokens,
        files: parts.len(),
        truncated,
    })
}

fn first_line_summary(content: &str) -> String {
    let line = content.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let mut out: String = line.chars().take(SUMMARY_MAX_CHARS).collect();
    if line.chars().count() > SUMMARY_MAX_CHARS {
        out.push('…');
    }
    out
}
