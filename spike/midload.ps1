# Mid-load destroy check: workspace C loads a page that takes 3 s, and its view is destroyed while that load is in flight.
# Records 60 fps with ffmpeg, sends Ctrl+4 (create C) / Ctrl+6 (destroy C), and reports the frame states, the server's view of
# the aborted request, the app's survival and the process count. Needs ffmpeg and node on PATH. Disposable.
param([string]$Exe = (Join-Path $PSScriptRoot 'target/debug/m0_spike.exe'), [int]$Port = 18765)
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class M { [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
 [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
 [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
 [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr e); }
'@
[void][M]::SetProcessDPIAware()
function Chord($vk) { [M]::keybd_event(0x11,0,0,[UIntPtr]::Zero); [M]::keybd_event($vk,0,0,[UIntPtr]::Zero); [M]::keybd_event($vk,0,2,[UIntPtr]::Zero); [M]::keybd_event(0x11,0,2,[UIntPtr]::Zero) }
$dir = Split-Path $Exe; Push-Location $dir
Remove-Item -Recurse -Force .m0-data -ErrorAction SilentlyContinue
$srvlog = Join-Path $dir 'midload-server.log'; Remove-Item $srvlog -ErrorAction SilentlyContinue
$srv = Start-Process node -ArgumentList (Join-Path $PSScriptRoot 'slow-server.js'), $Port, 3000, $srvlog -PassThru -WindowStyle Hidden
Start-Sleep 1
$red = 'data:text/html,<body%20style=background:%23d00><b>A</b>'; $blue = 'data:text/html,<body%20style=background:%2300d><b>B</b>'
$p = Start-Process $Exe -ArgumentList "--url-a=$red", "--url-b=$blue", "--url-c=http://127.0.0.1:$Port/page", '--exit-after=90' -PassThru -RedirectStandardOutput midload.out -RedirectStandardError midload.err
Start-Sleep 10; $p.Refresh(); $h = $p.MainWindowHandle
[void][M]::SetWindowPos($h, [IntPtr](-1), 0, 0, 0, 0, 0x43); [void][M]::SetForegroundWindow($h); Start-Sleep 1
$o = New-Object M+POINT; [void][M]::ClientToScreen($h, [ref]$o); $c = New-Object M+RECT; [void][M]::GetClientRect($h, [ref]$c)
$w = $c.R - ($c.R % 2); $ht = $c.B - ($c.B % 2); $raw = Join-Path $dir 'midload.raw'
$procsBefore = (Get-Process m0_spike -ErrorAction SilentlyContinue | Measure-Object).Count
$ff = Start-Process ffmpeg -ArgumentList '-y','-loglevel','error','-f','gdigrab','-framerate','60','-offset_x',$o.X,'-offset_y',$o.Y,'-video_size',"${w}x${ht}",'-i','desktop','-t','16','-vf','scale=32:18','-pix_fmt','rgb24','-f','rawvideo',$raw -PassThru -WindowStyle Hidden
Start-Sleep -Milliseconds 1500
# 1. create C (its page takes 3 s) and destroy it 700 ms later, mid-load
Chord 0x34; Start-Sleep -Milliseconds 700; Chord 0x36; Start-Sleep -Milliseconds 1500
Write-Host "after destroy mid-load: app alive = $(-not $p.HasExited)"
# 2. recreate it, let it finish loading this time (3.5 s), to prove the kept context still works
Chord 0x34; Start-Sleep -Milliseconds 3800
# 3. destroy and recreate with only 120 ms between, twice in a row
Chord 0x36; Start-Sleep -Milliseconds 400; Chord 0x34; Start-Sleep -Milliseconds 120; Chord 0x36; Start-Sleep -Milliseconds 120; Chord 0x34; Start-Sleep -Milliseconds 3800
$ff | Wait-Process -Timeout 20 -ErrorAction SilentlyContinue
Write-Host "end: app alive = $(-not $p.HasExited)"
Write-Host '--- app log'; Get-Content midload.out | Where-Object { $_ -match '^\[m0\] (mode|workspace C|recreating|browser closed|no workspace)' } | ForEach-Object { Write-Host $_ }
$e = Get-Content midload.err | Where-Object { $_ -match 'panicked|RefCell|already borrowed' } | Select-Object -First 3; if ($e) { Write-Host "STDERR: $($e -join ' | ')" }
Write-Host '--- server log (what the page server saw)'; Get-Content $srvlog | ForEach-Object { Write-Host $_ }
Write-Host "--- processes: m0_spike.exe before the swaps = $procsBefore, now = $((Get-Process m0_spike -ErrorAction SilentlyContinue | Measure-Object).Count)"
Get-Process m0_spike -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep 1
Write-Host "--- after closing the app: m0_spike.exe left = $((Get-Process m0_spike -ErrorAction SilentlyContinue | Measure-Object).Count)"
Stop-Process -Id $srv.Id -Force -ErrorAction SilentlyContinue
Remove-Item midload.out, midload.err -ErrorAction SilentlyContinue
Pop-Location
node (Join-Path $PSScriptRoot 'flash-analyze.js') $raw 32 18
Remove-Item $raw -ErrorAction SilentlyContinue
