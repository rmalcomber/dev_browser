# Reusing Home Screen

The spec says to "reuse Home Screen TypeScript UI concepts and components". This records what exists, so the reuse is a
decision rather than a copy-paste. Source: `projects/home_screen` (read-only from this project's point of view).

## What Home Screen is today (checked 2026-11-12)

- Rust server (`src/`: `api.rs`, `db.rs`, `server.rs`, `http_client.rs`, `net/`) with SQLite via `rusqlite`, serving a web app.
- Web app in `web/` (Vite, JSX): `web/src/core/` (app, store, layout, `launcher/`, `tiles/`, `ui/`, `lib/`) and
  `web/src/extensions/` with about 37 self-contained tools (jwt, json, base64, regex, cron, diff, hash, url, uuid, curl, ...).
- Ships as one small executable; the launcher is opened with `Ctrl+P` and doubles as the tool surface.

## Fit with The Dev Browser

| Area | Reuse idea | Caveat |
| --- | --- | --- |
| Launcher UI (`core/launcher`) | Same look and keyboard behaviour inside the browser shell. | Must talk to a typed Rust API with workspace scope, not the global store. |
| Tools (`extensions/jwt`, `json`, ...) | Pure-function tools are good candidates for the bundled set. | They must run behind the capability broker: explicit inputs only, no raw page or storage access. |
| Storage | Same SQLite approach. | Spec requires one `homescreen.db` per workspace and no shadow bookmark copies. |
| Server | None. | The Dev Browser has no localhost HTTP server; the shell talks to Rust over IPC. |

## To investigate (read-only)

1. How `core/launcher` gets its data and whether the data layer can be swapped for IPC without forking.
2. Which extensions are pure (input to output) and can run unchanged; which touch the network (`curl`, `http`, `net`) and need capabilities.
3. Whether Home Screen's UI stack and bundle size fit the Floppy Rule inside a browser shell.
4. Whether to share code by copying, by a shared package, or by keeping the two apps independent. Default: copy deliberately
   and record it; do not couple the repos until there is a clear shared package.

## What M2.3 took (2026-11-13, home_screen at `fcfc845`)

Read, not copied: `web/src/core/launcher/` (`registry.js` providers and ranking tiers, `providers.js` items for tiles, bookmark links, developer environments and GitHub repos) and `web/src/core/store.js` for the shape of a tile. Taken as ideas and rewritten in Rust and Preact in this project: result kinds with a badge, the `dev` / `stage` / `prod` environments (prod red), and ranking by match quality then use. Left for M3: tiles, pages, templates, the extension tools and their manifests. Answers to the questions above: (1) the launcher's data comes from a global store, so the data layer had to be replaced, not swapped; (2) not yet looked at per tool; (3) not measured; (4) decision: copy deliberately and record it, nothing is shared yet.

## Rules

- Do not edit `projects/home_screen` from here; changes there are their own commits in their own repo.
- Anything reused is recorded in `docs/ai/decisions.md` with the source path and commit.
