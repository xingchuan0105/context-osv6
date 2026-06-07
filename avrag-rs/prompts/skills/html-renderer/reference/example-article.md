# Example: Typography Article

**What it renders**: A long-form article with good typographic hierarchy and reading rhythm.

```html
<article class="html-article-b2e9">
  <style>
    .html-article-b2e9 {
      font-family: Georgia, "Times New Roman", serif;
      line-height: 1.7;
      color: #1f2328;
      max-width: 42rem;
      margin: 0 auto;
      padding: 1rem;
    }
    .html-article-b2e9 h1 {
      font-family: system-ui, sans-serif;
      font-size: 1.75rem;
      line-height: 1.3;
      margin: 0 0 0.75rem;
    }
    .html-article-b2e9 h2 {
      font-family: system-ui, sans-serif;
      font-size: 1.25rem;
      margin: 1.5rem 0 0.5rem;
    }
    .html-article-b2e9 p { margin: 0 0 1rem; }
    .html-article-b2e9 blockquote {
      border-left: 3px solid #d0d7de;
      margin: 1rem 0;
      padding-left: 1rem;
      color: #57606a;
    }
    .html-article-b2e9 code {
      font-family: ui-monospace, SFMono-Regular, monospace;
      background: #f6f8fa;
      padding: 0.125rem 0.25rem;
      border-radius: 4px;
      font-size: 0.9em;
    }
    .html-article-b2e9 ul { margin: 0 0 1rem 1.25rem; padding: 0; }
    .html-article-b2e9 li { margin-bottom: 0.25rem; }
    @media (max-width: 640px) {
      .html-article-b2e9 { padding: 0.75rem; }
      .html-article-b2e9 h1 { font-size: 1.5rem; }
    }
  </style>

  <h1>Understanding Rust Ownership</h1>
  <p>Rust's ownership system is the feature that most newcomers struggle with — and the feature that makes Rust uniquely safe without a garbage collector.</p>

  <h2>The Core Rule</h2>
  <p>Every value in Rust has a single owner. When the owner goes out of scope, the value is dropped. This sounds simple, but it has profound consequences.</p>

  <blockquote>
    "Ownership is Rust's most unique feature and has deep implications for the rest of the language."
  </blockquote>

  <h2>Three Key Behaviors</h2>
  <ul>
    <li><strong>Move</strong>: transferring ownership to another variable.</li>
    <li><strong>Borrow</strong>: temporary access via <code>&amp;T</code> (immutable) or <code>&amp;mut T</code> (mutable).</li>
    <li><strong>Copy</strong>: duplicate for types that are cheap to clone, like integers.</li>
  </ul>

  <p>Once these three behaviors become intuitive, most borrow-checker errors become easy to resolve.</p>
</article>
```

**Why this is good**:
- Semantic HTML: `<article>`, `<h1>`, `<h2>`, `<blockquote>`, `<ul>` instead of generic `<div>` soup.
- Namespaced under `.html-article-b2e9` so styles cannot leak.
- Responsive `@media` query for narrow viewports.
- Serif body + sans-serif headings creates typographic contrast.
- Max-width (`42rem`) and line-height (`1.7`) tuned for readability.
- No JavaScript — pure markup and CSS.
