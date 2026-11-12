# Subset grant and live revoke check for camera and microphone (fake capture devices: no real camera or microphone is opened).
#  A. the store allows the camera and denies the microphone; the page asks for both. With --grant-subset the app grants just the camera.
#  B. a page holds a live camera stream; the app revokes the decision (deletes the store entry) 4 s in. Without a reload the stream keeps
#     running; with --revoke-reload the page is reloaded, its stream ends and the next request is asked again.
# Needs node on PATH. Takes over the keyboard briefly. Disposable.
param([string]$Exe = (Join-Path $PSScriptRoot 'target/debug/m0_spike.exe'), [int]$Port = 18776)
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class R2 { [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
 [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr e); }
'@
[void][R2]::SetProcessDPIAware()
function Chord($vk) { [R2]::keybd_event(0x11,0,0,[UIntPtr]::Zero); [R2]::keybd_event($vk,0,0,[UIntPtr]::Zero); [R2]::keybd_event($vk,0,2,[UIntPtr]::Zero); [R2]::keybd_event(0x11,0,2,[UIntPtr]::Zero) }
$dir = Split-Path $Exe; Push-Location $dir
$srvlog = Join-Path $dir 'revoke-server.log'; Remove-Item $srvlog -ErrorAction SilentlyContinue
$srv = Start-Process node -ArgumentList (Join-Path $PSScriptRoot 'slow-server.js'), $Port, 3000, $srvlog -PassThru -WindowStyle Hidden
Start-Sleep 1
$base = "http://127.0.0.1:$Port"
$script:seen = 0
function Launch($label, $urlC, $answer, $extra) {
  $script:seen = 0; Remove-Item revoke.out -ErrorAction SilentlyContinue
  $argv = @("--url-a=$base/permcheck?kind=camera&mode=query&who=A", "--url-b=$base/permcheck?kind=camera&mode=query&who=B", "--url-c=$urlC", '--exit-after=120', '--media-store', '--use-fake-device-for-media-stream', "--answer-permissions=$answer") + $extra
  $p = Start-Process $Exe -ArgumentList $argv -PassThru -RedirectStandardOutput revoke.out -RedirectStandardError revoke.err
  Start-Sleep 10; $p.Refresh(); $h = $p.MainWindowHandle
  [void][R2]::SetWindowPos($h, [IntPtr](-1), 0, 0, 0, 0, 0x43); [void][R2]::SetForegroundWindow($h); Start-Sleep 1
  Write-Host "--- $label"; return @($p, $h)
}
function Show([switch]$Page = $true) {
  if (Test-Path $srvlog) { Get-Content $srvlog | Where-Object { $_ -match ' report (C|B|A) ' -and $_ -notmatch 'state-before.*(A|B) camera' } | ForEach-Object { '    page: ' + (($_ -split ' report ', 2)[1]) }; Clear-Content $srvlog }
  $lines = @(Get-Content revoke.out -ErrorAction SilentlyContinue | Where-Object { $_ -match 'media (request|store|decisions)|  -> |reloaded' })
  @($lines | Select-Object -Skip $script:seen) | ForEach-Object { '    app:  ' + ($_ -replace '^\[m0\] ', '' -replace 'http://127.0.0.1:\d+', '') }
  $script:seen = $lines.Count
}
function CloseGracefully($p, $h) { $r = New-Object R2+RECT; [void][R2]::GetWindowRect($h, [ref]$r); [void][R2]::SetCursorPos($r.R - 24, $r.T + 15); Start-Sleep -Milliseconds 400; [R2]::mouse_event(2,0,0,0,[UIntPtr]::Zero); [R2]::mouse_event(4,0,0,0,[UIntPtr]::Zero); $p | Wait-Process -Timeout 15 -ErrorAction SilentlyContinue; Start-Sleep 2 }
Get-Process m0_spike -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue; Start-Sleep 1
Remove-Item -Recurse -Force .m0-data -ErrorAction SilentlyContinue; Set-Content $srvlog ''

# A. subset grant
$p, $h = Launch 'A1. C asks for the camera, the user accepts (camera stored as allowed)' "$base/permcheck?kind=camera&who=C" 'accept' @()
Chord 0x34; Start-Sleep 3.5; Show; CloseGracefully $p $h
$p, $h = Launch 'A2. C asks for camera + microphone; the user denies the microphone; --grant-subset: grant what is allowed' "$base/permcheck?kind=cammic&who=C" 'deny' @('--grant-subset')
Chord 0x34; Start-Sleep 3.5; Show; CloseGracefully $p $h

# B. live revoke
Remove-Item -Recurse -Force .m0-data -ErrorAction SilentlyContinue
$p, $h = Launch 'B1. live stream, camera allowed; the app revokes 4 s after C is created, NO reload' "$base/livecam?who=C" 'accept' @('--auto-revoke-ms=4000')
Chord 0x34; Start-Sleep 9; Show; CloseGracefully $p $h
Remove-Item -Recurse -Force .m0-data -ErrorAction SilentlyContinue
$p, $h = Launch 'B2. same, but the app reloads the page after revoking, and the user now DENIES' "$base/livecam?who=C" 'accept' @('--auto-revoke-ms=4000', '--revoke-reload', '--answer-after-revoke=deny')
Chord 0x34; Start-Sleep 9; Show
Get-Process m0_spike -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Stop-Process -Id $srv.Id -Force -ErrorAction SilentlyContinue
Remove-Item revoke.out, revoke.err, $srvlog -ErrorAction SilentlyContinue
Pop-Location
