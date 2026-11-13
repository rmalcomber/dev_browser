# M1.7 restore checks on the real window (planning/m1-plan.md): what comes back after a restart, a clean one and a killed one.
#   A  The window size comes back after a clean close.
#   B  Two workspaces, their tabs and the selected tab come back after the app is KILLED (no clean exit), and a login made more than
#      Chromium's cookie flush interval earlier (about 30 s) is still there: the workspace's own cookie and localStorage token.
#   C  A second workspace on the same site is still separate after the kill (it made its own token, not the first one's).
# Takes over the mouse and keyboard (about 3 minutes). Needs a build and node. Exit code 0 = pass.
param([int]$Wait = 25)
. "$PSScriptRoot\lib\app.ps1"
$failures = @()
function Fail($m) { $script:failures += $m; Write-Host "FAIL: $m" -ForegroundColor Red }
function Pass($m) { Write-Host "ok:   $m" -ForegroundColor Green }
function Log($app) { Get-Content $app.Out, $app.Err -ErrorAction SilentlyContinue }
function Count($app, [string]$pattern) { @(Log $app | Where-Object { $_ -match $pattern }).Count }
function Wait-Count($app, [string]$pattern, [int]$n, [double]$seconds = $Wait) {
  $sw = [Diagnostics.Stopwatch]::StartNew()
  while ($sw.Elapsed.TotalSeconds -lt $seconds) { if ((Count $app $pattern) -ge $n) { return $true }; Start-Sleep -Milliseconds 100 }
  $false
}
function Go($app, [string]$text) { Send-Keys 'ctrl+l'; Start-Sleep -Milliseconds 700; Send-Text $text; Send-Keys 'enter' }
function Shown($app) { @(Log $app | ForEach-Object { if ($_ -match '\[host\] tab (tab-\S+): shown \(workspace (ws-\S+)\)') { "$($Matches[2])/$($Matches[1])" } }) }
function Identities($app) { @(Log $app | ForEach-Object { if ($_ -match '\[id\] had=(\S+)/(\S+) now=(\S+)') { [pscustomobject]@{ Cookie = $Matches[1]; Local = $Matches[2]; Now = $Matches[3] } } }) }

$srv = $null; $app = $null; $dir = $null
try {
  $srv = Start-TestServer
  $h = "127.0.0.1:$($srv.Port)"
  $app = Start-App -Url "http://$h/identity.html"
  $dir = $app.Dir
  Start-Sleep -Milliseconds 800
  $idOne = (Identities $app)[0]
  # a second workspace with its own tab on the same site, and a second tab after it
  Send-Keys 'alt+n'; Start-Sleep -Milliseconds 800; Send-Text 'Second'; Send-Keys 'enter'; Start-Sleep -Milliseconds 2500
  Go $app "$h/identity.html"
  [void](Wait-Count $app '\[id\] had=none/none' 2)
  $idTwo = (Identities $app)[-1]
  Send-Keys 'ctrl+t'; Start-Sleep -Milliseconds 1200
  Go $app "$h/nav-a.html"; Start-Sleep -Seconds 2
  $selectedBefore = @(Shown $app)[-1]

  # ---- A  a resized window comes back after a clean close
  $rect = New-Object Win+RECT; [void][Win]::GetWindowRect($app.Handle, [ref]$rect)
  [void][Win]::SetWindowPos($app.Handle, [IntPtr]::Zero, $rect.L, $rect.T, 1000, 640, 0x4)
  Start-Sleep -Seconds 1
  $c1 = Get-Client $app
  Write-Host "      client area before closing: $($c1.W)x$($c1.H)"
  if (-not (Close-App $app)) { Fail 'the app did not exit after one close' }
  Stop-App
  $app = Start-App -Url "http://$h/colours.html" -ReuseDir $dir
  Start-Sleep -Seconds 1
  $c2 = Get-Client $app
  if ([math]::Abs($c2.W - $c1.W) -le 4 -and [math]::Abs($c2.H - $c1.H) -le 4) { Pass "A: the window came back at the same size ($($c2.W)x$($c2.H))" } else { Fail "A: the window was $($c1.W)x$($c1.H), came back $($c2.W)x$($c2.H)" }

  # ---- B  a kill, after the cookie flush interval
  Write-Host 'Waiting 40 s so the login is flushed to disk, then killing the app'
  Start-Sleep -Seconds 40
  Get-Process dev_browser -ErrorAction SilentlyContinue | Stop-Process -Force
  Start-Sleep -Seconds 2
  $app = Start-App -Url "http://$h/colours.html" -ReuseDir $dir
  Start-Sleep -Seconds 3
  $first = @(Shown $app)[0]
  if ($first -eq $selectedBefore) { Pass 'B: after a kill the selected tab is the one shown' } else { Fail "B: after a kill $first was shown, expected $selectedBefore" }
  $seen = @($first)
  foreach ($k in 1..2) { Send-Keys 'ctrl+tab'; Start-Sleep -Milliseconds 1300; $seen += @(Shown $app)[-1] }
  if (@($seen | Select-Object -Unique).Count -eq 2 -and $seen[2] -eq $seen[0]) { Pass 'B: both tabs of the second workspace were restored' } else { Fail "B: tabs after the kill: $($seen -join ' ')" }
  Send-Keys 'alt+pageup'; Start-Sleep -Seconds 3
  $back = @(Identities $app)[-1]
  if ($back -and $back.Cookie -eq $idOne.Now -and $back.Local -eq $idOne.Now) { Pass 'B: workspace one is still logged in after the kill (cookie and localStorage)' } else { Fail "B: workspace one reported $($back.Cookie)/$($back.Local), expected its token $($idOne.Now)" }
  # ---- C
  Send-Keys 'alt+pagedown'; Start-Sleep -Seconds 2
  Go $app "$h/identity.html"; Start-Sleep -Seconds 3
  $two = @(Identities $app)[-1]
  if ($two.Cookie -eq $idTwo.Now -and $two.Cookie -ne $idOne.Now) { Pass 'C: workspace two kept its own, different login' } else { Fail "C: workspace two saw $($two.Cookie), expected $($idTwo.Now) (workspace one has $($idOne.Now))" }
  if (-not $app.Proc.Responding) { Fail 'the window stopped responding' }
}
finally {
  Stop-App
  if ($srv) { Stop-Process -Id $srv.Proc.Id -Force -ErrorAction SilentlyContinue }
  if ($dir -and (Test-Path $dir)) { Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue }
}
if ((Get-FocusRestores) -gt 0) { Write-Host "note: the app window lost the foreground $(Get-FocusRestores) time(s) during the run and was brought back" -ForegroundColor Yellow }
if ($failures.Count -gt 0) { Write-Host "M1.7 restore check: $($failures.Count) failure(s)" -ForegroundColor Red; exit 1 }
Write-Host 'M1.7 restore check: all passed' -ForegroundColor Green
