---
title: Releasing
description: How releases and the documentation site are published.
sidebar:
  order: 9
---

Releases are built by [GitHub Actions]({{repo}}/actions); nothing needs to be installed locally.

## Cutting a release

1. Bump the version everywhere it lives (the project's manifest, and `version` in `docs/src/site.config.mjs`). Set `stage` to `beta` when it is feature-complete and to `stable` at 1.0.
2. Add `docs/src/content/docs/releases/vX.Y.Z.md` (copy the previous page) and a row in `releases/index.md`. Give the new page a lower `sidebar.order` than the old ones so it lists first.
3. Commit, then tag and push:

   ```bash
   git tag vX.Y.Z
   git push origin main vX.Y.Z
   ```

4. Watch **Actions → Release**. Tags containing a `-` (`v0.2.0-alpha`) are published as pre-releases.

The tag must equal the version in the project's manifest, or the first job stops.

## What a release produces

| Output | Notes |
|---|---|
| Windows x64 | native runner |
| Linux x64 | static where possible |
| macOS Apple silicon and Intel | not signed or notarised |
| `SHA256SUMS` | checksums of every file above |

## The documentation site

The **Docs** workflow builds this site and publishes it to GitHub Pages whenever `docs/` changes on the default branch. Enable it once under **Settings → Pages → Source: GitHub Actions**. Changing `version` here also updates the release banner link, so the release page for that version must exist or the link validator fails the build.
