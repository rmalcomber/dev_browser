---
title: Install
description: How to get The Dev Browser.
sidebar:
  order: 1
---

:::caution[No download yet]
There is no installer or downloadable build. The application runs from source on Windows, as an unreleased alpha. Follow progress on the [roadmap](/dev_browser/roadmap/) or on [GitHub]({{repo}}).
:::

## Run it from source (Windows)

You need Rust, the Visual Studio 2022 C++ build tools, CMake, Ninja, Node, and a CEF 156 "minimal" Windows x64 export (see `tools/README.md` in the repository for how to make one).

```bash
git clone {{repo}}
cd dev_browser/ui && npm install && cd ..
```

```powershell
tools\build.ps1 run
```

Add `-- --data-dir=C:\temp\devb` to use a throw-away profile instead of your own data.

Release {{version}} is a planning release; see [releases]({{releases}}).
