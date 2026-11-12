# Glossary

Project-specific terms, so humans and agents use the same words. Fuller definitions are in spec section 2.

| Term | Meaning |
| --- | --- |
| Workspace | A named, colour-coded logical browser profile (Work, Personal). Owns its session, data, tabs, groups and widgets. |
| Tab | A web page or Home Screen page belonging to exactly one workspace. |
| Pane | A visible slot that references a tab. Owns no cookies, history or session. |
| Focused pane | The pane receiving browser commands and page-context actions. |
| Active workspace | The workspace of the focused pane; on the Home page, the last active workspace. |
| Launcher | The browser-owned floating search and command UI (`Ctrl+P`). Never injected into a web page. |
| Tool definition | A global, reusable, capability-limited tool (for example the JWT decoder). |
| Tool instance | A configured, possibly pinned use of a tool, owned by one workspace. |
| Widget panel | The collapsible right-hand panel of pinned tool instances, grouped by workspace. |
| Capability broker | Rust layer that authenticates the caller, resolves workspace scope and hands tools only permitted data. |
| Scope | The set of workspaces a launcher query may search. Contextual by default; broadening is explicit and temporary. |
| CEF | Chromium Embedded Framework, the way Chromium is embedded in the host. |
| Request context | CEF's per-session container for cookies and storage; one per workspace. |
| M0 spike | The disposable feasibility prototype that decides the shell integration. |
