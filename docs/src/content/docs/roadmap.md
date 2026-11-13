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
| M1 | Workspaces, per-workspace databases and sessions, sidebar, address bar, tabs, safety nets, restore | Built (unreleased alpha); the hand test on real accounts is still open |
| M2 | Launcher: scoped search, bookmarks, `Ctrl+D`, explicit wider search | Built (unreleased alpha): `Ctrl+P`, scoped search of tabs, commands, bookmarks, history and saved entries, ranking by use, `Ctrl+D`, and an explicit temporary wider search |
| M3 | Restricted tools, capability broker, right-hand widget panel | In progress: five tools in the launcher, the page selection and copy work; the widget panel is next |
| M4 | Split view, downloads and permissions, packaging, security updates, hardening | Planned |

## What works today

Built in M1, on Windows, not yet released:

- Workspaces you can create, rename, recolour and delete, each with its own login session and data, and an accent colour drawn around its pages.
- A sidebar tree of workspaces, tab groups and tabs; collapsed workspaces show only a name and an icon.
- Tabs, back, forward, reload, stop, an address bar that also searches, and page icons.
- A `Ctrl+P` launcher that searches the open tabs, bookmarks, history, saved entries and every command in the workspace you are in, with an explicit, temporary option to search wider. `Ctrl+D` bookmarks a page; entries (with dev, stage or prod badges) are saved from the launcher; history and launch counts can be cleared.
- Everything has a keyboard shortcut; see [shortcuts](/dev_browser/reference/shortcuts/).
- Opening a page in another workspace uses that workspace's own session, never the one you came from.
- The browser's own error pages for failed loads, certificate errors (never bypassed) and crashed pages; popups only when you asked for them; downloads that wait for your answer.
- Tabs, groups, the selected tab, the window size and each workspace's login come back after a restart, including after the app is killed.

- Five built-in tools (JWT decoder, JSON formatter, Base64, URL, UUID) in the launcher, started from the text selected on the page, with a copy button per value.

Not yet: editing entries, bookmark folders, a history page, tools, widget panel, split view, bookmarks, history, real permission prompts, an installer and the first tagged release. Pages that ask for the camera, the microphone, your location or notifications are denied for now, with a notice in the sidebar.

## After v1 (not committed)

- Horizontal or vertical tab layout, and richer pane layouts.
- Themes.
- Tear-off tabs and multiple windows.
- Third-party tools and widgets, only with an enforceable sandbox.

Other platforms, input methods, display scaling and the size figures are still to be checked before the first release. A 12-minute run with 14 tabs open stayed at 14 processes with memory flat (+0.4%); a multi-hour run has not been done.
