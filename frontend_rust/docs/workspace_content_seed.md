# Workspace Content Seed

This document defines the presentational seed content used for parity work, preview routes, and visual QA.
It is not the source of truth for live runtime data.
Its role is to stabilize screenshots and keep copy choices consistent during shell refactor work.

## 1. Notebook seed

- title: `Research Project Alpha`

## 2. Thread seed

Thread list sample order:

- `Generative AI trends 2024`
- `React Performance Optimization`
- `Vite build configurations`
- `Kubernetes vs Docker Swarm`
- `Figma to Code plugin features`
- `Tailwind grid system layout`

Preferred active thread for visual QA:

- `Generative AI trends 2024`

## 3. Chat seed

User prompt:

`Can you summarize the main findings from the uploaded Project Scope document?`

Assistant answer:

`Based on the Project Scope document, the main findings are:`

`1. Core Objective: The primary goal is to launch a unified dashboard by Q4 that integrates marketing and sales data.`

`2. Budget & Timeline: The allocated budget is $250,000, with a strict deadline of November 15th for the beta release.`

`3. Key Dependencies: The project heavily relies on the new API endpoints being delivered by the backend team by early September.`

`Would you like me to elaborate on the risks outlined in section 4?`

Citations:

- `1`
- `2`

Compose placeholder:

- `Ask a question about your sources...`

Compose mode seed chips for preview/debug:

- `RAG`
- `Chat`
- `Web`

## 4. Source seed

Source list samples:

- `Q3_Financial_Report.pdf`
- `Project_Scope_v2.docx`
- `Competitor Analysis - Wikipedia`
- `User_Research_Interviews.pdf`

Selected by default for visual QA:

- `Q3_Financial_Report.pdf`
- `Project_Scope_v2.docx`
- `User_Research_Interviews.pdf`

Unselected by default for visual QA:

- `Competitor Analysis - Wikipedia`

Source modal seed labels:

- file tab: `Upload File`
- link tab: `Web Link`
- text tab: `Paste Text`

Upload helper copy:

- `Drag and drop your files here`
- `Supports PDF, DOCX, TXT, CSV (Max 50MB)`

## 5. Note seed

Saved notes:

- title: `Summary of Q3 Goals`
  preview: `The main goal for Q3 is to increase user retention by 15% through targeted onboarding improvements and a new referral program.`
- title: `Key Risks Identified`
  preview: `Technical debt in the legacy payment system poses a significant risk to our planned rollout in November.`

Editor placeholder:

- title placeholder: `Note Title`
- body placeholder: `Start typing your note here...`

## 6. Empty-state seed

Thread empty:

- `No threads yet`

Thread search empty:

- `No matching threads`

Chat empty:

- `Start a new conversation or ask a question.`

Source empty:

- `No sources yet. Upload a file or add a URL to begin.`

Notes empty:

- `No saved notes yet. Capture your first idea to get started.`

## 7. Acceptance screenshot seed

For workspace screenshot comparison, prefer this fixed composition:

- notebook title visible
- one active thread
- one user message
- one assistant message with citations
- sources count visible
- notes list visible
- composer empty with placeholder

The screenshot should represent a normal in-progress research session, not an empty workspace.
