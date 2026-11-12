# Home Screen Browser

**Product and technical design specification**  
**Status:** Draft v0.1, capturing agreed decisions  
**Date:** 12 November 2026  
**Scope:** V1 / MVP and post-v1 roadmap

> This document distinguishes **Agreed**, **Recommended**, and **To validate** decisions. An implementation proposal is not a settled dependency choice.

## 1. Vision

Build a personal, developer-oriented desktop browser, written primarily in Rust, using Chromium for web rendering, and integrating the existing **Home Screen** launcher and tools directly into the browser. The objective is a more efficient, local-first browsing environment rather than a new rendering engine or a feature-for-feature Chrome clone.

Core differentiators:

- **Vertical workspace-grouped tabs**, with independently authenticated sessions and a single browser window.
- **A browser-owned, floating Home Screen launcher** available over any page via `Ctrl+P`.
- **Context-aware tools** that can act on explicitly selected webpage content.
- **Workspace-scoped history, bookmarks, launcher data and tool state**, prioritising screen-sharing privacy.
- **Two resizable side-by-side pages**, including pages from different workspaces.
- **A collapsible right widget panel** with workspace-specific pinned tools.

### Non-goals for v1

A new rendering engine; general-purpose third-party extension installation; a theme marketplace; arbitrary split-pane trees; multiwindow tab dragging; sync accounts; full Chrome-extension compatibility; replicating all of Chrome's features.

## 2. Terminology and ownership

- **Workspace:** A user-named, colour-coded logical browser profile (e.g. Work, Company, Personal). It owns its browsing session, browser data, Home Screen data, tabs, groups and widget configuration.
- **Tab:** A web page or Home Screen page associated with exactly one workspace.
- **Pane:** A visible slot referencing a tab. Panes do not own cookies, history or persistent sessions.
- **Focused pane:** The pane currently receiving browser commands and page-context actions.
- **Active workspace:** The workspace associated with the focused pane, except Home uses the last active workspace for launcher search.
- **Launcher:** A browser-owned floating search/command UI, not injected into the visited webpage.
- **Tool definition:** Reusable capability-limited implementation (e.g. JWT decoder).
- **Tool instance:** An individually configured and potentially pinned use of a tool, owned by a workspace.
- **Widget panel:** The collapsible right-hand surface for pinned tool instances.

## 3. Agreed user experience

### 3.1 Workspace and tab navigation

- All workspaces appear in the **left vertical tab sidebar**, with tab groups and tabs nested underneath.
- The active workspace is expanded by default; inactive workspaces are collapsed by default.
- Clicking a **workspace name** activates that workspace and restores its last selected tab.
- Clicking the **chevron** expands/collapses without switching sessions or changing the displayed page.
- Clicking an individual tab activates its workspace and the selected tab, then collapses the other workspaces.
- Each workspace has a user-selected accent colour, applied to its sidebar identity and a browser-owned outline around the displayed page.
- Workspace switching retains independent GitHub and other website logins without separate top-level windows.
- Browser views and tool panels must not momentarily flash content from the previously active workspace during transitions.

**Privacy caveat:** Expanding an inactive workspace intentionally reveals its tab/group names. Default collapsed presentation exposes only its name and icon; no tab counts, titles, URL previews, thumbnails or recent activity are shown in the collapsed workspace header.

### 3.2 Floating launcher

- `Ctrl+P` opens the Home Screen launcher, overriding the normal browser print shortcut.
- Launcher starts near the **top-centre**, in the style of VS Code's command palette.
- User can drag it with the mouse; the position persists and is clamped to the browser window after resizing.
- `Escape` dismisses the launcher and restores focus to the previous page or control.
- Fuzzy search covers allowed workspace tabs, tab groups, bookmarks, history, Home Screen entries and available tools/actions.
- `Enter` executes the highlighted result/action; tool results can appear inside the same launcher and offer copy buttons.
- The browser, not visited webpage JavaScript, owns the launcher UI.
- `Ctrl+D` saves the focused page to the owning workspace's bookmarks; browser bookmarks are automatically discoverable in Home Screen without duplicating bookmark records.
- Provide an alternative printing command, proposed as `Ctrl+Shift+P` (confirm collision with future shortcuts during implementation).

