---
title: About
description: What The Dev Browser is, who it is for and why it exists.
sidebar:
  order: 1
---

The Dev Browser is a personal desktop browser for developers. It is written in Rust, draws web pages with embedded Chromium,
and has the [Home Screen](https://github.com/rmalcomber/home_screen) launcher and tools built in.

:::caution[Planning stage]
Nothing is released yet. These pages describe the design that is being built, so they say what is *intended*. The
[roadmap](/dev_browser/roadmap/) shows where things stand.
:::

## Why it exists

Developers live in several identities at once: work, a client or company, personal projects. Browser profiles handle that
badly. They mean separate windows, and the useful tools (a JWT decoder, a JSON formatter, a quick search over everything
you have opened) live in other tabs or sites. The Dev Browser puts the identities in one window, keeps their data apart,
and puts the tools one keystroke away.

It is **not** a new rendering engine, a Chrome clone or an extension platform.

## Principles

- **Workspaces keep things apart.** Logins, history, bookmarks, launcher data and tool state belong to one workspace and stay there.
- **Local first.** No account, no sync, no telemetry.
- **Honest about security.** What is and is not a boundary is written down; see [privacy and security](/dev_browser/reference/privacy-and-security/).
- **The Floppy Rule, with one big exception.** The Dev Browser follows the [Floppy Rule](https://malcomber.com/floppy-rule/) for everything it
  writes. The Chromium runtime is large, and its size is reported rather than hidden.

## Status

Current release: [v{{version}}](/dev_browser/releases/v{{version}}/). Source and issues are on [GitHub]({{repo}}).
