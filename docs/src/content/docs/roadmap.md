---
title: Roadmap
description: What is planned for The Dev Browser.
sidebar:
  order: 1
---

An honest, short list of what is next. Move items to a release page when they ship.

| Milestone | What | Status |
|---|---|---|
| M0 | Feasibility spike: Chromium under Rust, overlay, `Ctrl+P` interception, two isolated sessions, two panes | Done on Windows: direct CEF through cef-rs |
| M1 | Workspaces, per-workspace databases and sessions, sidebar, address bar, tabs | Planned |
| M2 | Launcher: scoped search, bookmarks, `Ctrl+D`, explicit wider search | Planned |
| M3 | Restricted tools, capability broker, right-hand widget panel | Planned |
| M4 | Split view, downloads and permissions, packaging, security updates, hardening | Planned |

## After v1 (not committed)

- Horizontal or vertical tab layout, and richer pane layouts.
- Themes.
- Tear-off tabs and multiple windows.
- Third-party tools and widgets, only with an enforceable sandbox.

M0 showed the plan holds on Windows. Other platforms, input methods, display scaling and the size and memory figures are still to be checked before the first release.
