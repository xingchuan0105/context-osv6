#!/usr/bin/env python3
"""One-shot typeshare + ts-rs annotations for contracts/src/chat.rs."""

from pathlib import Path

path = Path(__file__).resolve().parents[1] / "contracts" / "src" / "chat.rs"
text = path.read_text()

# Imports
if "use typeshare::typeshare" not in text:
    text = text.replace(
        "use serde::{Deserialize, Serialize};\n",
        "use serde::{Deserialize, Serialize};\nuse ts_rs::TS;\nuse typeshare::typeshare;\n",
    )

# typeshare on public struct/enum lines (except AnswerBlock, ChatEvent, DegradeReason handled separately)
import re

def add_typeshare_before_derive(match: re.Match[str]) -> str:
    block = match.group(0)
    if "AnswerBlock" in block or "ChatEvent" in block or "DegradeReason" in block:
        return block
    if "#[typeshare]" in block:
        return block
    return block.replace("#[derive(", "#[typeshare]\n#[derive(", 1)

text = re.sub(
    r"#\[derive\([^\]]+\)\]\n(?:#\[[^\]]+\]\n)*pub (?:struct|enum) \w+",
    add_typeshare_before_derive,
    text,
)

# DegradeReason
text = text.replace(
    "/// Stable degradation reason codes surfaced in `DegradeTraceItem.reason`.\n#[derive(Debug, Clone, PartialEq, Eq)]\npub enum DegradeReason {",
    '/// Serializes as a stable snake_case string on the wire.\n#[typeshare(serialized_as = "String")]\n#[derive(Debug, Clone, PartialEq, Eq)]\npub enum DegradeReason {',
)

# AnswerBlock ts-rs
text = text.replace(
    "#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n#[serde(tag = \"type\", rename_all = \"lowercase\")]\npub enum AnswerBlock {",
    '#[derive(TS, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n#[ts(export, export_to = "../../frontend_next/lib/contracts/generated/answer_block.ts")]\n#[serde(tag = "type", rename_all = "lowercase")]\npub enum AnswerBlock {',
)

# ChatEvent ts-rs
text = text.replace(
    "#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]\n#[serde(tag = \"event\", rename_all = \"snake_case\")]\npub enum ChatEvent {",
    '''#[derive(TS, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[ts(export, export_to = "../../frontend_next/lib/contracts/generated/chat_event.ts")]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ChatEvent {''',
)

# ChatEvent field hints
replacements = [
    (
        "        #[serde(default)]\n        counts: BTreeMap<String, usize>,",
        '        #[serde(default)]\n        #[ts(type = "Record<string, number>")]\n        counts: BTreeMap<String, usize>,',
    ),
    (
        "        #[serde(default)]\n        sources_preview: Vec<ChatActivitySourcePreview>,",
        '        #[serde(default)]\n        #[ts(type = "Array<{ id: string; label: string; href?: string | null }>")]\n        sources_preview: Vec<ChatActivitySourcePreview>,',
    ),
    (
        "        #[serde(default)]\n        detail: Option<serde_json::Value>,",
        '        #[serde(default)]\n        #[ts(type = "unknown")]\n        detail: Option<serde_json::Value>,',
    ),
    (
        "        message_id: i64,\n        agent_type: String,",
        '        #[ts(type = "number")]\n        message_id: i64,\n        agent_type: String,',
    ),
    (
        "        message_id: i64,\n        content: String,",
        '        #[ts(type = "number")]\n        message_id: i64,\n        content: String,',
    ),
    (
        "        message_id: i64,\n        citations: Vec<serde_json::Value>,",
        '        #[ts(type = "number")]\n        message_id: i64,\n        #[ts(type = "Array<Record<string, unknown>>")]\n        citations: Vec<serde_json::Value>,',
    ),
    (
        "        message_id: i64,\n        payload: serde_json::Value,",
        '        #[ts(type = "number")]\n        message_id: i64,\n        #[ts(type = "Record<string, unknown>")]\n        payload: serde_json::Value,',
    ),
]
for old, new in replacements:
    text = text.replace(old, new)

# Integer field annotations for typeshare (struct fields only)
lines = text.splitlines()
out: list[str] = []
enum_depth = 0
for line in lines:
    stripped = line.rstrip()
    if enum_depth == 0 and re.match(r"^\s*pub enum \w+", stripped):
        enum_depth = max(1, stripped.count("{") - stripped.count("}"))
        out.append(line)
        continue
    if enum_depth > 0:
        enum_depth += stripped.count("{") - stripped.count("}")
        out.append(line)
        continue
    if re.search(r":\s*(?:Option<)?(?:i64|u64|usize|isize)(?:>)?\s*,?\s*$", stripped):
        if "serialized_as" not in "\n".join(out[-3:]):
            indent = re.match(r"^(\s*)", stripped).group(1)
            out.append(f'{indent}#[typeshare(serialized_as = "number")]')
    elif re.search(r":\s*(?:std::collections::)?(?:HashMap|BTreeMap)<String,\s*i64>", stripped):
        if "serialized_as" not in "\n".join(out[-3:]):
            indent = re.match(r"^(\s*)", stripped).group(1)
            out.append(f'{indent}#[typeshare(serialized_as = "Record<string, number>")]')
    elif re.search(r":\s*(?:std::collections::)?BTreeMap<String,\s*usize>", stripped):
        if "serialized_as" not in "\n".join(out[-3:]):
            indent = re.match(r"^(\s*)", stripped).group(1)
            out.append(f'{indent}#[typeshare(serialized_as = "Record<string, number>")]')
    elif re.search(r":\s*Vec<usize>", stripped):
        if "serialized_as" not in "\n".join(out[-3:]):
            indent = re.match(r"^(\s*)", stripped).group(1)
            out.append(f'{indent}#[typeshare(serialized_as = "number[]")]')
    out.append(line)

path.write_text("\n".join(out) + "\n")
print("patched chat.rs")
