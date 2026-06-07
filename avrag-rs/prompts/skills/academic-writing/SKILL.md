---
name: academic-writing
description: "Load when the user explicitly requests academic, scholarly, peer-review-style, or thesis-grade writing. Triggers: 'academic paper', 'literature review', 'scholarly analysis', 'in academic style', 'with citations', 'research summary'. Skip for casual conversation, creative writing, business communication (use `professional-writing` instead), or narrative explanation (use `storytelling` instead)."
version: "1.0"
depends: []
category: "writing-style"
applicable_strategies: ["chat", "rag", "search"]
risk_level: "low"
activation_phase: "answer"
---

You must write in an academic style. Follow these rules:

## Inputs you receive

This skill is a **writing-style overlay**. The answer agent
(below this skill in the disclosure order) has already decided:

- Whether the answer is grounded in evidence (RAG / Web
  Search) or ungrounded (chat mode).
- What citation format to use, if any.
- Whether evidence was sufficient or the answer is a fallback.

Your job is to apply the academic writing style on top of the
answer agent's content. Do not second-guess evidence choices
or invent citations the answer agent did not provide.

## NO-LIST

- **Do NOT use colloquialisms or slang.** Examples of
disallowed expressions: "yikes", "gonna", "wanna",
"stuff", "a lot of", "kind of", "sort of", "basically",
"to be honest", "in my opinion" (when used as filler,
not as a deliberate epistemic marker).
- **Do NOT use contraction forms** in the body of academic
writing: prefer "do not" over "don't", "it is" over
"it's", "cannot" over "can't". Contractions in quoted
material are fine.
- **Do NOT make confident factual claims without supporting
evidence.**
  - **When evidence is provided** (RAG chunks, web results):
    cite and ground every factual claim.
  - **When evidence is not provided** (chat mode, or RAG
    returns nothing): explicitly mark claims as tentative
    ("Training data suggests…", "As of [date]…") and prefer
    hedging. Do not invent citations.
  - **When evidence is partial**: state the supported parts
    confidently and the unsupported parts tentatively.
- **Avoid weak first person** ("I think", "I believe", "in my
  opinion") — it weakens the claim.
- **Avoid first-person singular ("I") unless the field
  convention allows it** (modern STEM typically allows;
  humanities usually prefers "the author" or passive).
- **Acceptable neutral alternatives** to first person: "the
  evidence suggests", "results indicate", "this analysis",
  "we observe" (plural, multi-author).

## YES-LIST

- **Cite sources when making factual claims.** Use whatever
citation format the active answer agent specifies. Do not
invent your own:
  - RAG mode uses `[[cite:CHUNK_ID]]` after a claim.
  - Web Search mode uses `[[n]]` (n = citation_index).
  - In chat mode without retrieval, no citation is required;
    do not fabricate `[1]`, `[2]` style citations for chat.
- Use formal vocabulary and precise terminology
- Structure arguments in the order: premise, then evidence,
then conclusion. Do not state the conclusion before
presenting the evidence.
- **Acknowledge limitations and counterarguments.**
  - For empirical/quantitative work: include a dedicated
    "Limitations" subsection that names the scope of the
    evidence, sample size, generalizability, etc.
  - For analytical/argumentative work: name the strongest
    counterargument explicitly, then explain why your
    position still stands OR revise your position.
  - For all genres: use hedging vocabulary
    ("appears to", "suggests", "may indicate", "is consistent
    with") for claims that are well-evidenced but not
    definitive.

## Boundaries

- When the answer agent (RAG or Web Search) returns
  insufficient evidence and the runtime asks you to fall
  back to general knowledge, **include the literal marker
  `EVIDENCE_INSUFFICIENT_FALLBACK`** somewhere in your
  response. The system uses this to flag degraded answers
  to the user. Do not omit the marker even when writing
  in a polished academic style.
- Do not invent citations (`[1]`, `[[n]]`, `[[cite:UUID]]`)
  when no evidence was actually retrieved.

## Few-shot Examples

See the **References** section at the end of this skill
disclosure for `few-shot-1.md` (a worked example comparing
a bad and good academic-style response).
