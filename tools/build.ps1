# The one entry point for building and checking The Dev Browser (Windows).
#   tools\build.ps1 check   cargo fmt --check, clippy, tests, then the ui tests and build. No CEF needed for the core checks.
#   tools\build.ps1 test    cargo test (workspace) and ui tests
#   tools\build.ps1 build   ui build, then cargo build of the app with the CEF runtime copied beside the exe (needs CMake, Ninja, CEF)
#   tools\build.ps1 run     build, then start the app
# Lessons carried over from the M0 spike (spike\run.ps1):
#  - CEF_PATH is an input of the cef-dll-sys build script: always use one spelling (forward slashes) or it rebuilds everything.
#  - winget installs CMake and Ninja but a shell started earlier may not have them on PATH.
param(
  [Parameter(Position = 0)][ValidateSet('check', 'test', 'build', 'run')][string]$Task = 'check',
  [Parameter(ValueFromRemainingArguments = $true)]$AppArgs
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent

function Run([string]$what, [scriptblock]$cmd) {
  Write-Host "==> $what"
  $prev = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
  & $cmd
  $code = $LASTEXITCODE; $ErrorActionPreference = $prev
  if ($code -ne 0) { throw "$what failed (exit $code)" }
}

function Need([string[]]$tools) {
  foreach ($d in (Join-Path $env:ProgramFiles 'CMake\bin'), (Join-Path $env:LOCALAPPDATA 'Microsoft\WinGet\Links')) {
    if ((Test-Path $d) -and (($env:PATH -split ';') -notcontains $d)) { $env:PATH += ";$d" }
  }
  foreach ($t in $tools) {
    if (-not (Get-Command $t -ErrorAction SilentlyContinue)) { throw "$t is not on PATH. Install it (CMake: winget install Kitware.CMake, Ninja: winget install Ninja-build.Ninja) and open a new shell." }
  }
}

function UiInstalled {
  if (-not (Test-Path (Join-Path $root 'ui\node_modules'))) { Run 'npm install (ui)' { Push-Location (Join-Path $root 'ui'); try { npm install } finally { Pop-Location } } }
}

Push-Location $root
try {
  switch ($Task) {
    'check' {
      # clippy compiles the host, which builds cef-dll-sys, so it needs the same inputs as a build
      Need 'cargo', 'npm', 'cmake', 'ninja'
      if (-not $env:CEF_PATH) { $env:CEF_PATH = "$env:USERPROFILE/.local/share/cef" }
      $env:CEF_PATH = $env:CEF_PATH.Replace([string][char]92, '/')
      Run 'cargo fmt --check' { cargo fmt --all -- --check }
      Run 'cargo clippy' { cargo clippy --workspace --all-targets -- -D warnings }
      Run 'cargo test' { cargo test --workspace }
      UiInstalled
      Run 'ui tests' { Push-Location ui; try { npm test } finally { Pop-Location } }
      Run 'ui build' { Push-Location ui; try { npm run build } finally { Pop-Location } }
    }
    'test' {
      Need 'cargo', 'npm'
      Run 'cargo test' { cargo test --workspace }
      UiInstalled
      Run 'ui tests' { Push-Location ui; try { npm test } finally { Pop-Location } }
    }
    { $_ -in 'build', 'run' } {
      Need 'cargo', 'npm', 'cmake', 'ninja'
      if (-not $env:CEF_PATH) { $env:CEF_PATH = "$env:USERPROFILE/.local/share/cef" }
      $env:CEF_PATH = $env:CEF_PATH.Replace([string][char]92, '/')
      if (-not (Test-Path (Join-Path $env:CEF_PATH 'libcef.dll'))) { throw "No CEF runtime in CEF_PATH ($env:CEF_PATH). See spike\README.md for how it was exported." }
      UiInstalled
      Run 'ui build' { Push-Location ui; try { npm run build } finally { Pop-Location } }
      Run 'cargo build' { cargo build -p dev_browser }
      $out = Join-Path $root 'target\debug'
      # the shell is served from the ui folder next to the exe (app://shell)
      if (Test-Path (Join-Path $out 'ui')) { Remove-Item (Join-Path $out 'ui') -Recurse -Force }
      Copy-Item (Join-Path $root 'ui\dist') (Join-Path $out 'ui') -Recurse -Force
      foreach ($f in 'libcef.dll', 'chrome_elf.dll', 'd3dcompiler_47.dll', 'dxcompiler.dll', 'vk_swiftshader.dll', 'vk_swiftshader_icd.json', 'vulkan-1.dll', 'icudtl.dat', 'resources.pak', 'chrome_100_percent.pak', 'chrome_200_percent.pak', 'v8_context_snapshot.bin') {
        Copy-Item (Join-Path $env:CEF_PATH $f) $out -Force
      }
      Copy-Item (Join-Path $env:CEF_PATH 'locales') $out -Recurse -Force
      if ($Task -eq 'run') { & (Join-Path $out 'dev_browser.exe') @AppArgs }
    }
  }
  Write-Host "OK: $Task"
} finally { Pop-Location }
