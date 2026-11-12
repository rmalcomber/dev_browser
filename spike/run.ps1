# Builds the spike and launches it with the CEF runtime beside the exe. Disposable.
# Needs: CMake and Ninja on PATH, CEF exported to $env:CEF_PATH (default ~/.local/share/cef). See ../planning/m0-feasibility-spike.md.
param([Parameter(ValueFromRemainingArguments = $true)]$AppArgs)
$ErrorActionPreference = 'Stop'
if (-not $env:CEF_PATH) { $env:CEF_PATH = "$env:USERPROFILE/.local/share/cef" }
Push-Location $PSScriptRoot
try {
  $ErrorActionPreference = "Continue"; cargo build; if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }; $ErrorActionPreference = "Stop"
  $out = Join-Path $PSScriptRoot 'target/debug'
  foreach ($f in 'libcef.dll','chrome_elf.dll','d3dcompiler_47.dll','dxcompiler.dll','vk_swiftshader.dll','vk_swiftshader_icd.json','vulkan-1.dll','icudtl.dat','resources.pak','chrome_100_percent.pak','chrome_200_percent.pak','v8_context_snapshot.bin') {
    Copy-Item (Join-Path $env:CEF_PATH $f) $out -Force
  }
  Copy-Item (Join-Path $env:CEF_PATH 'locales') $out -Recurse -Force
  & (Join-Path $out 'm0_spike.exe') @AppArgs
} finally { Pop-Location }
