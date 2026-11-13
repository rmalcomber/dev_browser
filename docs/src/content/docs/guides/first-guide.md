---
title: Guides
description: Task-oriented how-tos, added as features ship.
sidebar:
  order: 1
---

Guides answer "how do I...?", one task per page. The browser is an unreleased alpha, so the first guide is written for running it from source.

## Sign in to the same site as two people

1. Press `Alt+N`, name a workspace (for example "Personal") and press Enter. It gets its own colour.
2. In your first workspace open the site and sign in as the first person.
3. Press `Alt+PageDown` (or click the other workspace) and open the same site. You are signed out: this workspace has its own session. Sign in as the second person.
4. Switch between them with `Alt+PageUp` and `Alt+PageDown`. The coloured frame around the page tells you which workspace you are in.
5. To open the page you are looking at in the other workspace, use the arrow button named after the other workspace at the bottom of the sidebar. It loads with that workspace's login, never the one you came from.

Both logins survive closing the window and restarting. If the app is killed, a login made in the last half minute before the kill may be lost.

## Planned guides, in the order the features arrive

- Find anything with the launcher, and widen its scope safely.
- Decode a JWT from a selection and pin the tool.
- Use split view across two workspaces.
