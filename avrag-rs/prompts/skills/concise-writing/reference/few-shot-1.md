# Example: Concise Response

## Example 1: One-sentence definition (matches SKILL.md default)

**User**: "What is Rust?"

**Bad (verbose)**: "Rust is a systems programming language that has been developed with a strong focus on safety and performance, and it is important to note that it prevents segfaults..."

**Good (concise)**: "Rust is a systems programming language that prevents segfaults and guarantees thread safety while maintaining C-like performance."

## Example 2: Bullet list for parallel items (matches YES-LIST)

**User**: "What are the differences between TCP and UDP?"

**Bad (verbose)**: "TCP is connection-oriented, which means… UDP, on the other hand, is connectionless, which means…"

**Good (concise)**:
- **TCP**: connection-oriented, reliable, ordered, slower.
- **UDP**: connectionless, unreliable, unordered, faster.

## Example 3: Multi-step reasoning kept short

**User**: "Why does my Rust build fail with 'cannot move out of borrowed content'?"

**Bad (verbose)**: "There are several reasons why this error might occur. First, you should check whether…"

**Good (concise)**: The error means you're trying to take ownership of a value that was only lent to you. Either clone the value, return a reference, or restructure so ownership is moved cleanly. The compiler's error span usually points at the offending line.

## Example 4: Lead with the answer

**User**: "Should I use Python or Rust for a CLI tool?"

**Bad (verbose)**: "There are many factors to consider when choosing…"

**Good (concise)**: Use **Rust** for performance-critical or long-lived CLIs; use **Python** for one-off scripts. For a typical small CLI, Python's faster to write; for anything the user will run daily, Rust's faster to use.

## Example 5: Don't over-concise

**User**: "Explain how TCP's three-way handshake works."

**Bad (over-concise)**: "Client sends SYN, server replies SYN-ACK, client sends ACK."

**Good (concise + complete)**: TCP's three-way handshake establishes a connection in three steps:
1. Client sends **SYN** to server.
2. Server replies with **SYN-ACK**.
3. Client sends **ACK**; connection is now open.

The handshake guarantees both sides agree on initial sequence numbers before any data flows.
