---
title: Privacy and security
description: The privacy rules, trust tiers and honest limits of The Dev Browser.
sidebar:
  order: 3
---

:::note[Planned design]
These are rules the implementation must meet. Each has a matching acceptance test in the project's planning notes.
:::

## Privacy rules

- Browsing sessions, bookmarks, history, Home Screen entries and widget data are isolated by workspace.
- Launcher search is limited in Rust **before** results reach the interface. Filtering only in the UI is not allowed.
- The scope a page sends is a request, not authority; the trusted side decides what is allowed.
- Widening the search is deliberate, clearly shown, and resets when the launcher closes.
- There is no global "recent" list mixing titles or URLs from different workspaces.
- Collapsed workspaces show only a name and icon.
- Sensitive tool input (JWTs, tokens) stays in memory and is not logged or saved by default.

## Trust tiers

1. **Browser core**: Chromium sessions, navigation, permissions, storage, OS integration.
2. **Capability broker**: identifies the caller and workspace, checks the request, returns minimal data.
3. **Restricted tools**: get explicit inputs and capabilities, never raw browser objects or file paths.
4. **Web pages**: no access to privileged IPC or the launcher.

## Capabilities (examples)

| Capability | Meaning |
|---|---|
| `page.selection.read` | Selected text of the focused page, on a user action only. |
| `clipboard.write` | Copy one result. Does not allow reading the clipboard. |
| `clipboard.read` | Separate permission. |
| `bookmarks.search` | Workspace-scoped, limited fields. |
| `browser.tab.open` | Open a URL in a permitted workspace. |
| `tool.storage.read` / `write` | Storage scoped to one tool instance in one workspace. |

Tools never get unrestricted cookie access, file system access, arbitrary script evaluation or database access.

## Honest limits

- Splitting `browser.db` and `homescreen.db` organises data. **It does not sandbox tools.**
- A capability manifest does not confine native code running in the browser process.
- Bundled v1 tools are audited and sit behind the broker. Running untrusted third-party tools needs a real sandbox first, and is not in v1.
- A workspace-coloured outline is an indicator, not a security boundary.
- The clipboard is shared by the whole operating system.
- Decoding a JWT does not verify its signature.
- Chromium security updates must have a delivery plan before The Dev Browser is used for important accounts.
