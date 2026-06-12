#!/usr/bin/env python3
"""Move contract type imports from common::* to contracts::* across avrag-rs workspace."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CRATES = ROOT / "crates"

CONTRACT_TYPES: dict[str, str] = {}
for mod, names in {
    "chat": [
        "AnswerBlock", "ChatDonePayload", "ChatMessage", "ChatMessageListResponse",
        "ChatRequest", "ChatResponse", "ChatTokenUsage", "ChatTurnInput", "Citation",
        "DegradeReason", "DegradeTraceItem", "GeneralPlan", "GuardAction", "GuardReport",
        "GuardResult", "MessageFeedbackRating", "MessageFeedbackRequest", "ModeDebug",
        "PlannerOutput", "RagModeDebug", "RagPlan", "RagPlanItem", "RagTraceItem",
        "RagTraceSummary", "RiskLevel", "SearchPlan", "SourceRef", "SummaryInjectionTrace",
        "TraceInfo",
    ],
    "documents": [
        "CitationLookupRequest", "CitationLookupResponse", "CreateDocumentUploadResponse",
        "DocumentStatus", "DocumentStatusResponse",
    ],
    "notebooks": [
        "ChatSession", "ChatSessionListResponse", "CreateChatSessionRequest",
        "CreateNotebookNoteRequest", "Notebook", "NotebookAnalysisAccess",
        "NotebookAnalysisAlert", "NotebookAnalysisNotes", "NotebookAnalysisOverview",
        "NotebookAnalysisResponse", "NotebookAnalysisSources", "NotebookAnalysisThreads",
        "NotebookListResponse", "NotebookNote", "NotebookNoteListResponse",
        "NotebookNoteResponse", "NotebookResponse", "PromoteNotebookNoteResponse",
        "UpdateChatSessionRequest", "UpdateNotebookNoteRequest",
    ],
    "preferences": [
        "AgentPreference", "AgentPreferenceMemory", "BlockedAgentPreference",
        "DailyPreferenceLog", "DashboardPreferences", "NotebookNotePreference",
        "NotebookWorkspacePreference", "NotificationPreferences", "UserPreferences",
        "WorkspaceDraftPreference",
    ],
}.items():
    for name in names:
        CONTRACT_TYPES[name] = mod


def split_use_items(inner: str) -> list[str]:
    items: list[str] = []
    depth = 0
    current: list[str] = []
    for ch in inner:
        if ch in "{[":
            depth += 1
        elif ch in "}]":
            depth -= 1
        elif ch == "," and depth == 0:
            item = "".join(current).strip()
            if item:
                items.append(item)
            current = []
            continue
        current.append(ch)
    item = "".join(current).strip()
    if item:
        items.append(item)
    return items


def join_use_items(items: list[str]) -> str:
    return ", ".join(items)


def process_use_common_block(block: str) -> tuple[str, bool]:
    """Transform a single use common::{...}; statement."""
    m = re.match(r"(\s*)use\s+common::(\{[\s\S]*?\}|[^;{]+);", block)
    if not m:
        return block, False

    indent, path = m.group(1), m.group(2).strip()
    if not path.startswith("{"):
        # use common::SingleType;
        name = path.strip()
        if name in CONTRACT_TYPES:
            mod = CONTRACT_TYPES[name]
            return f"{indent}use contracts::{mod}::{name};", True
        return block, False

    inner = path[1:-1]
    items = split_use_items(inner)
    common_items: list[str] = []
    by_mod: dict[str, list[str]] = {}

    for item in items:
        # handle `Type as Alias`
        base = item.split(" as ")[0].strip()
        if base in CONTRACT_TYPES:
            mod = CONTRACT_TYPES[base]
            by_mod.setdefault(mod, []).append(item)
        else:
            common_items.append(item)

    if not by_mod:
        return block, False

    lines: list[str] = []
    if common_items:
        lines.append(f"{indent}use common::{{{join_use_items(common_items)}}};")
    for mod in sorted(by_mod):
        lines.append(f"{indent}use contracts::{mod}::{{{join_use_items(by_mod[mod])}}};")
    return "\n".join(lines), True


def process_use_common_multiline(content: str) -> tuple[str, int]:
    pattern = re.compile(r"use\s+common::\{[^;]*\};", re.MULTILINE | re.DOTALL)
    changes = 0

    def repl(match: re.Match[str]) -> str:
        nonlocal changes
        new, changed = process_use_common_block(match.group(0))
        if changed:
            changes += 1
        return new

    return pattern.sub(repl, content), changes


def replace_qualified_paths(content: str) -> tuple[str, int]:
    changes = 0
    for name, mod in sorted(CONTRACT_TYPES.items(), key=lambda x: -len(x[0])):
        pattern = re.compile(rf"\bcommon::{re.escape(name)}\b")
        new_content, n = pattern.subn(f"contracts::{mod}::{name}", content)
        if n:
            changes += n
            content = new_content
    return content, changes


def ensure_contracts_dep(cargo_toml: Path) -> bool:
    text = cargo_toml.read_text()
    if "contracts" in text:
        return False
    if "[dependencies]" not in text:
        return False
    insertion = 'contracts = { path = "../../../contracts" }\n'
    text = text.replace("[dependencies]\n", f"[dependencies]\n{insertion}", 1)
    cargo_toml.write_text(text)
    return True


def file_needs_contracts(content: str) -> bool:
    return "contracts::" in content or "use contracts" in content


def process_file(path: Path) -> tuple[int, bool]:
    original = path.read_text()
    content = original
    use_changes = 0
    qual_changes = 0

    content, use_changes = process_use_common_multiline(content)
    content, qual_changes = replace_qualified_paths(content)

    total = use_changes + qual_changes
    if content != original:
        path.write_text(content)
    return total, file_needs_contracts(content)


def main() -> int:
    total_changes = 0
    crates_needing_dep: set[str] = set()

    for rs in sorted(CRATES.rglob("*.rs")):
        if "target" in rs.parts:
            continue
        n, needs = process_file(rs)
        if n:
            total_changes += n
            print(f"  {rs.relative_to(ROOT)}: {n} fixes")
        if needs:
            crate = rs.relative_to(CRATES).parts[0]
            crates_needing_dep.add(crate)

    dep_adds = 0
    for crate in sorted(crates_needing_dep):
        cargo = CRATES / crate / "Cargo.toml"
        if cargo.exists() and ensure_contracts_dep(cargo):
            dep_adds += 1
            print(f"  added contracts dep to {crate}")

    print(f"\nTotal import fixes: {total_changes}")
    print(f"Cargo.toml updates: {dep_adds}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
