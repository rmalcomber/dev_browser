# Planning

Working documents for The Dev Browser. They are not published on the docs site; anything that becomes settled behaviour moves
into `docs/src/content/docs/` (docs first), and non-obvious choices are recorded in `docs/ai/decisions.md`.

## Index

| File | What it is |
| --- | --- |
| [home-screen-browser-spec.md](home-screen-browser-spec.md) | Source spec, Draft v0.1 (12 Nov 2026). Left as received; changes go in the files below. |
| [roadmap-and-milestones.md](roadmap-and-milestones.md) | M0 to M4 as checklists with exit criteria, plus the post-v1 list. |
| [m0-feasibility-spike.md](m0-feasibility-spike.md) | The Chromium feasibility spike as a concrete test plan. First thing to build. |
| [open-questions.md](open-questions.md) | Risks and unknowns from spec section 12, with next steps and who decides. |
| [security-acceptance-tests.md](security-acceptance-tests.md) | Spec section 7.2 as a test matrix to automate. |
| [home-screen-reuse.md](home-screen-reuse.md) | What can be reused from the `home_screen` project, and what must change. |

## Status legend

Taken from the spec and used everywhere in this folder.

- **Agreed**: decided; change only deliberately and record why.
- **Recommended**: proposed default, not yet confirmed.
- **To validate**: needs a prototype or measurement before it is a decision.

## Conventions

- One topic per file. Link rather than copy.
- Dates are absolute (`2026-11-12`).
- Keep the privacy rules in spec section 6.3 as invariants: no feature may weaken them without an explicit decision.
- Nothing private goes in here: this repo is public.
