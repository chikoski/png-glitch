# Architecture Decision Records

This directory records the significant design decisions made in `png-glitch`: what was decided, why, and what follows from it. The format follows [ADR 0001](0001-record-architecture-decisions.md).

## Index

| ADR | Title | Status | Issue |
| :-- | :---- | :----- | :---- |
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | Accepted | #115 |
| [0002](0002-design-principles.md) | Adopt "a valid PNG with strange data inside" as the design principle | Accepted | #94 |
| [0003](0003-home-of-high-level-effects.md) | Where high-level effects live | Accepted | #63 |
| [0004](0004-png-glitch-and-recipe-crates.md) | Split responsibilities between `png-glitch` and `png-glitch-recipe` | Accepted | #115 |
| [0005](0005-live-pipeline.md) | The live pipeline: where it lives, what `G` may touch, per-frame seeds and generations | Accepted | #100 |
| [0006](0006-pixel-level-targeting.md) | Pixel-level targeting: how `rect=` and `mask=` restrict an effect | Accepted | #187 |

## When to write an ADR

Write one when a change:

- adds, removes or renames a public abstraction (a trait, a crate, a module that other code builds on),
- decides between options that later work depends on (for example, an issue blocked by "decide X first"),
- weakens or reinterprets one of the [design principles](../../DEVELOPER_GUIDE.md#design-principles), or
- would make a reviewer ask "why was it done this way?" a year from now.

Bug fixes, new effects that follow the existing pattern, and refactors that keep the public API do not need one.

## How to write one

1. Copy [`template.md`](template.md) to `NNNN-short-title.md`, using the next free number.
2. Set the status to **Proposed** and open a pull request. Discussion happens on the pull request.
3. When the pull request is merged, set the status to **Accepted** (or **Rejected**, keeping the file so the reasoning is not lost).
4. Add a row to the index above.

An accepted ADR is not edited afterwards except for typos and links. To change a decision, write a new ADR, set the old one's status to **Superseded by [NNNN](NNNN-....md)**, and link back from the new one.

Living documentation, such as the full text of the design principles, belongs in [`DEVELOPER_GUIDE.md`](../../DEVELOPER_GUIDE.md); an ADR records the decision to adopt or change it.
