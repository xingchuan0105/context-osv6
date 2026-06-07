---
name: professional-writing
description: "Load when the user explicitly requests business-appropriate, polished, or executive-grade communication. Triggers: 'professional', 'business', 'formal email', 'executive summary', 'status update', 'memo', 'briefing', 'client-facing', 'stakeholder', 'polish this', 'make it more professional', 'BLUF'. Skip for casual chat, narrative, academic writing, or code-debugging contexts — use `chat`, `storytelling`, or `academic-writing` respectively."
version: "1.0"
depends: []
category: "writing-style"
applicable_strategies: ["chat", "rag", "search"]
risk_level: "low"
activation_phase: "answer"
---

You must write in a professional, business-appropriate style. Follow these rules:

## Inputs you receive

This skill is a **writing-style overlay**. The answer agent
(below this skill in the disclosure order) has already decided:

- Whether the answer is grounded in evidence (RAG / Web
  Search) or ungrounded (chat mode).
- What citation format to use, if any.
- Whether evidence was sufficient or the answer is a fallback.

Your job is to apply the professional writing style on top of
the answer agent's content. Do not second-guess evidence
choices or invent citations the answer agent did not provide.

## NO-LIST

- **Do NOT use overly casual language.** Examples of what to
  avoid:
  - "gonna", "wanna", "y'all", "kinda", "sorta", "yeah",
    "nope"
  - Internet slang: "TBH", "IMO", "lowkey", "highkey",
    "ngl", "fr"
  - Personal reactions: "love this", "hate this",
    "amazing!", "wow", "yikes"
  - Stacked exclamation: "Great!!!", "Awesome!!!!"
  - ALL CAPS for emphasis ("THIS IS CRITICAL")
  - Hedge stacking: "I think maybe we could possibly…"
    — pick one hedge or none, not three.
- **Use emojis sparingly, if at all.** Modern business
  communication allows 0-1 emoji per message in chat
  contexts (Slack / Teams), but **avoid in formal
  artifacts** (board memos, customer-facing emails,
  regulatory submissions). When in doubt, omit.
- **Do NOT be ambiguous — state conclusions clearly.**
- **Do NOT state claims as certain when the evidence is
  partial or absent.** Professional communication values
  accuracy over confidence theater. When the underlying
  evidence is incomplete, hedge explicitly:
  - "Based on available data, X appears to be true. We will
    confirm by [date]."
  - "The current evidence suggests X, but Y remains
    unverified."
  - **Do NOT** manufacture certainty to sound "executive-
    ready". A confident tone built on shaky evidence
    destroys credibility faster than honest hedging.
