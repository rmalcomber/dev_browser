---
title: Contributing
description: How to build, test and send a change to dev_browser.
sidebar:
  order: 1
---

Contributions are welcome. The repository's `CONTRIBUTING.md` is the source of truth; this is the summary.

## Before you start

Say what the project's budget is (size, dependencies) and when to open an [issue]({{repo}}/issues) first.

## Build and test

Give the exact commands that build the project and run its gates (formatting, lint, tests).

## Pull requests

- Branch from `main` and open the pull request against `main`.
- Keep it focused: one change per pull request.
- Add or update tests for behaviour changes.
- Update the docs when behaviour changes (they live in `docs/`).

## Working on the docs

```bash
cd docs
npm install
npm run dev
npm run build     # also fails on broken internal links
```

## Releases

Maintainers cut releases: see [Releasing](/reference/releasing/).

## Funding

- [GitHub Sponsors](https://github.com/sponsors/rmalcomber)
- [Buy Me a Coffee](https://buymeacoffee.com/rmalcomber)
