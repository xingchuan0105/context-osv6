# Example: Simple Dashboard

**What it renders**: A 3-column metric dashboard with an inline SVG bar chart.

```html
<div class="html-dash-7a3f">
  <style>
    .html-dash-7a3f { font-family: system-ui, sans-serif; padding: 1rem; }
    .html-dash-7a3f h2 { margin: 0 0 1rem; font-size: 1.25rem; }
    .html-dash-7a3f .grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 1rem; }
    .html-dash-7a3f .card { background: #f6f8fa; border-radius: 8px; padding: 1rem; }
    .html-dash-7a3f .label { color: #57606a; font-size: 0.875rem; }
    .html-dash-7a3f .value { font-size: 1.5rem; font-weight: 600; margin-top: 0.25rem; }
    .html-dash-7a3f svg { width: 100%; height: 120px; margin-top: 1rem; }
    .html-dash-7a3f rect.bar { fill: #0969da; }
    .html-dash-7a3f text { font-size: 10px; fill: #57606a; }
  </style>

  <h2>Weekly Traffic</h2>
  <div class="grid">
    <div class="card">
      <div class="label">Visitors</div>
      <div class="value">12.4K</div>
    </div>
    <div class="card">
      <div class="label">Page Views</div>
      <div class="value">38.1K</div>
    </div>
    <div class="card">
      <div class="label">Bounce Rate</div>
      <div class="value">34%</div>
    </div>
  </div>

  <svg viewBox="0 0 300 120" preserveAspectRatio="none" role="img" aria-label="Bar chart of daily visits">
    <g transform="translate(0, 100) scale(1, -1)">
      <rect class="bar" x="10"  y="0" width="25" height="45" />
      <rect class="bar" x="45"  y="0" width="25" height="72" />
      <rect class="bar" x="80"  y="0" width="25" height="60" />
      <rect class="bar" x="115" y="0" width="25" height="90" />
      <rect class="bar" x="150" y="0" width="25" height="55" />
      <rect class="bar" x="185" y="0" width="25" height="80" />
      <rect class="bar" x="220" y="0" width="25" height="65" />
    </g>
    <text x="22"  y="115">Mon</text>
    <text x="57"  y="115">Tue</text>
    <text x="92"  y="115">Wed</text>
    <text x="127" y="115">Thu</text>
    <text x="162" y="115">Fri</text>
    <text x="197" y="115">Sat</text>
    <text x="232" y="115">Sun</text>
  </svg>
</div>
```

**Why this is good**:
- Single unique class prefix (`.html-dash-7a3f`) prevents CSS leaking to host.
- Semantic labels + `role="img"` + `aria-label` for accessibility.
- Inline SVG is self-contained, no external resources.
- Responsive grid works at 320 px and up.
- Total size: ~1.5 KB.
