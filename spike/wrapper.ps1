# Render-process getUserMedia wrapper check (fake capture devices: no real camera or microphone is opened). Four scenarios: a control with
# the wrapper installed and no revoke; revoke + stop-capture (all) then a retry that the user denies; revoke only the microphone while the
# camera stays live; and a stream held by a same-origin iframe. Needs node on PATH. Takes over the keyboard briefly. Disposable.
param([string]$Exe = (Join-Path $PSScriptRoot 'target/debug/m0_spike.exe'), [int]$Port = 18777, [string]$Only = '')
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class R3 { [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
 [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr e); }
'@
[void][R3]::SetProcessDPIAware()
function Chord($vk) { [R3]::keybd_event(0x11,0,0,[UIntPtr]::Zero); [R3]::keybd_event($vk,0,0,[UIntPtr]::Zero); [R3]::keybd_event($vk,0,2,[UIntPtr]::Zero); [R3]::keybd_event(0x11,0,2,[UIntPtr]::Zero) }
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
  [void][R3]::SetWindowPos($h, [IntPtr](-1), 0, 0, 0, 0, 0x43); [void][R3]::SetForegroundWindow($h); Start-Sleep 1
  Write-Host "--- $label"; return @($p, $h)
}
function Show([switch]$Page = $true) {
  if (Test-Path $srvlog) { Get-Content $srvlog | Where-Object { $_ -match ' report (C[a-z]*|B|A) ' -and $_ -notmatch 'state-before.*(A|B) camera' } | ForEach-Object { '    page: ' + (($_ -split ' report ', 2)[1]) }; Clear-Content $srvlog }
  $lines = @(Get-Content revoke.out -ErrorAction SilentlyContinue | Where-Object { $_ -match 'media (request|store|decisions)|  -> |reloaded' })
  @($lines | Select-Object -Skip $script:seen) | ForEach-Object { '    app:  ' + ($_ -replace '^\[m0\] ', '' -replace 'http://127.0.0.1:\d+', '') }
  $script:seen = $lines.Count
}
function CloseGracefully($p, $h) { $r = New-Object R3+RECT; [void][R3]::GetWindowRect($h, [ref]$r); [void][R3]::SetCursorPos($r.R - 24, $r.T + 15); Start-Sleep -Milliseconds 400; [R3]::mouse_event(2,0,0,0,[UIntPtr]::Zero); [R3]::mouse_event(4,0,0,0,[UIntPtr]::Zero); $p | Wait-Process -Timeout 15 -ErrorAction SilentlyContinue; Start-Sleep 2 }
Get-Process m0_spike -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue; Start-Sleep 1
Remove-Item -Recurse -Force .m0-data -ErrorAction SilentlyContinue; Set-Content $srvlog ''

function Scenario($label, $url, $flags) {
  if ($Only -and $label -notmatch "^($Only).") { return }
  Remove-Item -Recurse -Force .m0-data -ErrorAction SilentlyContinue
  $p, $h = Launch $label $url 'accept' $flags
  Chord 0x34; Start-Sleep 11; Show; CloseGracefully $p $h
}
Scenario 'W1. control: wrapper installed, camera stream live, NO revoke (the wrapper must not break getUserMedia)' "$base/livecam?who=C" @()
Scenario 'W2. camera stream; revoke + stop-capture all at 4 s; the user now denies, so the page cannot get a stream again' "$base/livecam?who=C" @('--auto-revoke-ms=4000', '--revoke-stop=all', '--answer-after-revoke=deny')
Scenario 'W3. camera + microphone stream; revoke only the MICROPHONE at 4 s (camera stays allowed)' "$base/livecam?who=C&kinds=both" @('--auto-revoke-ms=4000', '--revoke-stop=audio', '--answer-after-revoke=deny')
Scenario 'W4. stream held by a same-origin IFRAME; revoke + stop-capture all at 4 s' "$base/liveframe" @('--auto-revoke-ms=4000', '--revoke-stop=all', '--answer-after-revoke=deny')
Scenario 'W5. a page that calls MediaDevices.prototype.getUserMedia to dodge the wrapper; the INSTANCE-ONLY wrapper (--wrap-instance-only)' "$base/livecam?who=C&evade=1" @('--auto-revoke-ms=4000', '--revoke-stop=all', '--answer-after-revoke=deny', '--wrap=instance')
Scenario 'W6. the same dodging page against the PROTOTYPE wrapper (the default now)' "$base/livecam?who=C&evade=1" @('--auto-revoke-ms=4000', '--revoke-stop=all', '--answer-after-revoke=deny')
Scenario 'W7. a page that CLONES the stream and uses only the clone; wrapper variant proto (no clone tracking)' "$base/livecam?who=C&clone=1" @('--auto-revoke-ms=4000', '--revoke-stop=all', '--answer-after-revoke=deny', '--wrap=proto')
Scenario 'W8. the same cloning page against the full wrapper (the default)' "$base/livecam?who=C&clone=1" @('--auto-revoke-ms=4000', '--revoke-stop=all', '--answer-after-revoke=deny')
Scenario 'W9. a hostile page replaces MediaStreamTrack.prototype.stop with a no-op; the full wrapper (the default)' "$base/livecam?who=C&tamper=1" @('--auto-revoke-ms=4000', '--revoke-stop=all', '--answer-after-revoke=deny')
Get-Process m0_spike -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Stop-Process -Id $srv.Id -Force -ErrorAction SilentlyContinue
Remove-Item revoke.out, revoke.err, $srvlog -ErrorAction SilentlyContinue
Pop-Location
