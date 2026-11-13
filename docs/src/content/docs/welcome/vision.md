---
title: Vision
description: The problems The Dev Browser is meant to solve, the idea behind it, and the principles it is built on.
sidebar:
  order: 2
---

:::note[Where this stands]
This page is the thinking behind the project. Some of it works today and some of it is still design; the [features](/dev_browser/welcome/features/) page says which, and the [roadmap](/dev_browser/roadmap/) says when.
:::

For a developer the browser is more than a way to read websites. It is a reference library, a debugger, a project navigator and the front door to
repositories, dashboards, cloud consoles and local servers. Yet most browsers still treat every tab as an unrelated destination and know very
little about the work the tabs belong to.

{{name}} does not try to out-render Chromium. Chromium already does that well. It tries to change how a browser is organised around the person
using it: by identity and project first, by tab second.

## What goes wrong in a conventional browser

**Tabs with no context.** Twenty or thirty tabs across a client project, a personal project, documentation and a few admin consoles. Nothing
says why a page is open or what it belongs to. Tab groups help, but a group is a label, not a boundary.

**One identity per window.** Most developers have more than one account with the same service: a personal GitHub account, a company one, an
employer's. The usual answer is a browser profile per identity, which means a window per identity, and the continuity of one working
environment is lost.

**Small jobs that break concentration.** Decoding a JWT means selecting it, copying it, opening another tab, finding a decoder, pasting,
reading the claims and finding your way back. The same goes for formatting JSON, encoding a value, generating an identifier or taking a URL
apart. Each is trivial, and together they cost more than they should.

**Search that tells on you.** History, bookmarks and autocomplete hold things you would not want on a shared screen. A suggestion from a
personal search appearing during a work presentation is exactly the kind of leak a workspace should make impossible, not just unlikely.

**An interface that does not bend.** Some people want vertical tabs and others want horizontal ones. Some compare pages side by side all day
and some want one quiet page. The browser should be a steady foundation you can shape.

## The central idea: a workspace is an environment

A workspace is not a folder of tabs. It is a complete, independently signed-in browsing environment with its own cookies and logins, history,
bookmarks, tab groups, launcher data and tools.

Take three: **Work**, **Company** and **Personal**. All three live in one window and each keeps its own state. You can be signed in to GitHub in
all three as three different people at the same moment, because the account in use is decided by the workspace a tab sits in, not by one global
session. Switching between them is instant and signs nobody out.

That one idea carries organisation (what belongs together), isolation (what must never mix) and identity (which one am I in right now) together.

### You can see which one you are in

Each workspace has a name, an icon and a colour you choose. The colour is drawn by the browser around the page, in the sidebar, on tab
indicators, in launcher results and on widget groups, so a glance tells you which login a page is using. The colour belongs to the browser's own
interface and never touches the website's content, so what is browser and what is page stays clear.

### Moving a tab changes who it is

Moving a tab to another workspace reloads it with that workspace's session. Cookies and credentials are never copied across; the page simply
becomes a page of the other identity.

## The launcher and the tools

Pressing `Ctrl+P` will open a floating launcher over the current page, like a code editor's command palette. It belongs to the browser, so a website
can neither see it nor drive it. It is meant to be the quickest route to the small, frequent actions: switching tabs, opening a bookmark or a
saved entry, running a tool, running a browser command, jumping to a workspace.

The launcher can also take what you have selected on the page. Select a token, press `Ctrl+P`, pick the decoder, read the claims and copy one.
The same works for formatting JSON, converting Base64, editing the parameters of a URL, generating identifiers or trying a regular expression
on a piece of text.