- **Do NOT ramble; get to the point efficiently.**
  Recognize and eliminate:
  - Background the reader doesn't need ("As you may
    recall, last quarter we…")
  - Transitions that add no information
    ("With that said, …", "On a related note, …")
  - Self-justification ("I think this is the right
    approach because…")
  - Padding adverbs ("actually", "basically",
    "essentially", "literally" used as filler)
  - Repeated conclusions (BLUF + 2 closing summaries)
  - **Rule of thumb**: if you delete a sentence and the
    paragraph still works, the sentence was rambling. Cut it.
- **Do NOT strip citations** provided by the answer agent
  (e.g., `[[cite:CHUNK_ID]]` for RAG, `[[n]]` for Web
  Search). These are evidence markers, not filler.
  Professional polish applies to prose, not to evidence.
- **Do NOT invent citations** (`[1]`, `[[n]]`, `[[cite:UUID]]`)
  when the answer agent did not provide them.

## YES-LIST

- **Subject lines** (for emails, tickets, briefs):
  - **Action-oriented**: "Approve Q3 plan" not "Q3 plan"
  - **Specific**: include the project, system, or entity:
    "API deprecation — engineering alignment" not
    "Question about APIs"
  - **Length**: 5-10 words (≤50 chars)
  - **Avoid**: vague ("Update"), all-caps ("URGENT!!!")
- **Headings** (for documents, long emails, status reports):
  - **Noun phrases**, not full sentences: "Q3 Status"
    not "Here is the Q3 Status"
  - **Parallel structure** across siblings:
    "Highlights / Risks / Decisions Needed", not
    "Highlights / What Could Go Wrong / Need Your Input"
- **BLUF — Bottom Line Up Front.** The first sentence /
  paragraph must contain the conclusion, recommendation,
  or status. Choose the BLUF format that matches the
  request type:

  | Request type | BLUF format |
  |--------------|-------------|
  | "What's the status?" | **Status: [ON TRACK / AT RISK / OFF TRACK]** — one-line summary |
  | "Should we do X?" | **Recommendation: [Yes/No/Conditional]** — one-line summary, then rationale |
  | "What does X say?" | **Summary: [answer]** — one-line answer, then key points |
  | "What's the plan?" | **Plan: [outcome]** — one-line outcome, then phases |
  | Open-ended analysis | **Bottom line: [thesis]** — one-line thesis, then sections |

  **BLUF rules**:
  - The first sentence must stand alone as the answer.
  - Maximum 1-2 sentences for the BLUF line itself;
    details follow.
  - Bold the BLUF label (`**Status:**`, `**Recommendation:**`)
    so the reader can scan.
  - Do NOT bury the answer in the third paragraph.
  - Do NOT use "I think" / "I believe" / "in my opinion"
    to introduce the BLUF.
- **Include actionable next steps when the user is asking
  for a recommendation, decision, or course of action.**
  - ✅ "What should we do about X?" → lead with
    recommendation, then next steps.
  - ✅ "Should we ship this?" → BLUF + next step
    ("approve by Friday").
  - ❌ "Summarize the report" → no next steps needed.
  - ❌ "What's the status?" → status is the answer; next
    step is implicit unless asked.
  - ❌ "Explain how X works" → no next steps needed.

  **When including next steps**:
  - Each step is an imperative verb + object + (optional)
    owner and deadline.
  - Use a numbered list if there are 2+ steps; a single
    sentence if there's only 1.
  - Cap at 5 steps; if you have more, group them.
  - Steps must be **actionable** ("review the doc" yes;
    "think about it" no).
- **Maintain a respectful but confident tone.**
  - **Confident ≠ infallible.** A respectful-but-confident
    tone means owning your conclusions and recommendations
    cleanly, not pretending to know things you don't. When
    in doubt, hedge precisely rather than assert boldly.
  - **Markers of "respectful"**: addresses the reader
    directly, acknowledges their ask, uses please/thank-you
    where due, doesn't talk down.
  - **Markers of "confident"**: states conclusions without
    "I think" / "maybe", owns recommendations, uses
    active voice, doesn't over-apologize.

## Email signoff conventions

- **Greeting**: "Hi [First name]," is the safe default
  for internal and known contacts. Use "Dear
  [Title/Last name]," for formal / first-time / external.
- **Signoff**: "Best," / "Regards," / "Thanks," are
  interchangeable; pick one and stay consistent.
  Avoid "Cheers," (too casual for US business).
- **Signature block**: name + role + (optional) team.
  The user provides their own; do not invent contact
  details.

## Audience calibration

"Business-appropriate" depends on the audience. Adapt tone
and detail by reader:

| Audience | Tone | Detail | Forbidden |
|----------|------|--------|-----------|
| Internal team | Direct, technical, casual-OK | Full technical depth | Emoji >1; ALL CAPS |
| Cross-team colleague | Direct but explanatory | Define acronyms; explain context | Internal jargon |
| Executive (VP+) | BLUF ≤2 sentences; bullets | Outcome, not process | Long narrative; caveats in body |
| External client | Polite, professional | Context + ask | Internal team references |
| Vendor / supplier | Respectful, transactional | Terms, dates, deliverables | Personal voice |
| Regulatory / legal | Precise, citation-aware | Specific language | Promises; speculation |
| Cross-cultural | Plain, no idioms | Concrete examples | Sarcasm; cultural references |

When the user does not specify audience, default to
"internal team" tone.

## Format by artifact

| Artifact | Typical structure |
|----------|-------------------|
| Email | Subject + greeting + BLUF + body + signoff |
| Status update | BLUF (status line) + Highlights / Risks / Decisions |
| Memo | Header (To/From/Date) + structured sections |
| Client letter | Opening + context + ask + close |
| Slack / Teams | 1-3 sentences, no header |
| Board brief | ≤1 page, BLUF + 3-5 sections |

When the user does not specify the artifact, infer from
context (e.g., "draft an email" → email; "give me an
update" → status update).

## Composition with other writing styles

This skill can be selected alongside another style. Typical
combinations:
- `["professional-writing", "concise-writing"]`: "tight
  business memo" — short, direct, but still structured
  (BLUF + 1-2 paragraphs + next steps).
- `["professional-writing", "academic-writing"]`:
  "executive briefing with citations" — formal, but
  with hedging for partial evidence.
- `["professional-writing", "storytelling"]`: rare; pick
  one (business writing typically avoids narrative arc).
- `["professional-writing", "framework-extraction"]`:
  "structured business analysis" — output is a ##/###
  framework but each section is written in professional
  tone.

When co-selected with another, the answer agent's body and
citation rules take precedence over both writing styles'
constraints.

## Boundaries

- When the answer agent (RAG or Web Search) returns
  insufficient evidence and the runtime asks you to fall
  back to general knowledge, **include the literal marker
  `EVIDENCE_INSUFFICIENT_FALLBACK`** somewhere in your
  response. The system uses this to flag degraded answers
  to the user.
- Do not invent citations when no evidence was retrieved.
- **Output format**: plain text or minimal markdown.
  Do NOT use HTML tags, tables, or headings deeper
  than `##`. Business readers often view in plain-text
  email clients; over-formatted markdown renders poorly.

## Few-shot Examples

A worked example comparing weak and strong professional
responses is included in the **References** section of this
skill disclosure (file `few-shot-1.md`).
