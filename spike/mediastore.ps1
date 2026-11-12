# Media permission store check (camera and microphone, which CEF does not remember). The app keeps its own decisions per workspace
# and origin and answers from them; the "user" is simulated by --answer-permissions. Uses Chromium's fake capture devices
# (--use-fake-device-for-media-stream), so no real camera or microphone is opened. Needs node on PATH. Disposable.
param([string]$Exe = (Join-Path $PSScriptRoot 'target/debug/m0_spike.exe'), [int]$Port = 18771)
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class S2 { [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
 [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr e); }
'@
[void][S2]::SetProcessDPIAware()
function Chord($vk) { [S2]::keybd_event(0x11,0,0,[UIntPtr]::Zero); [S2]::keybd_event($vk,0,0,[UIntPtr]::Zero); [S2]::keybd_event($vk,0,2,[UIntPtr]::Zero); [S2]::keybd_event(0x11,0,2,[UIntPtr]::Zero) }
$dir = Split-Path $Exe; Push-Location $dir
$srvlog = Join-Path $dir 'mediastore-server.log'; Remove-Item $srvlog -ErrorAction SilentlyContinue
$srv = Start-Process node -ArgumentList (Join-Path $PSScriptRoot 'slow-server.js'), $Port, 3000, $srvlog -PassThru -WindowStyle Hidden
Start-Sleep 1
$script:seen = 0
function Launch($label, $kindC, $answer, $extra, $kindB) {
  $script:seen = 0; Remove-Item mediastore.out -ErrorAction SilentlyContinue
  $base = "http://127.0.0.1:$Port/permcheck"
  $b = if ($kindB) { "${base}?kind=$kindB&who=B" } else { "${base}?kind=camera&mode=query&who=B" }
  $argv = @("--url-a=${base}?kind=camera&mode=query&who=A", "--url-b=$b", "--url-c=${base}?kind=$kindC&who=C", '--exit-after=120', '--media-store', '--use-fake-device-for-media-stream', "--answer-permissions=$answer") + $extra
  $p = Start-Process $Exe -ArgumentList $argv -PassThru -RedirectStandardOutput mediastore.out -RedirectStandardError mediastore.err
  Start-Sleep 10; $p.Refresh(); $h = $p.MainWindowHandle
  [void][S2]::SetWindowPos($h, [IntPtr](-1), 0, 0, 0, 0, 0x43); [void][S2]::SetForegroundWindow($h); Start-Sleep 1
  Write-Host "--- $label"
  return @($p, $h)
}
function Show { 
  if (Test-Path $srvlog) { Get-Content $srvlog | Where-Object { $_ -match ' report ' } | ForEach-Object { '    page: ' + (($_ -split ' report ', 2)[1]) }; Clear-Content $srvlog }
  $lines = @(Get-Content mediastore.out -ErrorAction SilentlyContinue | Where-Object { $_ -match 'media (request|store)|  -> ' })
  @($lines | Select-Object -Skip $script:seen) | ForEach-Object { '    app:  ' + ($_ -replace '^\[m0\] ', '' -replace 'http://127.0.0.1:\d+', '') }
  $script:seen = $lines.Count
}
function CloseGracefully($p, $h) { $r = New-Object S2+RECT; [void][S2]::GetWindowRect($h, [ref]$r); [void][S2]::SetCursorPos($r.R - 24, $r.T + 15); Start-Sleep -Milliseconds 400; [S2]::mouse_event(2,0,0,0,[UIntPtr]::Zero); [S2]::mouse_event(4,0,0,0,[UIntPtr]::Zero); $p | Wait-Process -Timeout 15 -ErrorAction SilentlyContinue; Start-Sleep 2 }
function StoreFiles { Get-ChildItem (Join-Path $dir '.m0-data\workspaces') -Recurse -Filter media-permissions.txt -ErrorAction SilentlyContinue | ForEach-Object { "    store $($_.Directory.Name): " + ((Get-Content $_.FullName) -join ' ; ' -replace "`t", ' ') } }
Get-Process m0_spike -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue; Start-Sleep 1
Remove-Item -Recurse -Force .m0-data -ErrorAction SilentlyContinue; Set-Content $srvlog ''

$p, $h = Launch '1. first visit: C asks for the camera, the simulated user ACCEPTS; pane B (another workspace) also asks, same origin' 'camera' 'accept' @() 'camera'
Show
Chord 0x34; Start-Sleep 3.5; Show; StoreFiles
Write-Host '--- 2. C destroyed and recreated: answered from the store, no question'
Chord 0x36; Start-Sleep 1.5; Chord 0x34; Start-Sleep 3.5; Show
CloseGracefully $p $h
$p, $h = Launch '3. after a restart; the simulated user would now DENY, so an answer from the store proves the store wins' 'camera' 'deny' @() 'camera'
Show; Chord 0x34; Start-Sleep 3.5; Show
Write-Host '--- 4. C now asks for camera + microphone: camera is stored, microphone is not'
Chord 0x36; Start-Sleep 1.5; CloseGracefully $p $h
$p, $h = Launch '4. restart, C asks camera+mic, the simulated user DENIES the new (microphone) part' 'cammic' 'deny' @() 'camera'
Show; Chord 0x34; Start-Sleep 3.5; Show; StoreFiles
CloseGracefully $p $h
$p, $h = Launch '5. revoke: C store cleared at start-up, C asks for the camera again (user would deny)' 'camera' 'deny' @('--clear-media-store=ws-c') 'camera'
Show; Chord 0x34; Start-Sleep 3.5; Show; StoreFiles
Get-Process m0_spike -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Stop-Process -Id $srv.Id -Force -ErrorAction SilentlyContinue
Remove-Item mediastore.out, mediastore.err, $srvlog -ErrorAction SilentlyContinue
Pop-Location