Tools are given only what you hand them. A decoder receives the selected text, not your cookies, your other tabs or your tokens. The tools come
from the [Home Screen](https://malcomber.com/projects/home-screen/) project and are moving into the browser, so there is no separate server to run.

## Search that respects boundaries

Search results can leak even when the underlying sites cannot be opened, so launcher search is scoped by default:

- With a page from one workspace in front of you, the launcher searches that workspace and nothing else.
- On the browser's home page it searches the workspace you used last, and the scope is shown on screen.
- With two workspaces side by side it searches both, and labels every result with its workspace and colour.
- You can widen the search to chosen workspaces or to all of them on purpose, but the wider scope lasts only until the launcher closes. An
  earlier "search everything" cannot quietly expose data in a later screen-share.

Matching is fuzzy and forgiving, ranked by how well it matches, how recently and how often you used it and what you are looking at. It reaches
tabs, bookmarks, history, saved entries, commands and tools.

## Two pages, side by side

Split view puts two resizable panes in one window, and each pane can hold a tab from any workspace. A Work repository can sit beside a
Personal one; documentation from one project can sit beside a local server in another. Each pane keeps its own workspace's login.

One pane has the focus, marked by a slightly stronger border in its workspace colour, and shortcuts and page-aware actions go to that pane.
Closing the split keeps the focused pane and leaves the other tab open in its own workspace. The layout code is shaped so that stacked panes and
three-pane layouts can arrive later without a rewrite.

## Widgets that stay

Some tools are worth keeping on screen. A collapsible panel on the right holds pinned tools as persistent widgets, with their own layout and
state. Each workspace has its own set, so Work might pin a token decoder and a JSON formatter while Personal pins a calculator.

In split view the panel shows both workspaces' widgets in separate, collapsible sections named and coloured for their workspace. A tool pinned
in two workspaces is two independent instances. The launcher and the panel run the same tool code, so a tool is written once and can appear as a
quick action, a larger result or a pinned widget.

## Privacy is structure, not a promise

- Each workspace has its own Chromium storage, and its own browser database (history, bookmarks, tabs, groups) and Home Screen database
  (launcher entries, widget layout, tool settings). Those databases are a tidy separation, not a security wall: a process with full file access
  could read both, so the real protection is that the application never hands one part's data to another.
- Tools and widgets are treated as less trusted than the browser. They receive no handle to a Chromium session, a database or a credential. They
  ask for a specific operation, such as reading the selected text or saving a bookmark, and the browser decides.
- Sensitive values such as tokens are not logged or saved by default.
- A list of permissions is not a sandbox. Anything that runs untrusted code has to run inside a boundary that is actually enforced.

The [privacy and security](/dev_browser/reference/privacy-and-security/) page lists the rules and what each one is checked by.

## How it is built

Rust runs the trusted application: workspaces, tabs, storage, search, permissions and the native integration. Chromium, through the Chromium
Embedded Framework, handles web standards, scripting, rendering, networking and each site's storage. The interface is a small web page served
inside the app and talking to Rust over a narrow, checked channel, so no HTTP server is involved.

The design stays proportionate. A workspace behaves like an independent service, but that is a logical separation, not an operating-system
process each; Chromium already runs its own processes. There is no microservice layer, no external database and no cloud dependency. Interfaces
and abstractions appear where they pay for themselves. See the [architecture](/dev_browser/reference/architecture/) page.

## Beyond the first release

Once the browser is dependable, the plan leaves room for horizontal or vertical tabs, richer pane layouts, themes, tear-off tabs and several
windows (all views onto the same workspaces, never copies), and eventually community-made tools and widgets. Those would use documented
interfaces and declare what they need, and run inside real boundaries; there will be no extensions in the first release.

## What success looks like

Rendering benchmarks are not the measure. The measure is a working day with fewer small frictions:

- You open the browser and land back in the workspaces and tabs you left.
- You move from your personal GitHub account to your employer's without a new window or a new login.
- Two pages from different workspaces sit side by side.
- You select a token, press `Ctrl+P`, copy a claim and carry on.
- You bookmark a page and find it in the launcher straight away.
- You search your Work history during a screen-share and nothing from Personal can appear.

## Principles

1. **Workspaces are first class.** An environment with its own identity, not a coloured group of tabs.
2. **Private by default.** Data stays in its workspace unless you choose a wider scope, and the wider scope does not stick.
3. **One browser, several identities.** More accounts should not mean more windows.
4. **Tools where the work is.** A small task should not take you off the page.
5. **One command centre.** Navigation, search, tools and browser commands share a single launcher.
6. **Customisation serves work.** Colours, layouts, widgets and navigation exist to help you see and control your environment.
7. **Use what already works.** Chromium keeps doing what it does well; Rust supplies what makes this browser different.
8. **Boundaries have to be real.** A restricted tool must not reach privileged data just because it shares a process or a window.
9. **Keep it proportionate.** No service, dependency or layer without a reason, in line with the [Floppy Rule](https://malcomber.com/how-i-work/floppy-rule/).
10. **Useful to its author first.** It is built to improve one developer's day. Wider appeal can follow from solving real problems well.

The aim is not another Chrome with a new coat of paint. It is a browser that works the way its user thinks, and feels less like a pile of tabs
and more like part of the desktop workspace.
