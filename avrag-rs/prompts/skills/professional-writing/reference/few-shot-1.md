# Example: Professional Business Response

## Example 1: Email with subject line and BLUF

**User**: "Draft an email to engineering about the Q3 API deprecation timeline."

**Bad (vague prose)**: "Hi team, I wanted to reach out to talk about the API. There are some things we should think about regarding how things are changing. Let me know if you have any thoughts."

**Good (professional)**:

**Subject**: Q3 API deprecation — engineering alignment needed by Jun 20

Hi team,

**Bottom line**: We're deprecating v1 of the public API on **Sep 30**. Engineering needs to confirm the migration plan by **Jun 20** so we can communicate timelines to customers in July.

**What's changing**:
- v1 endpoints return 410 Gone after Sep 30 (not 404 — important for client retry logic).
- The `/v2/migrate` helper endpoint will be available Aug 1.
- Existing v1 API keys continue to work but cannot be re-issued after Aug 15.

**Action items**:
- @platform: confirm migration plan in #api-migration by Jun 20.
- @docs: draft customer-facing migration guide by Jul 10.
- @support: prepare v1→v2 FAQ by Aug 1.

**Next step**: please reply with blockers or "ship it" by EOD Friday.

Thanks,
[Name]

---

## Example 2: Executive status update (BLUF + structured)

**User**: "Give me a status update on the database migration project for the VP."

**Bad (rambling)**: "So we've been working on the database migration for a while. There are a lot of things happening. Some are going well, some are not. Let me walk you through everything that's been going on..."

**Good (professional)**:

**Status: ON TRACK** — with 2 risks.

**Highlights**:
- 60% of services migrated (was 40% last week).
- Zero data-loss incidents in staging.
- Cutover plan approved by Security on Jun 5.

**Risks**:
- **R1**: legacy service X has 3 blocking compatibility issues (engineering review by Jun 12).
- **R2**: rollback procedure not yet tested at scale (drill planned Jun 15).

**Decisions needed**:
- Approve extending the cutover window from 4h to 8h (recommend: yes; reduces R2 risk).

**Next sync**: Jun 19, 14:00.

---

## Example 3: Polite but clear pushback (respectful + confident)

**User**: "Help me respond to a vendor who's asking for a 90-day payment extension."

**Bad (too soft, ambiguous)**: "Thanks so much for reaching out! We really appreciate your partnership and value everything you do. We will definitely look into this and get back to you as soon as we can. Thanks again!"

**Good (respectful + clear)**:

**Subject**: Re: payment terms — request declined

Hi [Vendor name],

Thank you for the proposal. After reviewing our Q3 cash position, **we're not able to extend payment terms beyond the current Net 30**.

We're happy to discuss alternative arrangements that work within our current cycle:
- Volume discount in exchange for a 12-month commitment.
- Quarterly payment plan with three equal installments.

If either of these works, let's set up 30 minutes next week.

Best,
[Name]

---

## Example 4: When to NOT add next steps

**User**: "Summarize the key findings from this report."

**Bad (over-eager)**:
Key findings: [...].
**Next step**: review the full report and share feedback by Friday.

**Good (no forced next step)**:
Key findings:
- [...]
- [...]

(Do not add "Next step" boilerplate when the user only asked for a summary. The "when appropriate" caveat in YES-LIST means the next step is omitted unless the user signals one is wanted — e.g., "summarize AND tell me what to do next".)