#### Launcher default search scope

| Context | Search scope by default |
| --- | --- |
| Single page in Work/Company/Personal | That page's workspace only |
| Home page | Last active workspace |
| Split view, both panes same workspace | That workspace once |
| Split view, panes from two workspaces | Both visible workspaces |
| Explicit selected-workspace search | Only explicitly selected workspaces |
| Explicit global search | All workspaces |

- Explicitly broadened scope is clearly indicated and **resets on launcher close**.
- Search is scoped and filtered in Rust **before results reach the frontend**. No querying all workspaces and filtering only in the UI.
- Search suggestions, history and cached results must not leak data from a previously broader scope.
- Results from multiple workspaces show their workspace name and accent colour.
- **Page-aware actions target the focused pane**, not both panes, unless the user explicitly chooses otherwise.

### 3.3 Launcher tools and contextual actions

- Reuse Home Screen utilities including JWT decode, JSON formatting and copyable values.
- A tool can be invoked with selected webpage text, explicitly chosen clipboard input, or another authorised input source.
- Example: select JWT on current page → `Ctrl+P` → search `jwt` → decode selection → inspect claims → copy chosen value.
- No automatic scraping of page data, credentials, cookies, tokens or arbitrary browser storage.
- A tool must not directly invoke unrestricted Chromium APIs, native filesystem APIs or another workspace's service.
- Individual tools may offer a **Pin** action, adding a tool instance to the workspace's widget panel and revealing the panel/section.
- A tool's definition is global; its instances, settings and state are workspace-specific. Multiple instances of a tool are allowed.
- Sensitive inputs (notably JWTs) should be kept in memory and **not persisted or logged by default**. JWT decoding alone does not verify a signature.

### 3.4 Right-hand widget panel

- Collapsible and resizable sidebar to the **right** of the web page(s).
- Pinned widgets are organised into **independently collapsible workspace sections**, distinguished by workspace accent colour.
- Single-pane mode shows only its active/associated workspace's widgets.
- Split view across distinct workspaces shows **both sections**; if both panes belong to the same workspace, show its section once.
- Workspace sections follow left-to-right pane order when split.
- Widgets support pin/unpin, reorder, individual collapse, and persistence of non-sensitive configuration and layout.
- Hidden widgets retain their state; secret inputs are cleared as dictated by security policy.
- Widgets acting on page content default to the focused pane and require explicit commands to read context.

### 3.5 Split view (v1)

- Maximum **two resizable panes**, arranged side by side with a draggable divider.
- A pane can show a tab from **any workspace**, including a different session from the other pane.
- Page outlines show workspace colour; focused pane is visually stronger.
- Tab and workspace identity remain unchanged on split/unsplit operations.
- A tab can be assigned to a pane through the UI (including drag-to-pane if the Chromium/UI implementation proves reliable).
- When closing split view, **keep the currently focused pane** visible; the other tab remains open in its workspace.
- Preserve divider position, selected tabs and split configuration across restarts, subject to session-restoration safety.
- Context-aware launcher commands and `Ctrl+D` target the focused pane; launcher search defaults to the union of visible workspaces.

## 4. Browser functionality required for a usable v1

Required platform capabilities, even if implemented incrementally:

- Page loading and navigation, address bar and URL/search entry, back/forward/reload/stop.
- Multiple tabs, tab close, tab selection, tab groups and workspace management.
- Basic downloads, file selection/upload, popup and new-window requests, permission prompts and error pages.
- Persistent authentication per workspace, history, bookmarks and saved sessions.
- Browser-owned keyboard dispatch and focus handling, including launcher shortcut interception.
- Clear handling for TLS/certificate errors, website permissions and crashes; **never silently bypass certificate errors**.
- Platform-specific packaging and a Chromium/CEF security-update strategy before using the app with important accounts.

