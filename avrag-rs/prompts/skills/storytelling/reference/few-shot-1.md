# Example: Storytelling Response

## Example 1: Analogy-based (matches SKILL.md default)

**User**: "How does a database index work?"

**Bad (dry)**: "A database index is a data structure that improves query speed. It works by creating a sorted copy of column values..."

**Good (analogy)**: "Imagine you're a librarian in a massive library with millions of books. A patron asks for every book published in 1997. Without an index, you'd have to check every single book—page by page. With an index (like a card catalog sorted by year), you walk directly to the 1997 shelf. That's what a database index does: it pre-sorts the data so the database doesn't have to scan every row."

---

## Example 2: Historical example

**User**: "Why does the QWERTY keyboard layout still exist?"

**Bad (dry)**: "QWERTY was designed to prevent typewriter jams. It persists due to network effects..."

**Good (history)**: "In 1873, Christopher Latham Sholes was designing a typewriter in Milwaukee, and he had a problem: when typists hit keys too fast in succession, the metal arms would jam. He rearranged the layout—pulling common letter pairs apart—to slow the typist down. That arrangement became QWERTY. By the time typewriters could handle any speed, QWERTY was already on every machine, every typing school, every finger muscle memory. Switching costs became too high. A 1936 study showed DVORAK was objectively faster, but QWERTY's incumbency won."

---

## Example 3: Tension / curiosity

**User**: "Why do some programming languages use garbage collection while others don't?"

**Bad (dry)**: "Garbage collection trades memory efficiency for programmer convenience..."

**Good (tension)**: "Here's a paradox: a language with no garbage collector (like C) is faster but crash-prone; a language with one (like Java) is safer but slower. Why would anyone pick the slower one?

In 1995, a team at Sun Microsystems was building a language for interactive TV. They realized that the bottleneck wasn't the runtime—it was the programmers. Programmers spent 40% of their time chasing memory bugs that a runtime could fix automatically. The performance hit was real; the productivity gain was bigger. Trade accepted.

The same trade has played out in every GC language since: Python, JavaScript, Go, Java. Different choices, same bet: the machine should manage memory so humans can manage complexity."

---

## Example 4: When NOT to use storytelling

**User**: "What is the capital of France?"

**Bad (over-narrative)**: "Centuries ago, in the heart of Europe, a city rose to prominence. Its winding streets..."

**Good (direct, no story)**: "Paris."
