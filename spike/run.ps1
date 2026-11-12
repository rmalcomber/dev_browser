# Builds the spike and launches it with the CEF runtime beside the exe. Disposable.
# Needs: CMake and Ninja, CEF exported to $env:CEF_PATH (default ~/.local/share/cef). See README.md.
param([Parameter(ValueFromRemainingArguments = $true)]$AppArgs)
$ErrorActionPreference = 'Stop'
# CEF_PATH is an input of the cef-dll-sys build script: a different spelling of the same folder forces a full
# rebuild, so always use one spelling (forward slashes).
if (-not $env:CEF_PATH) { $env:CEF_PATH = "$env:USERPROFILE/.local/share/cef" }
$env:CEF_PATH = $env:CEF_PATH.Replace([string][char]92, '/')
# winget installs CMake and Ninja but a shell started before that may not have them on PATH yet.
foreach ($d in (Join-Path $env:ProgramFiles 'CMake\bin'), (Join-Path $env:LOCALAPPDATA 'Microsoft\WinGet\Links')) {
  if ((Test-Path $d) -and (($env:PATH -split ';') -notcontains $d)) { $env:PATH += ";$d" }
}
foreach ($t in 'cmake', 'ninja', 'cargo') {
  if (-not (Get-Command $t -ErrorAction SilentlyContinue)) { throw "$t is not on PATH (needed to build cef-dll-sys). Install it, e.g. winget install Kitware.CMake / Ninja-build.Ninja, then open a new shell." }
}
if (-not (Test-Path (Join-Path $env:CEF_PATH 'libcef.dll'))) { throw "No CEF runtime in CEF_PATH ($env:CEF_PATH). See README.md." }
Push-Location $PSScriptRoot
try {
  $ErrorActionPreference = 'Continue'; cargo build; if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }; $ErrorActionPreference = 'Stop'
  $out = Join-Path $PSScriptRoot 'target/debug'
  foreach ($f in 'libcef.dll','chrome_elf.dll','d3dcompiler_47.dll','dxcompiler.dll','vk_swiftshader.dll','vk_swiftshader_icd.json','vulkan-1.dll','icudtl.dat','resources.pak','chrome_100_percent.pak','chrome_200_percent.pak','v8_context_snapshot.bin') {
    Copy-Item (Join-Path $env:CEF_PATH $f) $out -Force
  }
  Copy-Item (Join-Path $env:CEF_PATH 'locales') $out -Recurse -Force
  & (Join-Path $out 'm0_spike.exe') @AppArgs
} finally { Pop-Location }