**Recommended v1 sequencing:** First prove Chromium, workspace-isolated sessions, shortcut/overlay and two panes before spending time refining visual polish.

## 5. Architecture

### 5.1 Logical components

```text
Trusted TypeScript browser shell
  - workspace/tree sidebar, panes, launcher, widget containers
             |
     Narrow typed IPC / commands / events
             |
Rust browser application API and capability broker
  - caller identity, workspace scope, input validation, permissions
             |
   +---------+----------------+-------------------+
   |                          |                   |
Workspace Manager        Search Coordinator   Global Tool Registry
   |                          |                   |
   +-- Workspace A            +-- authorised       +-- tool metadata
   |     Browser Service          workspace        +-- restricted execution
   |     Home Screen Service      searches
   |     Chromium Context
   +-- Workspace B ...
             |
CEF/Chromium browser views and per-workspace request contexts
             |
Persistent storage: global browser.db, per-workspace browser.db,
per-workspace homescreen.db, per-workspace Chromium-managed data
```

### 5.2 Service ownership

- **WorkspaceManager:** Create, delete, rename, restore, look up and transition workspaces; owns workspace lifecycle and references.
- **WorkspaceBrowserService:** Tabs, groups, history, bookmarks, downloads, browser-side search index and browser database access.
- **WorkspaceHomeScreenService:** Launcher entries, tool instance state, widgets, layouts and Home Screen database access.
- **ChromiumSession:** CEF request context and related browser integration for one workspace.
- **PaneManager:** Layout and focused-pane references; panes do not hold credentials or own sessions.
- **SearchCoordinator:** Resolves allowed scope, queries authorised services, merges and ranks results and labels provenance.
- **ActionRegistry/CapabilityBroker:** Describes tools, validates calls and supplies only permitted data/capabilities.
- **WindowManager:** Single-window v1; model ownership should not prevent adding windows later.

These are **logical Rust services/structs, not separate operating-system background services**. Begin with concrete structs and introduce traits only at real boundaries (engine adapters, test doubles, plugin runtime). Keep lazy loading of expensive tab/view resources. Search should not require starting hidden webpages.

### 5.3 Technology decisions

| Area | Direction | Status |
| --- | --- | --- |
| Core language | Rust | Agreed |
| UI | Reuse Home Screen TypeScript UI concepts/components | Agreed intention |
| Rendering | Chromium embedded, likely CEF | Agreed direction; binding/runtime to validate |
| Shell integration | Direct CEF integration vs Tauri with CEF | **To validate in prototype** |
| Data | SQLite, separated per workspace by responsibility | Agreed |
| Search | Local fuzzy search plus recency/frequency relevance | Agreed intent; scoring to tune |
| Interprocess communication | Typed API with Rust permission enforcement | Recommended |
| Extension runtime | Restricted sandbox model designed up front | Required boundary; format deferred |

**CEF implementation note:** CEF's request context settings support distinct `cache_path` directories for per-context persistent browser storage. Paths must be configured relative to the common root cache path as required by CEF. Confirm the chosen Rust bindings expose and correctly apply this behaviour. CEF also offers pre-renderer keyboard handling for shortcut interception. These are feasibility anchors, **not proof that the selected shell supports every overlay and split-view combination out of the box**.

## 6. Storage and privacy model

### 6.1 Per-workspace filesystem layout

```text
application-data/
├── browser.db                   # Only global metadata/preferences
├── chromium-root/               # Common CEF root; distinct subpaths
│   └── <workspace-id>/          # Chromium-managed cookies/storage/cache
└── workspaces/
    ├── <workspace-id>/
    │   ├── browser.db           # History/bookmarks/tabs/groups/search
    │   └── homescreen.db        # Launcher/widgets/tools/configuration
    └── <workspace-id>/
        ├── browser.db
        └── homescreen.db
```

Use stable, generated workspace IDs for filesystem keys. Do not use mutable names. The Chromium root location and exact internal directory structure depend on CEF version and platform.

### 6.2 Data ownership

