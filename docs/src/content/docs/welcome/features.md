---
title: Features
description: What The Dev Browser is designed to do.
sidebar:
  order: 2
---

:::caution[Design, not shipped]
Everything here is the planned v1. Nothing is built yet; see the [roadmap](/dev_browser/roadmap/).
:::

## Workspaces

All workspaces sit in a **vertical sidebar** on the left, with their tab groups and tabs nested underneath. Each has a
name and an accent colour, drawn around the page it shows.

- Clicking a workspace name switches to it and restores its last tab. The chevron only expands or collapses it.
- Each workspace has its own login session, so you can be signed in to GitHub as two people in one window.
- A collapsed workspace shows only its name and icon: no tab counts, titles or thumbnails.

## The launcher

`Ctrl+P` opens a floating, draggable, VS Code style launcher owned by the browser, not by the page. It searches the
tabs, groups, bookmarks, history, Home Screen entries and tools of the workspaces you are working in, and nothing else.
You can widen the search deliberately; it narrows again when you close the launcher. `Ctrl+D` bookmarks the focused page.

## Tools

Home Screen tools such as the JWT decoder and JSON formatter run from the launcher. Select text on a page, press `Ctrl+P`,
search `jwt`, and decode it. Tools only get what you hand them: no scraping, no cookies, no storage. Sensitive input stays
in memory and is not saved by default. Any tool can be **pinned** to the widget panel.

## Widget panel

A collapsible, resizable panel on the right holds pinned tools, in sections per workspace and coloured to match.

## Split view

Two resizable side-by-side panes, each showing a tab from any workspace, including two different sessions at once.
Page-aware actions apply to the focused pane only. Closing the split keeps the focused pane.

## A usable browser

Navigation, tabs, downloads, uploads, popups, permission prompts, error pages and crash recovery. Certificate errors are
never silently bypassed.

## Not in v1

Third-party extensions, themes, sync, tear-off windows and arbitrary pane layouts. See the [roadmap](/dev_browser/roadmap/).
