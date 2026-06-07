# Example Deck

**Topic**: Rust Ownership (technical, uses progressive disclosure)

```json
{
  "$schema_version": "1.0",
  "title": "Rust Ownership in 5 Slides",
  "language": "en",
  "slides": [
    {
      "title": "The problem with manual memory",
      "layout": "content",
      "bullets": [
        { "text": "C/C++ require manual malloc/free, prone to leaks and use-after-free.", "citations": [1] },
        { "text": "GC languages trade away performance and pause-time predictability.", "citations": [2] }
      ],
      "notes": "Hook: most audiences have felt the pain of at least one of these two camps."
    },
    {
      "title": "Ownership: one owner, one lifetime",
      "layout": "content",
      "bullets": [
        { "text": "Every value has exactly one owner.", "citations": [3] },
        { "text": "When the owner goes out of scope, the value is dropped automatically.", "citations": [3] },
        { "text": "Ownership can be moved, borrowed, or copied — never silently shared.", "citations": [4] }
      ],
      "notes": null
    },
    {
      "title": "Borrowing without copying",
      "layout": "content",
      "bullets": [
        { "text": "Immutable borrow (&T): many readers, no writers.", "citations": [5] },
        { "text": "Mutable borrow (&mut T): one writer, no readers.", "citations": [5] },
        { "text": "The borrow checker enforces these rules at compile time.", "citations": [5] }
      ],
      "notes": null
    },
    {
      "title": "Key takeaway",
      "layout": "content",
      "bullets": [
        { "text": "Ownership eliminates entire classes of memory bugs without a runtime GC.", "citations": [3] },
        { "text": "The compile-time guarantee comes with a learning curve — but it pays off.", "citations": [6] }
      ],
      "notes": "Close with confidence: the trade-off is intentional and well-documented."
    },
    {
      "title": "Questions?",
      "layout": "title",
      "bullets": [],
      "notes": null
    }
  ]
}
```

**Why this is good**:
- 5 slides = short deck, perfect for a quick overview.
- Progressive disclosure: problem → mechanism → mechanics → takeaway → Q&A.
- Technical topic starts with "Why" (the pain) before "How" (ownership rules).
- Citations map back to evidence chunks — every factual claim is grounded.
- Slide 5 uses `"layout": "title"` for a clean closing slide with no bullets.
- Speaker notes add value on slides 1 and 4; omitted (null) elsewhere.
