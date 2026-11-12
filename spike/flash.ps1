# Workspace-switch flash check. Pane A is a solid red page and pane B a solid blue one, so any frame that is not the expected
# colour layout stands out. The script records the client area at 60 fps with ffmpeg (gdigrab) while sending Ctrl+3 three times
# (split -> only A -> only B -> split), then flash-analyze.js prints the sequence of states. Needs ffmpeg and node on PATH. Disposable.
param([string]$Exe = (Join-Path $PSScriptRoot 'target/debug/m0_spike.exe'), [int]$Presses = 3, [int]$GapMs = 1800, [switch]$Fresh, [string]$AppFlags = '', [string]$Raw = '')
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class V { [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
 [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
 [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
 [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr e); }
'@
[void][V]::SetProcessDPIAware()
$Seconds = if ($Fresh) { 13 } else { [int][math]::Ceiling(2 + $Presses * $GapMs / 1000.0 + 1.5) }
$dir = Split-Path $Exe; Push-Location $dir
if (-not $Raw) { $Raw = Join-Path $dir 'flash.raw' }
Remove-Item -Recurse -Force .m0-data -ErrorAction SilentlyContinue
$red = 'data:text/html,<body%20style=background:%23d00;color:%23fff;font:200px%20sans-serif><b>A</b>'
$blue = 'data:text/html,<body%20style=background:%2300d;color:%23fff;font:200px%20sans-serif><b>B</b>'
$p = Start-Process $Exe -ArgumentList (@("--url-a=$red", "--url-b=$blue", '--exit-after=90') + $(if ($AppFlags) { $AppFlags -split ' ' } else { @() })) -PassThru -RedirectStandardOutput flash.out -RedirectStandardError flash.err
Start-Sleep 10; $p.Refresh(); $h = $p.MainWindowHandle
[void][V]::SetWindowPos($h, [IntPtr](-1), 0, 0, 0, 0, 0x43); [void][V]::SetForegroundWindow($h); Start-Sleep 1
$o = New-Object V+POINT; [void][V]::ClientToScreen($h, [ref]$o); $c = New-Object V+RECT; [void][V]::GetClientRect($h, [ref]$c)
$w = $c.R - ($c.R % 2); $ht = $c.B - ($c.B % 2)
Write-Host "recording client area $w x $ht at ($($o.X),$($o.Y)) for $Seconds s"
$ff = Start-Process ffmpeg -ArgumentList '-y', '-loglevel', 'error', '-f', 'gdigrab', '-framerate', '60', '-offset_x', $o.X, '-offset_y', $o.Y, '-video_size', "${w}x${ht}", '-i', 'desktop', '-t', $Seconds, '-vf', 'scale=32:18', '-pix_fmt', 'rgb24', '-f', 'rawvideo', $Raw -PassThru -WindowStyle Hidden
function Ctrl4 { [V]::keybd_event(0x11,0,0,[UIntPtr]::Zero); [V]::keybd_event(0x34,0,0,[UIntPtr]::Zero); [V]::keybd_event(0x34,0,2,[UIntPtr]::Zero); [V]::keybd_event(0x11,0,2,[UIntPtr]::Zero) }
function Ctrl3 { [V]::keybd_event(0x11,0,0,[UIntPtr]::Zero); [V]::keybd_event(0x33,0,0,[UIntPtr]::Zero); [V]::keybd_event(0x33,0,2,[UIntPtr]::Zero); [V]::keybd_event(0x11,0,2,[UIntPtr]::Zero) }
Start-Sleep -Milliseconds 1500
if ($Fresh) {
  # split -> brand-new workspace C (view created on the spot) -> back to split -> workspace C again (view already exists)
  Ctrl4; Start-Sleep -Milliseconds 3000; Ctrl3; Start-Sleep -Milliseconds 2000; Ctrl4; Start-Sleep -Milliseconds 2500
} else {
foreach ($i in 1..$Presses) { Ctrl3; Start-Sleep -Milliseconds $GapMs }
}   # split -> only A -> only B -> split -> ...
$ff | Wait-Process -Timeout ($Seconds + 10) -ErrorAction SilentlyContinue
Get-Content flash.out | Where-Object { $_ -match '^\[m0\] mode|relayout|workspace C' } | ForEach-Object { Write-Host $_ }
Get-Process m0_spike -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Remove-Item flash.out, flash.err -ErrorAction SilentlyContinue
Pop-Location
node (Join-Path $PSScriptRoot 'flash-analyze.js') $Raw 32 18
Remove-Item $Raw -ErrorAction SilentlyContinue