| Owner | Records and state |
| --- | --- |
| Global `browser.db` | Workspace IDs, display names, colour, ordering, global shortcuts, app/window preferences and top-level layout references; no history or credential contents |
| Workspace `browser.db` | Bookmarks, visit history, tab groups, tab metadata, download metadata, workspace search index |
| Workspace `homescreen.db` | Launcher entries excluding browser-owned bookmarks, widget placement, tool instance settings, non-secret persistent state, tool search metadata |
| Workspace Chromium context | Cookies, authentication-related website state, local storage, IndexedDB, cache, service worker data and other browser-managed session state |
| Volatile memory | Transient selected text, decoded JWTs, copied secret values, temporary tool outputs and non-persisted sensitive inputs |

The launcher searches both databases through Rust services; it does **not** maintain a shadow copy of bookmarks in Home Screen storage. Where tools reference bookmarks, use stable bookmark IDs rather than duplicated content.

Each workspace database may have separate migrations. Do not assume cross-database atomic transactions; design operations to be idempotent or recoverable.

### 6.3 Privacy and search rules

- Isolation should be by workspace for browsing sessions, bookmarks, history, Home Screen entries, widget instances and their persisted data.
- **No global recent-results list** containing cross-workspace titles or URLs.
- Broader search requires deliberate user selection and must reset to the contextual default on close.
- Collapsed workspace headings contain no private tab metadata.
- Copy/paste and OS clipboard are inherently shared OS facilities; design tools to avoid automatically reading or replaying secrets.
- Workspace-coloured page outlines are a visual indicator, **not a security boundary**.

## 7. Threat model and enforceable boundaries

**Core requirement:** Tools/widgets, particularly future third-party extensions, are less trusted than the browser core. They must not have direct access to cookies, credentials, history databases, Chromium storage or unrelated widget data.

Trust tiers:

1. **Privileged browser core:** Chromium session management, navigation, permission decisions, browser storage and OS integrations.
2. **Capability broker:** Authenticates the caller, resolves workspace identity, verifies the requested operation/scope, performs minimal data extraction, and returns constrained outputs.
3. **Restricted tool execution:** Tools receive explicit authorised inputs and capabilities, never raw browser objects or filesystem paths.
4. **Untrusted webpages:** No access to privileged browser IPC or launcher internals.

### 7.1 Capabilities (examples)

- `page.selection.read`: Allowed only on the explicitly focused page through a user-initiated action.
- `clipboard.write`: Copy a specific tool result; never implies clipboard read access.
- `clipboard.read`: Separate permission/action where required.
- `bookmarks.search`: Workspace-scoped, returns limited result fields.
- `browser.tab.open`: Opens URL in a specified permitted workspace.
- `tool.storage.read/write`: Tool-instance- and workspace-scoped storage namespace.

Do not expose capabilities such as unrestricted cookie read, unrestricted local filesystem access, unconstrained JavaScript evaluation, or arbitrary database access to tools.

**Critical:** Splitting `browser.db` and `homescreen.db` only provides data organisation. **It does not sandbox tools.** Similarly, splitting Rust structs or using a capability manifest does not confine arbitrary native Rust code running in the browser process.

For v1 bundled tools, favour audited implementations behind broker APIs. If their runtime shares a JavaScript realm with privileged UI, they do **not** have strong isolation from it. Before installing genuinely untrusted third-party tools, introduce an enforceable sandbox (e.g., isolated restricted process or appropriately constrained WebAssembly environment), resource limits, strict IPC validation and a permission lifecycle. A webview/renderer only counts as a sandbox if its privilege boundary is demonstrably enforced.

### 7.2 Security acceptance tests

- Work scoped launcher request cannot return Personal history, including via autocomplete, event streams, caches or error messages.
- Tool code cannot directly read browser DBs or Chromium session files through the exposed runtime.
- A visited webpage cannot invoke privileged browser IPC.
- `page.selection.read` never silently collects content from both panes.
- Switching workspace/pane never flashes previously visible sensitive content.
- Token inputs are not logged or automatically persisted.
- Cross-workspace tab opening uses destination context, not source credentials.
- OS-level file permissions and process model limitations are documented honestly.

