---
title: Features
description: What The Dev Browser is designed to do.
sidebar:
  order: 2
---

:::caution[Partly built]
Workspaces, the sidebar, tabs, navigation and the safety nets work today in an unreleased alpha (Windows). The launcher, tools, widget panel and split view are designed but not built. Each section says which it is. See the [roadmap](/dev_browser/roadmap/).
:::

## Workspaces (works today)

All workspaces sit in a **vertical sidebar** on the left, with their tab groups and tabs nested underneath. Each has a
name and an accent colour, drawn around the page it shows.

- Clicking a workspace name switches to it and restores its last tab. The chevron only expands or collapses it. Clicking a tab of an expanded workspace switches to that workspace and tab.
- Each workspace has its own login session, so you can be signed in to GitHub as two people in one window, at the same time, with nothing shared.
- Each workspace has a name, an icon and an accent colour. The colour frames its pages, so you always know which identity a page is using. It belongs to the browser, never to the website.
- A collapsed workspace shows only its name and icon: no tab counts, titles or thumbnails. The titles of a collapsed workspace never even reach the sidebar.
- Tabs can be grouped inside a workspace, reordered, and opened in another workspace. Opening a page in another workspace loads it with that workspace's own login; cookies and credentials are never carried across.
- Workspaces, tabs, groups, the selected tab and the window size come back after a restart.

## The launcher (planned)

`Ctrl+P` opens a floating, draggable, VS Code style launcher owned by the browser, not by the page. It searches the
tabs, groups, bookmarks, history, Home Screen entries and tools of the workspaces you are working in, and nothing else.
You can widen the search deliberately; it narrows again when you close the launcher. `Ctrl+D` bookmarks the focused page.

## Tools (planned)

Home Screen tools such as the JWT decoder and JSON formatter run from the launcher. Select text on a page, press `Ctrl+P`,
search `jwt`, and decode it. Tools only get what you hand them: no scraping, no cookies, no storage. Sensitive input stays
in memory and is not saved by default. Any tool can be **pinned** to the widget panel.

## Widget panel (planned)

A collapsible, resizable panel on the right holds pinned tools, in sections per workspace and coloured to match.

## Bookmarks (planned)

`Ctrl+D` bookmarks the focused page in its own workspace. There is one bookmark store per workspace, and the launcher reads it, so there are never two bookmark systems to keep in step. Bookmarks stay in their workspace and appear in a wider search only when you ask for that workspace or split view puts it in scope.

## Split view (planned)

Two resizable side-by-side panes, each showing a tab from any workspace, including two different sessions at once, each with its own login.
The focused pane has a stronger border in its workspace colour; shortcuts and page-aware actions apply to it only. Closing the split keeps the focused pane and leaves the other tab open in its own workspace. The layout is built so stacked and three-pane layouts can come later.

## A usable browser (mostly works today)

Works today: navigation, an address bar that also searches, tabs and groups, popups only when you asked for them, downloads that wait for your answer, error pages and crash recovery. Certificate errors are never silently bypassed. Pages that ask for the camera, location or notifications are denied for now, with a notice; real permission prompts come later. File uploads have not been checked yet.

## Keyboard first (works today)

Every action is a named command with a default shortcut, handled at window level so it works whether the focus is in a page, a frame or the sidebar, and no page ever sees it. See [shortcuts](/dev_browser/reference/shortcuts/).

## What the first release is meant to include

A reliable, everyday browser rather than a copy of everything in Chrome: Chromium browsing in a Rust desktop app, independently signed-in workspaces, vertical tabs and groups under workspaces, workspace colours, two resizable panes, the floating launcher with workspace-aware fuzzy search, workspace bookmarks and history, a set of built-in tools with controlled page access, a collapsible widget panel per workspace, separate browser and Home Screen storage, enforced separation between the browser and restricted tools, and persistence of all of it. The thinking behind these is on the [vision](/dev_browser/welcome/vision/) page.

## Not in v1

Third-party extensions, themes, sync, tear-off windows and arbitrary pane layouts. See the [roadmap](/dev_browser/roadmap/).
