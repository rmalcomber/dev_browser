# M2 plan: the launcher

Source: `planning/home-screen-browser-spec.md` sections 3.2, 8 and 10, and the M2 entry in `roadmap-and-milestones.md`.
**Exit (from the roadmap):** a Work-scoped query cannot return Personal data. S1 to S4 in `security-acceptance-tests.md` are this milestone's tests.

## Design decisions

| # | Question | Decision | Why |
| --- | --- | --- | --- |
| L1 | Where does the launcher live? | A **second trusted view** (`app://shell/launcher.html`, role `Launcher`) added with `add_overlay_view` above the pages, hidden until `Ctrl+P`. | M0 proved a browser-owned overlay above live pages. The sidebar view is only 220 px wide, so it cannot draw over the page. A page can never see or touch the overlay. |
| L2 | Who holds the scope? | **Core.** The launcher sends `launcher.scope {kind, workspaces}` as a *request*; core resolves it (unknown ids dropped, nothing outside the existing workspaces) and holds the result. A search sends only the query. | The spec says the client's scope is a request, not authority, and a search cannot name a workspace by accident. Closing the launcher resets it in core (S4). |
| L3 | What is searched in M2.1? | Open tabs of the workspaces in scope, and commands. History, bookmarks and Home Screen entries arrive in M2.2 and M2.3. | Tabs and commands exist already, so the whole path (overlay, scope, ranking, run) can be proven before new storage is added. |
| L4 | Result ids | Opaque text (`tab/<workspace>/<tab>`, `cmd/<id>`). `launcher.run` re-resolves the scope and refuses an id outside it (S2). | The shell never gets to act on a workspace that was not in the results it was given. |
| L5 | Empty query | Open tabs of the scope in tab order, then commands. No recent-items list is kept anywhere (S3). | A recent list is exactly the kind of cache that leaks across scopes. |
| L6 | Moving the launcher | The launcher page sends `launcher.drag {dx, dy}`; the host clamps to the window and saves the position when the launcher closes. Clamped again when the window is resized. | Matches the spec; the page cannot place the overlay outside the window. |
| L7 | After closing | Focus goes back to the page. | "Restore the previous control" is refined later if it proves wrong for the address bar. |

## Slices

- **M2.1** Overlay, `Ctrl+P`, `Escape`, drag, scoped fuzzy search of tabs and commands, explicit scope (this workspace, chosen workspaces, all) that resets on close. Tests S1 (tabs part), S2, S3, S4. Real-window check.
- **M2.2** Per-workspace history and bookmarks (schema, `Ctrl+D`, search over them). S1 in full.
- **M2.3** Home Screen entries and the reuse of Home Screen UI pieces (`home-screen-reuse.md`); ranking by recency and frequency.

**M2.1 status: done 2026-11-13** (see the M2.1 entry in `docs/ai/decisions.md`). `tools\m21-launcher.ps1` passes on the real window; S2, S3, S4 pass as Rust tests, S1 for tabs and commands.

**M2.2 status: done 2026-11-13** (see the M2.2 entry in `docs/ai/decisions.md`). `tools\m22-places.ps1` passes on the real window; S1 is complete.

**M2.3 status: done 2026-11-13** (see the M2.3 entry in `docs/ai/decisions.md`). `tools\m23-entries.ps1` passes on the real window. **M2 is done**, apart from the exit test below.

**M2 exit ("a Work-scoped query cannot return Personal data"):** S1 to S4 pass as Rust tests and on the real window (`m21-launcher.ps1`, `m22-places.ps1`), now also covering entries and launch counts.

## Not in M2

Tools and widgets (M3), split-view scope rules (M4: the contextual scope is one workspace until there are two panes), rebinding shortcuts.
