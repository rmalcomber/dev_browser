# Render-process getUserMedia wrapper check (fake capture devices: no real camera or microphone is opened). Four scenarios: a control with
# the wrapper installed and no revoke; revoke + stop-capture (all) then a retry that the user denies; revoke only the microphone while the
# camera stays live; and a stream held by a same-origin iframe. Needs node on PATH. Takes over the keyboard briefly. Disposable.
param([string]$Exe = (Join-Path $PSScriptRoot 'target/debug/m0_spike.exe'), [int]$Port = 18777, [string]$Only = '', [string]$Sweep = '')
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
  if (Test-Path $srvlog) { Get-Content $srvlog | Where-Object { $_ -match ' report (C[a-z0-9]*|N[a-z0-9]*|B|A) ' -and $_ -notmatch 'state-before.*(A|B) camera' } | ForEach-Object { '    page: ' + (($_ -split ' report ', 2)[1]) }; Clear-Content $srvlog }
  $lines = @(Get-Content revoke.out -ErrorAction SilentlyContinue | Where-Object { $_ -match 'media (request|store|decisions)|  -> |reloaded|stop-capture' })
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
Scenario 'X1. stream in a cross-ORIGIN iframe (same site, other port); stop sent to the main frame only (the previous behaviour)' "$base/liveframe-s" @('--auto-revoke-ms=4000', '--revoke-stop=all', '--answer-after-revoke=deny', '--stop-main-only')
Scenario 'X2. the same page; stop sent to EVERY frame (frame handler list)' "$base/liveframe-s" @('--auto-revoke-ms=4000', '--revoke-stop=all', '--answer-after-revoke=deny')
Scenario 'X3. stream in a cross-SITE iframe (localhost, probably its own renderer process); stop sent to the main frame only' "$base/liveframe-x" @('--auto-revoke-ms=4000', '--revoke-stop=all', '--answer-after-revoke=deny', '--stop-main-only')
Scenario 'X4. the same cross-site page; stop sent to EVERY frame' "$base/liveframe-x" @('--auto-revoke-ms=4000', '--revoke-stop=all', '--answer-after-revoke=deny')
Scenario 'N1. nested: top page -> iframe on localhost -> iframe on port+1 holds the stream; stop sent to every frame' "$base/liveframe-nested" @('--auto-revoke-ms=4000', '--revoke-stop=all', '--answer-after-revoke=deny')
Scenario 'N2. frames added late: one at the start, one on another site at 1.5 s, one created then removed again before the stop' "$base/liveframes-late" @('--auto-revoke-ms=4000', '--revoke-stop=all', '--answer-after-revoke=deny')
Scenario 'N3. a srcdoc iframe and a sandboxed iframe (opaque origin) each try to hold a stream' "$base/liveframes-odd" @('--auto-revoke-ms=4000', '--revoke-stop=all', '--answer-after-revoke=deny')
Scenario 'N4. a cross-site iframe streams, then navigates to another origin at 2.5 s (the new page streams too); stop at 4 s' "$base/nav-iframe" @('--auto-revoke-ms=4000', '--revoke-stop=all', '--answer-after-revoke=deny')
Scenario 'N5. the MAIN frame streams, then navigates to another site at 2.5 s; stop at 4 s' "$base/livecam?who=Nm&navat=2500&navto=http%3A%2F%2Flocalhost%3A$Port%2Flivecam%3Fwho%3DNm2" @('--auto-revoke-ms=4000', '--revoke-stop=all', '--answer-after-revoke=deny')
Scenario 'N6. the stop races a navigation: the main frame navigates at 3.9 s and the revoke + stop fires at 4 s' "$base/livecam?who=Nr&navat=3900&navto=http%3A%2F%2Flocalhost%3A$Port%2Flivecam%3Fwho%3DNr2" @('--auto-revoke-ms=4000', '--revoke-stop=all', '--answer-after-revoke=deny')
Scenario 'N7. the same race with the stop sent only once (--stop-once), a navigation caught mid-flight' "$base/livecam?who=Nr&navat=3300&navto=http%3A%2F%2Flocalhost%3A$Port%2Flivecam%3Fwho%3DNr2" @('--auto-revoke-ms=4000', '--revoke-stop=all', '--answer-after-revoke=deny', '--stop-once')
Scenario 'N8. the same race with the stop repeated (default)' "$base/livecam?who=Nr&navat=3300&navto=http%3A%2F%2Flocalhost%3A$Port%2Flivecam%3Fwho%3DNr2" @('--auto-revoke-ms=4000', '--revoke-stop=all', '--answer-after-revoke=deny')
if ($Sweep -and $Sweep -notlike 'grant-*') {
  # sweep the navigation time across the moment the stop fires (4 s after C is created); -Sweep once = stop sent once, repeat = default
  foreach ($navat in 3300, 3500, 3700, 3900, 4100) {
    $fl = @('--auto-revoke-ms=4000', '--revoke-stop=all', '--answer-after-revoke=deny'); if ($Sweep -eq 'once') { $fl += '--stop-once' }
    Scenario "S$navat. navigate at $navat ms, stop mode $Sweep" "$base/livecam?who=Nr&navat=$navat&navto=http%3A%2F%2Flocalhost%3A$Port%2Flivecam%3Fwho%3DNr2" $fl
  }
}
if ($Sweep -like 'grant-*') {
  # deterministic race: the page navigates (not timed against the revoke) and the revoke + stop fires N ms after the SECOND document's grant
  $mode = $Sweep -replace '^grant-', ''
  foreach ($ms in 0, 40, 120, 400) {
    $fl = @('--revoke-after-grant-ms=' + $ms, '--revoke-stop=all', '--answer-after-revoke=deny'); if ($mode -eq 'once') { $fl += '--stop-once' }
    Scenario "G$ms. revoke + stop $ms ms after the new document's grant, stop mode $mode" "$base/livecam?who=Nr&navat=2500&navto=http%3A%2F%2Flocalhost%3A$Port%2Flivecam%3Fwho%3DNr2" $fl
  }
}
Get-Process m0_spike -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Stop-Process -Id $srv.Id -Force -ErrorAction SilentlyContinue
Remove-Item revoke.out, revoke.err, $srvlog -ErrorAction SilentlyContinue
Pop-Location
