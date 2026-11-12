# Drives the spike with real mouse/keyboard input (SendInput) and prints the [m0] log lines. Disposable.
# Run after a build + runtime copy (run.ps1 does both). Needs an unlocked interactive desktop; do not touch the mouse while it runs.
param([string]$Exe = (Join-Path $PSScriptRoot 'target/debug/m0_spike.exe'))
$ErrorActionPreference = 'Stop'
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class W {
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr e);
}
'@
[void][W]::SetProcessDPIAware()
function Key($vk, $down) { [W]::keybd_event([byte]$vk, 0, $(if ($down) { 0 } else { 2 }), [UIntPtr]::Zero) }
function Chord($vk, [switch]$Ctrl) { if ($Ctrl) { Key 0x11 $true }; Key $vk $true; Key $vk $false; if ($Ctrl) { Key 0x11 $false }; Start-Sleep -Milliseconds 700 }
function Click($x, $y) { [W]::SetCursorPos($x, $y) | Out-Null; Start-Sleep -Milliseconds 150; [W]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); [W]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 600 }
$script:seen = 0
function Flush { $l = @(Get-Content (Join-Path $dir 'drive.out') -ErrorAction SilentlyContinue | Where-Object { $_ -match '^\[m0' }); $l | Select-Object -Skip $script:seen | ForEach-Object { Add-Content $log $_ }; $script:seen = $l.Count }
function Mark($t) { Flush; Add-Content $log "--- $t" }

$dir = Split-Path $Exe; Push-Location $dir
$script:dirv = $dir
Remove-Item -Recurse -Force .m0-data -ErrorAction SilentlyContinue
$log = Join-Path $dir 'drive.log'; Set-Content $log ''
$page = 'file:///' + ((Join-Path $PSScriptRoot 'testpage.html') -replace '\\', '/')
$p = Start-Process $Exe -ArgumentList "--url-a=$page", '--url-b=https://example.com/', '--exit-after=75' -RedirectStandardOutput (Join-Path $dir 'drive.out') -RedirectStandardError (Join-Path $dir 'drive.err') -PassThru
Start-Sleep -Seconds 9
$p.Refresh(); $h = $p.MainWindowHandle
if ($h -eq [IntPtr]::Zero) { throw 'no main window handle' }
[W]::ShowWindow($h, 9) | Out-Null; Key 0x12 $true; Key 0x12 $false; [W]::SetForegroundWindow($h) | Out-Null; Start-Sleep -Seconds 1
$o = New-Object W+POINT; [void][W]::ClientToScreen($h, [ref]$o); $c = New-Object W+RECT; [void][W]::GetClientRect($h, [ref]$c)
Write-Host "client origin=($($o.X),$($o.Y)) size=$($c.R)x$($c.B)"
$cw = $c.R

Mark 'T3a: Ctrl+P with focus in the page input (pane A)'
Click ($o.X + 170) ($o.Y + 35); Chord 0x50 -Ctrl
Mark 'T2: overlay drag (header is ~15px below the overlay top)'
Click ($o.X + 600) ($o.Y + 40)   # inside the overlay header: just focus; then drag
[W]::SetCursorPos($o.X + 600, $o.Y + 40) | Out-Null; Start-Sleep -Milliseconds 200
[W]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 200
foreach ($i in 1..12) { [W]::SetCursorPos($o.X + 600 + 10 * $i, $o.Y + 40 + 5 * $i) | Out-Null; Start-Sleep -Milliseconds 40 }
[W]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 600
Mark 'T3b: Escape closes the overlay and returns focus'
Chord 0x1B
Mark 'T3c: Ctrl+P with focus in the iframe input'
Click ($o.X + 20 + 10 + 100) ($o.Y + 100 + 10 + 15); Chord 0x50 -Ctrl; Chord 0x1B
Mark 'T5a: Ctrl+2 focuses pane B, Ctrl+P from pane B, Esc returns to pane B'
Chord 0x32 -Ctrl; Chord 0x50 -Ctrl; Chord 0x1B
Mark 'T5b: click into pane A (mouse focus), then Ctrl+1'
Click ($o.X + 150) ($o.Y + 400); Chord 0x31 -Ctrl
Mark 'T5c: click into pane B'
Click ($o.X + $cw - 150) ($o.Y + 400)
Start-Sleep -Seconds 2
if (-not $p.HasExited) { $p | Wait-Process -Timeout 90 -ErrorAction SilentlyContinue }
Flush
Get-Process m0_spike -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Pop-Location
Get-Content $log