## 8. Search and action contracts (illustrative)

```ts
interface LauncherSearchRequest {
  query: string;
  scope: { kind: 'contextual' }
       | { kind: 'selected'; workspaceIds: string[] }
       | { kind: 'all' };
  focusedPaneId?: string;
}

interface LauncherResult {
  id: string;
  workspaceId?: string; // undefined only for safe global tools/actions
  kind: 'tab' | 'history' | 'bookmark' | 'entry' | 'action';
  title: string;
  subtitle?: string;
  score: number;
}

interface ToolManifest {
  id: string;
  version: string;
  name: string;
  keywords: string[];
  requestedCapabilities: string[];
  pinnable: boolean;
}
```

The client-provided scope is a **request**, not authority. The trusted broker must resolve allowed workspaces and reject or narrow unauthorised requests; page-context operations must bind to the actual focused pane and the initiating tool identity.

Ranking proposal: fuzzy-text quality first, then open-tab relevance, recency and visit frequency, with workspace provenance preserved. Do not allow global tool results to enumerate personal browser metadata.

## 9. Keyboard shortcuts (proposed defaults)

| Shortcut | Action |
| --- | --- |
| `Ctrl+P` | Floating launcher |
| `Escape` | Dismiss launcher / restore focus |
| `Ctrl+D` | Bookmark focused page in its workspace |
| `Ctrl+\\` | Toggle split view |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | Next / previous tab in focused workspace |
| `Ctrl+1` / `Ctrl+2` | Focus left / right pane in split mode |
| `Ctrl+Shift+P` | Print focused page, subject to shortcut collision review |

Shortcuts should be centralised and configurable in a later phase. Confirm OS-specific key behaviour and conflicts before freezing defaults.

## 10. V1 implementation milestones

### M0: Chromium feasibility spike

- Embed Chromium under Rust, navigate a page, support platform event loop.
- Test a browser-owned draggable overlay on top of a live page.
- Intercept `Ctrl+P` reliably, including when webpage has keyboard focus.
- Test **two independently authenticated GitHub sessions** in one window using separate CEF request contexts and persistent cache paths.
- Test side-by-side views and focus handling.
- **Exit criteria:** all five work on the selected runtime/platform; otherwise reconsider shell integration before expanding the codebase.

### M1: Browser and workspace foundation

- Rust workspace manager; global and workspace databases; migrations.
- Persistent Chromium context per workspace; vertical workspace/groups/tabs sidebar.
- Address bar, navigation, basic tab lifecycle, crash/error behaviour.
- Workspace colours and restore-active-tab semantics.

### M2: Home Screen launcher

- Integrate existing Home Screen launcher UI, local Rust API, `Ctrl+P`, draggable position, escape/focus restoration.
- Workspace-scoped fuzzy search of tabs, history, bookmarks, Home Screen entries and actions.
- Bookmark interception (`Ctrl+D`) and Home Screen visibility without duplication.
- Explicit selected/all-workspace overrides with reset-on-close.

### M3: Restricted tools and widget panel

- Global tool manifest/registry and workspace-owned instances.
- Capability broker for selected-text input, output copying and scoped search.
- JWT decoder and JSON formatter examples; non-sensitive state persistence.
- Resizable collapsible right panel; workspace-grouped sections and pin/unpin.
- Security tests for IPC, scope and storage.

### M4: Split view and hardening

- Two adjustable side-by-side panes, cross-workspace contexts, coloured borders and focused-pane commands.
- Combined contextual launcher search; combined grouped widgets panel.
- Focused-pane retention when split closes; state restoration.
- Downloads/permissions/new-window edge cases, packaging, sandbox checks, Chromium update procedure, regression and privacy testing.

## 11. Post-v1 roadmap (explicitly deferred)

