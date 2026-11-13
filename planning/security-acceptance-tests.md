# Security and privacy acceptance tests

From spec sections 6.3 and 7.2. Each row becomes an automated test where possible, otherwise a manual checklist run before
any release. None can be written until the code exists; the point now is that the design is testable.

| # | Test | Milestone | Type | Status |
| --- | --- | --- | --- | --- |
| S1 | A Work-scoped launcher request cannot return Personal history, via results, autocomplete, event streams, caches or error messages. | M2 | Automated (Rust) | Planned |
| S2 | Client-supplied scope is a request, not authority: an out-of-scope workspace id is rejected or narrowed by the broker. | M2 | Automated | Planned |
| S3 | No global recent-results list contains cross-workspace titles or URLs. | M2 | Automated | Planned |
| S4 | Broader scope resets to the contextual default on launcher close, and leaves no cached broader results. | M2 | Automated | Planned |
| S5 | Tool code cannot read browser databases or Chromium session files through the exposed runtime. | M3 | Automated | Planned |
| S6 | A visited webpage cannot invoke privileged browser IPC or reach launcher internals. | M1 | Automated + manual | First check passing (2026-11-13): `toolss6-hostile-page.ps1` |
| S7 | `page.selection.read` binds to the focused pane and never collects from both panes. | M3/M4 | Automated | Planned |
| S8 | Switching workspace or pane never flashes previously visible content. | M1/M4 | Manual, screen capture | **M1 passing** (2026-11-13): `tools\s8-switch.ps1`, workspace switches and tab creation/closing recorded at 60 fps, frames in order, none of the previous one; shown to fail when the old view is not hidden. Split-view panes: M4 |
| S9 | Token inputs (JWTs etc.) are not logged or persisted by default. | M3 | Automated (log and DB scan) | Planned |
| S10 | Opening a tab across workspaces uses the destination context, never the source credentials. | M1 | Automated | **Passing** (2026-11-13): `tools\m15-navigation.ps1`, both directions with a token stored in cookie and localStorage; shown to fail when the view is built in another workspace's profile. Core side: `tabs.open_in` creates the tab in the destination's database and names the destination in `Show` |
| S11 | Collapsed workspace headers expose only name and icon: no counts, titles, URLs, thumbnails or activity. | M1 | Automated (UI test) | **Passing** (2026-11-13): `ui/src/Sidebar.test.tsx` and, at the source, `backend::tests::a_collapsed_workspaces_tabs_never_appear_in_the_reply` |
| S12 | TLS errors are never silently bypassed. | M4 | Manual | Planned |
| S13 | OS file permission and process-model limits are documented honestly in the docs. | M4 | Docs review | Planned |

## Rules for writing these

- Test the boundary where it is enforced (the Rust broker), not only the UI.
- Include negative cases: a malformed request, a stale workspace id, a deleted workspace.
- Workspace-coloured outlines are a visual indicator, not a security boundary; no test relies on them.
