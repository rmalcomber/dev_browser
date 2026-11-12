# AGENTS.md: The Dev Browser (repo: dev_browser)

Instructions for AI coding agents (Claude Code, Codex, Cursor, etc.). Humans: this is also the contributor cheat sheet.
Keep this file short. If a rule matters, it goes here; if it is detail, it goes in `docs/`.

## What this project is
The Dev Browser is a personal, developer-oriented desktop browser written in Rust, rendering with embedded Chromium (CEF),
with the Home Screen launcher and tools built in. Its core ideas: vertical tabs grouped by **workspace** (each with its own
isolated login session and data), a browser-owned `Ctrl+P` launcher, a right-hand widget panel, and two-pane split view.
Status: planning, with the M0 feasibility spike under way in `spike/` (disposable).

Start with `planning/README.md`, then `planning/home-screen-browser-spec.md` (the source spec) and
`planning/roadmap-and-milestones.md`.

## Commands
Fill these in as soon as they exist. Agents must use these, not guess.

| Task | Command |
| --- | --- |
| Install | _TODO_ |
| Run / dev | _TODO_ |
| Test | _TODO_ |
| Build | _TODO_ |
| Docs dev | `cd docs && npm install && npm run dev` |
| Docs build (also checks links) | `cd docs && npm run build` |

## Layout
- `docs/`: Astro + Starlight documentation site. Source of truth for behaviour.
- `docs/src/content/docs/`: the pages. Folders become sidebar groups.
- `docs/ai/`: agent working notes (not published): `decisions.md`, `glossary.md`, `conventions.md`.
- `planning/`: working plans, spec, milestones, open questions, security test matrix. Not published. Settled behaviour moves to `docs/`.
- `spike/` (future, M0): disposable feasibility code. Clearly marked; not the product.
- `src/` (future): Rust application. `ui/` (future): trusted TypeScript shell (launcher, sidebar, widgets).
- Sibling project `../home_screen` is the source of the launcher and tools. Read it; never edit it from here.

## Invariants (spec section 6.3; do not weaken without a recorded decision)
- Workspaces isolate browsing sessions, bookmarks, history, Home Screen entries and widget data.
- Search scope is resolved and filtered in Rust **before** results reach the UI. The client's scope is a request, not authority.
- No global recent-results list with cross-workspace titles or URLs. Broader search resets when the launcher closes.
- Sensitive tool input (JWTs, tokens) stays in memory; never logged or persisted by default.
- Never silently bypass certificate errors. Tools get explicit inputs and capabilities, never raw browser objects or paths.
- The `browser.db`/`homescreen.db` split is organisation, not a sandbox. Do not describe it as one.

## Principles (from the manifesto, https://malcomber.com/manifesto/)
1. **The Floppy Rule.** Everything must justify its bytes. The Linux kernel fits on a 3.5" floppy (1.44 MB); this project's
   artifacts, dependencies and pages should not be heavier than they need to be. Prefer the standard library and platform
   features over new dependencies. Adding a dependency requires a one-line justification in `docs/ai/decisions.md`.
2. **Docs first.** Behaviour change = docs change in the same commit. If the docs and the code disagree, that is a bug.
3. **Small and boring.** Smallest change that works. No speculative abstractions, no frameworks for their own sake.
4. **Master the language, not the framework.** Read the core docs before reaching for a library.
5. **Make it observable.** Errors are explicit and logged with context. No silent catches.

## Working agreement for agents
- Read this file and `docs/ai/conventions.md` first. Look at existing code before writing new code; reuse before adding.
- Plan non-trivial work in a short list before editing; keep diffs focused on the task. No drive-by refactors.
- Run the Test and Build commands above before saying work is done; report failures as they are, with output.
- Record non-obvious decisions (and rejected alternatives) in `docs/ai/decisions.md`. Add new terms to `docs/ai/glossary.md`.
- Ask before: deleting data or files you did not create, adding dependencies, changing public interfaces,
  touching CI/deploy config, force-pushing, or anything outward-facing (publishing, sending, deploying).
- Never commit secrets, `.env` files, tokens, personal data. If you find some, stop and tell the user.
- Commit only when asked. Small commits, imperative subject line, explain *why* in the body.

## Definition of done
- [ ] Code works and tests pass (tests added for new behaviour and bug fixes)
- [ ] Docs updated; `cd docs && npm run build` passes
- [ ] No new dependency without a recorded reason
- [ ] Size/weight did not regress without a reason (Floppy Rule)
- [ ] Diff reviewed by the author for stray debug code and unrelated changes
