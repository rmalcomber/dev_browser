# Roadmap and milestones

Derived from spec sections 10 and 11. Sequence matters: **prove Chromium, workspace-isolated sessions, the shortcut/overlay
and two panes before polishing anything** (spec section 4). Each milestone ends with its docs updated and a build that passes.

## M0: Chromium feasibility spike (status: done on Windows, decision recorded 2026-11-12)

Detail in [m0-feasibility-spike.md](m0-feasibility-spike.md).

- [x] Embed Chromium under Rust, navigate a page, run the platform event loop.
- [x] Browser-owned draggable overlay on top of a live page.
- [x] `Ctrl+P` intercepted reliably, even when the page has keyboard focus.
- [x] Two independently authenticated sessions (separate request contexts and cache paths) in one window.
- [x] Side-by-side views with correct focus handling.

**Exit:** all five work on the chosen runtime and first platform. Otherwise reconsider the shell integration before growing
the codebase. Outcome recorded in `docs/ai/decisions.md`: direct CEF through cef-rs, Alloy-style views, Windows validated.
Left over from M0 (see the spike notes): IME, real OS-level DPI, a real two-workspace login, the first non-Windows platform, and the size and memory numbers.

## M1: Browser and workspace foundation

Plan: [m1-plan.md](m1-plan.md) (draft).

- [ ] Workspace manager; global and per-workspace SQLite with migrations.
- [ ] Persistent Chromium context per workspace.
- [ ] Vertical sidebar: workspaces, groups, tabs; chevron vs name click semantics.
- [ ] Address bar, back/forward/reload/stop, tab lifecycle, crash and error pages.
- [ ] Workspace accent colour and restore-last-tab.

**Exit:** log in to the same site as two identities in two workspaces, restart, both still signed in, nothing shared.

## M2: Home Screen launcher

- [ ] Reuse the Home Screen launcher UI behind a typed local API.
- [ ] `Ctrl+P`, draggable position (clamped on resize), `Escape` restores focus.
- [ ] Workspace-scoped fuzzy search: tabs, history, bookmarks, entries, actions. Scope filtered in Rust before reaching the UI.
- [ ] `Ctrl+D` bookmarking; bookmarks visible to the launcher without duplication.
- [ ] Explicit selected/all-workspace search that resets on close.

**Exit:** a Work-scoped query cannot return Personal data (see security tests).

## M3: Restricted tools and widget panel

- [ ] Global tool registry; workspace-owned instances.
- [ ] Capability broker: selected text in, copy out, scoped search.
- [ ] JWT decoder and JSON formatter; sensitive input kept in memory only.
- [ ] Right panel: resizable, collapsible, per-workspace sections, pin/unpin/reorder.
- [ ] Security tests for IPC, scope and storage.

## M4: Split view and hardening

- [ ] Two resizable panes, cross-workspace, coloured outlines, focused-pane commands.
- [ ] Union search and combined widget sections for split view; keep focused pane when closing split.
- [ ] Downloads, permission prompts, popups, TLS errors (never silently bypassed).
- [ ] Packaging, Chromium security-update procedure, sandbox checks, regression and privacy testing.

**Exit:** safe to use with important accounts (needs the update strategy in place).

## Post-v1 (deferred, not agreed requirements)

1. Horizontal or vertical tab orientation.
2. Richer layouts, e.g. one tall column beside two stacked rows.
3. Themes (light/dark, density, typography) with workspace identity kept visible.
4. Tear-off tabs and multiwindow, preserving workspace session identity.
5. Third-party tools and widgets, only with an enforceable sandbox and permission lifecycle.

Candidates, not agreed: settings sync, private sessions, Chrome extension compatibility, workspace backup/export,
configurable shortcuts, advanced tool permissions.
