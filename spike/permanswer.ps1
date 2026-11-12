# Accept/deny permission check. For each kind and answer: workspace C asks for the permission and the app answers it; C is then
# destroyed and recreated (second visit, no new prompt expected), the app is closed and restarted (third visit, answer flag off so a
# new prompt would be held and logged), while panes A and B (their own profiles, same origin) only query the state. The page
# reports its view of the permission to the server, which this script prints. Needs node on PATH. Disposable.
param([string]$Kinds = 'geo,notify,clipboard,camera', [string]$Answers = 'accept,deny', [string]$Exe = (Join-Path $PSScriptRoot 'target/debug/m0_spike.exe'), [int]$Port = 18770)
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class P { [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
 [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr e); }
'@
[void][P]::SetProcessDPIAware()
function Chord($vk) { [P]::keybd_event(0x11,0,0,[UIntPtr]::Zero); [P]::keybd_event($vk,0,0,[UIntPtr]::Zero); [P]::keybd_event($vk,0,2,[UIntPtr]::Zero); [P]::keybd_event(0x11,0,2,[UIntPtr]::Zero) }
$dir = Split-Path $Exe; Push-Location $dir
$srvlog = Join-Path $dir 'permanswer-server.log'
$srv = Start-Process node -ArgumentList (Join-Path $PSScriptRoot 'slow-server.js'), $Port, 3000, $srvlog -PassThru -WindowStyle Hidden
Start-Sleep 1
$script:appSeen = 0
function Launch($kind, $flags) {
  $script:appSeen = 0
  $base = "http://127.0.0.1:$Port/permcheck?kind=$kind"
  $argv = @("--url-a=$base&mode=query&who=A", "--url-b=$base&mode=query&who=B", "--url-c=$base&who=C", '--exit-after=120') + $flags
  $p = Start-Process $Exe -ArgumentList $argv -PassThru -RedirectStandardOutput permanswer.out -RedirectStandardError permanswer.err
  Start-Sleep 10; $p.Refresh(); $h = $p.MainWindowHandle
  [void][P]::SetWindowPos($h, [IntPtr](-1), 0, 0, 0, 0, 0x43); [void][P]::SetForegroundWindow($h); Start-Sleep 1
  return @($p, $h)
}
function CloseGracefully($p, $h) { $r = New-Object P+RECT; [void][P]::GetWindowRect($h, [ref]$r); [void][P]::SetCursorPos($r.R - 24, $r.T + 15); Start-Sleep -Milliseconds 400; [P]::mouse_event(2,0,0,0,[UIntPtr]::Zero); [P]::mouse_event(4,0,0,0,[UIntPtr]::Zero); $p | Wait-Process -Timeout 15 -ErrorAction SilentlyContinue }
function Show($tag) {
  Write-Host "  [$tag]"
  if (Test-Path $srvlog) { Get-Content $srvlog | Where-Object { $_ -match ' report ' } | ForEach-Object { '    ' + (($_ -split ' report ', 2)[1]) }; Clear-Content $srvlog }
  $lines = @(Get-Content permanswer.out -ErrorAction SilentlyContinue | Where-Object { $_ -match 'permission (prompt|request)' -and $_ -notmatch 'dismissed by CEF' })
  $new = @($lines | Select-Object -Skip $script:appSeen)
  foreach ($l in $new) { '    app: ' + ($l -replace '^[m0] ', '' -replace 'http://127.0.0.1:d+', '') }
  if ($new.Count -eq 0) { '    app: (no permission prompt reached the app)' }
  $script:appSeen = $lines.Count
}
foreach ($kind in ($Kinds -split ',')) { foreach ($answer in ($Answers -split ',')) {
  Get-Process m0_spike -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue; Start-Sleep 1
  Remove-Item -Recurse -Force .m0-data -ErrorAction SilentlyContinue; Set-Content $srvlog ''
  Write-Host "=== $kind, app answers: $answer"
  $p, $h = Launch $kind @("--answer-permissions=$answer")
  Show 'panes A and B at startup (their own profiles; they only query)'
  Chord 0x34; Start-Sleep 3.5; Show 'visit 1: C asks'
  Chord 0x36; Start-Sleep 1.5; Chord 0x34; Start-Sleep 3.5; Show 'visit 2: C destroyed and recreated, asks again'
  CloseGracefully $p $h; Start-Sleep 2
  $p, $h = Launch $kind @('--own-permissions')
  Show 'after restart: panes A and B'
  Chord 0x34; Start-Sleep 3.5; Show 'visit 3 after restart: C asks (new prompts would be held)'
  Get-Process m0_spike -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
} }
Stop-Process -Id $srv.Id -Force -ErrorAction SilentlyContinue
Remove-Item permanswer.out, permanswer.err, $srvlog -ErrorAction SilentlyContinue
Pop-Location
