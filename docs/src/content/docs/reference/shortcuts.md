---
title: Keyboard shortcuts
description: The shortcuts that exist today, how they are routed, and the ones promised for later.
sidebar:
  order: 4
---

:::note[Alpha]
The defaults are fixed for now; they become configurable later. Every shortcut below is checked against the application's command list
by an automated test, so this page cannot drift from what the app does.
:::

The Dev Browser is keyboard-first: every action is a named command with a default shortcut, and no action works only with the mouse.

## Available now

| Shortcut | Command |
|---|---|
| `Ctrl+T` | New tab, in the current workspace |
| `Ctrl+W` | Close the tab (closing the last one opens a fresh tab) |
| `Ctrl+Tab` | Next tab |
| `Ctrl+Shift+Tab` | Previous tab |
| `Alt+1` … `Alt+8` | Go to tab 1 to 8 (`Alt+1`, `Alt+2`, `Alt+3`, `Alt+4`, `Alt+5`, `Alt+6`, `Alt+7`, `Alt+8`) |
| `Alt+9` | Go to the last tab |
| `Ctrl+Shift+PageDown` | Move the tab down the list |
| `Ctrl+Shift+PageUp` | Move the tab up the list |
| `Ctrl+Shift+D` | Unload background tabs (their pages are closed to free memory; they reload when you select them) |
| `Ctrl+Shift+G` | Put the tab in a new group (rename it in the sidebar) |
| `Ctrl+Shift+U` | Take the tab out of its group (a group with no tabs disappears) |
| `Ctrl+L` | Go to the address bar (type an address or a search) |
| `Alt+Left` | Back |
| `Alt+Right` | Forward |
| `F5` or `Ctrl+R` | Reload |
| `Ctrl+Shift+R` | Reload, ignoring the cache |
| `Ctrl+.` | Stop loading |
| `Ctrl+Shift+S` | Save the download that is waiting for an answer |
| `Ctrl+Shift+X` | Cancel the download that is waiting for an answer |
| `Ctrl+P` | Open the launcher (search tabs, bookmarks, history and commands; `Esc` closes it) |
| `Ctrl+D` | Bookmark this page in its workspace, or remove its bookmark |
| `Alt+N` | New workspace (opens the name field in the sidebar) |
| `Alt+PageDown` | Next workspace |
| `Alt+PageUp` | Previous workspace |
| `F6` | Move focus between the sidebar and the page |
| `Esc` | While the focus is in the sidebar: return to the page |

In the sidebar, `Tab` and `Shift+Tab` move between controls, `Up` and `Down` move between the workspaces and tabs, `Home` and `End` jump to
the first and last, `Right` expands and `Left` collapses the focused workspace or group (without switching to it), and `Enter` activates the
focused control.

## How shortcuts are routed

Shortcuts are handled by the browser itself, before a page sees the key, and they work wherever the focus is: in a page, inside a frame
of a page, in a text field, or in the sidebar. The consequence is that **a page never receives these keys**: they are reserved by the
browser, as `Ctrl+T` and `Ctrl+W` are in every browser. `Esc` is the one exception: it is only taken while the focus is in the sidebar,
so pages keep their own use of it.

## Promised for later

| Shortcut | Planned command | When |
|---|---|---|
| `Ctrl+\` | Toggle split view | M4 |
| `Ctrl+1` / `Ctrl+2` | Focus the left / right pane in split view | M4 |
| `Ctrl+Shift+P` | Print the focused page | later |

Those keys are deliberately left unused so that nothing has to change when they arrive.