1. Configurable **horizontal or vertical** tab orientation.
2. More flexible pane layouts, particularly **one full-height column beside two stacked rows** (1+2 layout), and potentially other grid arrangements.
3. Customisable **themes** (light/dark, colours, density and typography) while keeping workspace identity visible.
4. **Tear-off tabs** into new windows and drag/merge tabs between windows while preserving workspace session identity.
5. **Community/third-party launcher tools and widgets** with enforceable sandboxing, permissions, packaging, install/uninstall/update support and compatibility policy.

Potential later candidates, **not agreed requirements**: settings sync, dedicated private sessions, full Chrome extension compatibility, workspace backup/export, configurable keyboard shortcuts, advanced tool permissions and richer pane layouts.

## 12. Risks, open engineering questions and validation items

1. **Rust/CEF host choice:** Direct CEF binding vs Tauri CEF integration, version maturity, renderer ownership, packaging, GPU support and lifecycle hooks. Treat as a prototype decision, not a foregone conclusion.
2. **Browser-owned overlay:** Validate layering, keyboard focus, drag, IME, DPI scaling and resizing across platforms, especially with two Chromium views.
3. **CEF session persistence:** Verify distinct request context cache paths under the CEF common root; test authentication survival after application restart and no unintended cross-context sharing.
4. **Page context:** Selection extraction across frames, privileged/internal pages, iframes, context menus and permission/consent behaviour.
5. **Security updates:** Distribution packaging and rapid Chromium security patch adoption are mandatory before routine use with sensitive accounts.
6. **Tool sandbox:** Bundled tools can start with narrowly exposed APIs; untrusted installable tools require stronger isolation. Do not oversell the database split as a security barrier.
7. **Search data and privacy:** Define deletion and retention settings, FTS index ownership, ranking behaviour and leak-prevention tests.
8. **Platform target:** Choose first supported OS for M0; cross-platform release is a separate compatibility/test commitment.
9. **Accessibility:** Keyboard-first launcher, visible focus, screen readers, contrast and draggable overlay fallback.
10. **Single process vs separate workers:** Prefer single trusted Rust host plus Chromium's process model initially; isolate third-party execution separately when implemented.

## 13. Decision log

| Decision | Status |
| --- | --- |
| Rust core + Chromium rendering | Agreed direction |
| Vertical tabs grouped under workspaces for v1 | Agreed |
| Workspace name switches; chevron expands without switching | Agreed |
| Per-workspace accent colour, visible around page panes | Agreed |
| Distinct browsing/authentication sessions per workspace | Agreed |
| Collapsed inactive workspace headers reveal no tab/activity previews | Agreed |
| Floating, draggable top-centred `Ctrl+P` launcher | Agreed |
| Contextual launcher search respects workspace boundaries | Agreed |
| Home defaults search to last active workspace | Agreed |
| Split view searches the union of the two visible workspaces | Agreed |
| All/selected-workspace search is an explicit, temporary override | Agreed |
| Two resizable side-by-side panes in v1 | Agreed |
| Closing split retains focused pane | Agreed |
| Right pinned-widget panel grouped by workspace | Agreed |
| One global tool definition, independent instances per workspace | Agreed |
| Two SQLite DBs per workspace: browser and Home Screen | Agreed |
| Chromium manages its own cookie/session storage | Agreed technical requirement |
| Restricted tool permissions and no direct sensitive browser data access | Agreed security requirement |
| Third-party tools/extensions | Future, not v1 |
| Detailed CEF/Tauri choice | To validate |

## 14. References checked

- CEF request-context settings (`cache_path`, persistence and root path constraints): https://docs.rs/cef-dll-sys/latest/cef_dll_sys/struct._cef_request_context_settings_t.html
- CEF settings (`root_cache_path`): https://docs.rs/libcef-sys/latest/cef_sys/struct._cef_settings_t.html
- CEF keyboard handler (`OnPreKeyEvent`): https://cef-builds.spotifycdn.com/docs/151.3/classCefKeyboardHandler.html

These sources describe relevant CEF primitives; they do not replace a proof-of-concept test of the selected Rust bindings and windowing stack.
