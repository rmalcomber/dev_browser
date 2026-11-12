# Destroy-on-a-busy-page check. For each scenario workspace C loads a page that is busy (spinning script, endless event stream,
# open WebSocket, endless download, alert() with onbeforeunload, popup), its view is destroyed with Ctrl+6, and the script reports
# whether the app survived, how many browsers and processes were left, what the server saw, and whether C can be recreated.
# Needs node on PATH. Takes over the keyboard for ~25 s per scenario. Disposable.
param([string[]]$Routes = @('busy', 'sse', 'ws', 'download', 'alert', 'popup'), [int]$AutoDestroyMs = 0, [string]$AppFlags = '', [string]$Exe = (Join-Path $PSScriptRoot 'target/debug/m0_spike.exe'), [int]$Port = 18766)
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class U { [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr e); }
'@
[void][U]::SetProcessDPIAware()
function Chord($vk) { [U]::keybd_event(0x11,0,0,[UIntPtr]::Zero); [U]::keybd_event($vk,0,0,[UIntPtr]::Zero); [U]::keybd_event($vk,0,2,[UIntPtr]::Zero); [U]::keybd_event(0x11,0,2,[UIntPtr]::Zero) }
function Count { (Get-Process m0_spike -ErrorAction SilentlyContinue | Measure-Object).Count }
$Routes = @($Routes | ForEach-Object { $_ -split ',' })   # -File passes a comma list as one string
$dir = Split-Path $Exe; Push-Location $dir
$srvlog = Join-Path $dir 'busy-server.log'
$srv = Start-Process node -ArgumentList (Join-Path $PSScriptRoot 'slow-server.js'), $Port, 3000, $srvlog -PassThru -WindowStyle Hidden
Start-Sleep 1
$red = 'data:text/html,<body%20style=background:%23d00><b>A</b>'; $blue = 'data:text/html,<body%20style=background:%2300d><b>B</b>'
$rows = @()
foreach ($route in $Routes) {
  Get-Process m0_spike -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue; Start-Sleep 1
  Remove-Item -Recurse -Force .m0-data -ErrorAction SilentlyContinue; Remove-Item $srvlog -ErrorAction SilentlyContinue
  Write-Host "=== $route"
  $argv = @("--url-a=$red", "--url-b=$blue", "--url-c=http://127.0.0.1:$Port/$route", '--exit-after=120')
  if ($AutoDestroyMs) { $argv += "--auto-destroy-ms=$AutoDestroyMs" }
  if ($AppFlags) { $argv += ($AppFlags -split ' ') }
  $p = Start-Process $Exe -ArgumentList $argv -PassThru -RedirectStandardOutput busy.out -RedirectStandardError busy.err
  Start-Sleep 10; $p.Refresh(); $h = $p.MainWindowHandle
  [void][U]::SetWindowPos($h, [IntPtr](-1), 0, 0, 0, 0, 0x43); [void][U]::SetForegroundWindow($h); Start-Sleep 1
  $base = Count
  Chord 0x34; Start-Sleep 3.5          # create C; the page goes busy ~0.4 s after it loads
  $during = Count; $winsDuring = (Get-Process m0_spike | Where-Object { $_.MainWindowTitle }).Count
  Chord 0x36; Start-Sleep 3.5          # destroy C while it is busy
  $aliveAfter = -not $p.HasExited; $after = Count
  $fell = (Get-Content busy.out | Where-Object { $_ -match 'fell back' }).Count
  $by = (Get-Content busy.out | Where-Object { $_ -match 'destroyed (context kept) by (w+)' } | Select-Object -First 1) -replace '.* by ', ''
  $closed = (Get-Content busy.out | Where-Object { $_ -match 'browser closed; (\d+) left' } | Select-Object -Last 1)
  Chord 0x34; Start-Sleep 3.5          # recreate C from the kept context
  $recreated = (Get-Content busy.out | Where-Object { $_ -match 'recreating workspace C' }).Count
  $aliveEnd = -not $p.HasExited
  $dialogLines = (Get-Content busy.out | Where-Object { $_ -match 'dialog|permission' } | ForEach-Object { $_ -replace '^[m0] ', '' }) -join ' | '
  if ($dialogLines) { Write-Host "  app: $dialogLines" }
  $stderr = Get-Content busy.err | Where-Object { $_ -match 'panicked|RefCell|already borrowed' } | Select-Object -First 1
  $srvLines = if (Test-Path $srvlog) { Get-Content $srvlog | ForEach-Object { ($_ -split ' ', 2)[1] } } else { @() }
  Get-Process m0_spike -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue; Start-Sleep 1
  $rows += [pscustomobject]@{ Route = "$route $AppFlags"; Procs = "$base / $during / $after"; AliveAfterDestroy = $aliveAfter; FellBack = $fell; By = "$by"; Closed = "$closed"; Recreated = ($recreated -gt 0); AliveEnd = $aliveEnd; WindowsWithTitle = $winsDuring; Stderr = "$stderr"; LeftAfterKill = (Count) }
  Write-Host ($srvLines | Where-Object { $_ -notmatch 'favicon|listening' } | ForEach-Object { "  server: $_" })
}
Stop-Process -Id $srv.Id -Force -ErrorAction SilentlyContinue
Remove-Item busy.out, busy.err, $srvlog -ErrorAction SilentlyContinue
Pop-Location
Write-Host ''; $rows | Format-Table -AutoSize | Out-String -Width 220 | Write-Host
Write-Host '(Procs = m0_spike.exe processes: before C / while C is busy / after destroying it)'
