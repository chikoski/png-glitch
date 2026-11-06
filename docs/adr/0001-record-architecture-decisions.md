# 0001. Record architecture decisions

- **Status:** Accepted
- **Date:** 2026-11-06
- **Issue:** #115

## Context

The roadmap (#115 to #118) begins with a phase of design decisions that later API and performance work depends on: the design principles (#94) and where high-level effects live (#63). Later issues such as #68 (recipe vs. image), #70 (escape hatches), #73 (API tidy-up before 1.0) and #100 (live pipeline) will raise similar questions.

So far, decisions have been recorded in issue comments and in `DEVELOPER_GUIDE.md`. Issue threads are hard to find once closed and mix the decision with the discussion. `DEVELOPER_GUIDE.md` describes the current state but not the alternatives that were rejected, or why.

## Options

### Option A: Keep decisions in issues and DEVELOPER_GUIDE

No new process. The reasoning stays scattered, and there is no single place to check a pull request against.

### Option B: Architecture Decision Records in the repository

One short Markdown file per decision under `docs/adr/`, numbered, reviewed in a pull request, versioned with the code it affects.

## Decision

Use Architecture Decision Records (Option B), as described in [`docs/adr/README.md`](README.md):

- One file per decision, `docs/adr/NNNN-short-title.md`, from [`template.md`](template.md).
- Sections: Context, Options, Decision, Consequences.
- Status: Proposed, Accepted, Rejected, or Superseded. Accepted ADRs are not rewritten; a new ADR supersedes them.
- Living documents (the design principles, the architecture overview) stay in `DEVELOPER_GUIDE.md`. An ADR records the decision to adopt or change them and links there.

## Consequences

- Issues whose deliverable is a decision (for example #63) close with an ADR.
- Reviewers can link to an ADR instead of re-explaining a decision.
- A small amount of overhead per decision. The README lists when an ADR is not needed, to keep that overhead for decisions that matter.
