# AGENTS.md: The Dev Browser (repo: dev_browser)

Instructions for AI coding agents (Claude Code, Codex, Cursor, etc.). Humans: this is also the contributor cheat sheet.
Keep this file short. If a rule matters, it goes here; if it is detail, it goes in `docs/`.

## What this project is
The Dev Browser is a personal, developer-oriented desktop browser written in Rust, rendering with embedded Chromium (CEF),
with the Home Screen launcher and tools built in. Its core ideas: vertical tabs grouped by **workspace** (each with its own
isolated login session and data), a browser-owned `Ctrl+P` launcher, a right-hand widget panel, and two-pane split view.
Status: the M0 feasibility spike is done and deleted (direct CEF through cef-rs, Alloy-style views; see `planning/m0-feasibility-spike.md`). M1 is under way (`planning/m1-plan.md`): the Cargo workspace, `core` storage, the CEF host (one window, trusted shell from `app://shell`, a page), IPC v0, the S6 hostile-page check and the `tools/` scripts exist. M1.2 (workspaces and storage), M1.3 (tabs, the command registry, window-level shortcuts) and M1.4 (the sidebar tree, groups, the accent frame; S8 and S11) and M1.5 (navigation chrome, search-or-address entry, favicons, open across workspaces; S10) and M1.6 (safety nets: error pages, certificate errors, popups, notices, downloads; S12 minimal) are done. M1.7 (window state, restore after a kill, a soak) is built; the hand exit test (`planning/m1-exit-test.md`), a long soak and the `0.2.0-alpha` tag are still open. Nothing is released.

Start with `planning/README.md`, then `planning/home-screen-browser-spec.md` (the source spec) and
`planning/roadmap-and-milestones.md`.

## Commands
Fill these in as soon as they exist. Agents must use these, not guess.

| Task | Command |
| --- | --- |
| Install | `cd ui && npm install` (Rust needs CMake, Ninja and a CEF export for the app build; see `tools/build.ps1`) |
| Run / dev | `tools\build.ps1 run` (Windows; builds `ui/` and the app, copies the CEF runtime, starts it; `-- --data-dir=PATH --url=URL` for a throwaway profile) |
| Test | `tools\build.ps1 test` |
| Check (fmt, clippy, tests, ui build) | `tools\build.ps1 check` |
| Build | `tools\build.ps1 build` |
| Security check S6 (hostile page; needs a build and node) | `tools\s6-hostile-page.ps1` |
| M1.7 checks: restore after a clean close and a kill, window size, logins kept separate (about 3 minutes) | `toolsm17-restore.ps1` |
| Soak: open tabs churned for `-Minutes`, process and memory bound (needs the keyboard) | `toolssoak.ps1 -Minutes 12` |
| M1.6 checks: closing risky pages (dialogs, permissions, a busy loop), popups, error pages, S12, downloads, a crashed renderer (about 3 minutes, takes over mouse and keyboard; needs node and openssl) | `tools\m16-safety.ps1` |
| M1.5 checks: address bar, back/forward/reload/stop, favicon, S10 across workspaces (about 2 minutes, takes over mouse and keyboard; typing assumes a US layout) | `tools\m15-navigation.ps1` |
| M1.4 checks: sidebar tree, accent frame, recolour (about 40 seconds, takes over mouse and keyboard) | `tools\m14-sidebar.ps1` |
| S8 no-flash check: workspace switches and tab churn recorded at 60 fps (about 1 minute; needs ffmpeg and node) | `tools\s8-switch.ps1` |
| M1.3 check: shortcuts from page, iframe and sidebar; 200 tab cycles; restore (about 6 minutes, takes over mouse and keyboard) | `tools\m13-tabs.ps1` |
| M1.2 check: two workspaces, separate logins, delete (takes over mouse and keyboard) | `toolsm12-workspaces.ps1` |
| Screenshot / 60 fps frame check of the real window (takes over mouse and keyboard) | `tools\shot.ps1`, `tools\record.ps1` |
| Docs dev | `cd docs && npm install && npm run dev` |
| Docs build (also checks links) | `cd docs && npm run build` |

## Layout
- `docs/`: Astro + Starlight documentation site. Source of truth for behaviour.
- `docs/src/content/docs/`: the pages. Folders become sidebar groups.
- `docs/ai/`: agent working notes (not published): `decisions.md`, `glossary.md`, `conventions.md`.
- `planning/`: working plans, spec, milestones, open questions, security test matrix. Not published. Settled behaviour moves to `docs/`.
- `crates/core` (domain and SQLite storage, no CEF), `crates/host` (CEF only, M1.1), `crates/app` (the binary).
- `ui/`: trusted Preact shell (Vite, vitest), served from `app://shell`. `tools/`: build, test and window-driving scripts (see `tools/README.md`).
- Sibling project `../home_screen` is the source of the launcher and tools. Read it; never edit it from here.

## Invariants (spec section 6.3; do not weaken without a recorded decision)
- Workspaces isolate browsing sessions, bookmarks, history, Home Screen entries and widget data.
- Search scope is resolved and filtered in Rust **before** results reach the UI. The client's scope is a request, not authority.
- No global recent-results list with cross-workspace titles or URLs. Broader search resets when the launcher closes.
- Sensitive tool input (JWTs, tokens) stays in memory; never logged or persisted by default.
- Never silently bypass certificate errors. Tools get explicit inputs and capabilities, never raw browser objects or paths.
- The `browser.db`/`homescreen.db` split is organisation, not a sandbox. Do not describe it as one.
- Keyboard-first (Roy, 2026-11-13): every user action is a named command with a default shortcut, routed at window level; no action is mouse-only, and focus is always visible and never trapped. See `planning/m1-plan.md`, section 8a.

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
