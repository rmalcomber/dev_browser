# M2.2 checks on the real window (planning/m2-plan.md): history and bookmarks, found by the launcher, per workspace.
#   A  A page visited earlier (no longer open) is found in history and opens in a new tab.
#   B  Ctrl+D bookmarks the shown page; the bookmark opens from the launcher; "Clear this workspace's history" removes history but the
#      bookmark is still found (so the bookmark is its own record, not a view of history).
#   C  S1 on the real window: a bookmark that exists only in another workspace is not found by a normal search, is found when "All
#      workspaces" is chosen on purpose, and opens in its own workspace.
# Pages have unique titles (Quokka Atlas, Marmot Index, Zephyr Ledger) so a launcher command can never match by accident.
# Takes over the mouse and keyboard (about 2 minutes). Needs a build and node. Exit code 0 = pass.
param([int]$Wait = 20)
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
function Open-Launcher($app) { $n = Count $app 'launcher shown'; Send-Keys 'ctrl+p'; Wait-Count $app 'launcher shown' ($n + 1) 8 }
function Shown($app) { @(Log $app | ForEach-Object { if ($_ -match '\[host\] tab (tab-\S+): shown \(workspace (ws-\S+)\)') { "$($Matches[2])/$($Matches[1])" } }) }
function Query-Box($app) { $c = Get-Client $app; $x = (($c.W - 640) / 2) + 300; @($x, 76) }
function All-Button($app) { $c = Get-Client $app; $x = (($c.W - 640) / 2) + 171; @($x, 442) }
# open the launcher, type the query, press Enter: runs the first result (if any)
function Search-Run($app, [string]$text) {
  [void](Open-Launcher $app)
  $b = Query-Box $app; Click-Client $app $b[0] $b[1]; Send-Text $text; Start-Sleep -Milliseconds 700; Send-Keys 'enter'; Start-Sleep -Milliseconds 1500
  # if nothing ran the launcher is still open: close it
  if ((Count $app 'launcher shown') -gt (Count $app 'launcher hidden')) { Send-Keys 'esc'; Start-Sleep -Milliseconds 500 }
}

$srv = $null; $app = $null
try {
  $srv = Start-TestServer
  $h = "127.0.0.1:$($srv.Port)"
  $app = Start-App -Url "http://$h/colours.html"
  Start-Sleep -Milliseconds 800

  # ---- A  history
  Write-Host 'History'
  Go $app "$h/places-a.html"; if (-not (Wait-Count $app 'page title: Quokka Atlas' 1)) { Fail 'A: the first page did not load' }
  Start-Sleep -Milliseconds 800
  Go $app "$h/places-b.html"; if (-not (Wait-Count $app 'page title: Marmot Index' 1)) { Fail 'A: the second page did not load' }
  Start-Sleep -Milliseconds 800
  Send-Keys 'ctrl+d'; Start-Sleep -Milliseconds 800      # bookmark Marmot Index
  Go $app "$h/colours.html"; Start-Sleep -Milliseconds 1500   # the tab moves on: neither page is open any more
  Search-Run $app 'quokka'
  if (Wait-Count $app 'page title: Quokka Atlas' 2 10) { Pass 'A: a page visited earlier was found in history and opened' } else { Fail 'A: history search did not find the page' }
  Send-Keys 'ctrl+w'; Start-Sleep -Milliseconds 800

  # ---- B  bookmarks
  Write-Host 'Bookmarks'
  Search-Run $app 'marmot'
  if (Wait-Count $app 'page title: Marmot Index' 2 10) { Pass 'B: Ctrl+D made a bookmark and the launcher opened it' } else { Fail 'B: the bookmark was not found' }
  Send-Keys 'ctrl+w'; Start-Sleep -Milliseconds 800
  Search-Run $app 'clear history'
  Start-Sleep -Milliseconds 800
  $views = Count $app 'page title: Quokka Atlas'
  Search-Run $app 'quokka'
  if ((Count $app 'page title: Quokka Atlas') -eq $views) { Pass 'B: after clearing history the visited page is no longer found' } else { Fail 'B: history survived "Clear history"' }
  $views = Count $app 'page title: Marmot Index'
  Search-Run $app 'marmot'
  if (Wait-Count $app 'page title: Marmot Index' ($views + 1) 10) { Pass 'B: but the bookmark is still found (it is its own record)' } else { Fail 'B: clearing history removed the bookmark' }
  Send-Keys 'ctrl+w'; Start-Sleep -Milliseconds 800

  # ---- C  S1 across workspaces
  Write-Host 'Another workspace'
  Send-Keys 'alt+n'; Start-Sleep -Milliseconds 800; Send-Text 'Second'; Send-Keys 'enter'; Start-Sleep -Milliseconds 2500
  Go $app "$h/secret.html"; if (-not (Wait-Count $app 'page title: Zephyr Ledger' 1)) { Fail 'C: the second workspace page did not load' }
  Start-Sleep -Milliseconds 800
  Send-Keys 'ctrl+d'; Start-Sleep -Milliseconds 800
  Go $app "$h/colours.html"; Start-Sleep -Milliseconds 1500
  Send-Keys 'alt+pageup'; Start-Sleep -Milliseconds 1500
  $shown = @(Shown $app).Count
  Search-Run $app 'zephyr'
  if (@(Shown $app).Count -eq $shown) { Pass 'C: a normal search found nothing of the other workspace''s bookmark or history (S1)' } else { Fail 'C: a search found data of another workspace' }
  [void](Open-Launcher $app)
  $b = All-Button $app; Click-Client $app $b[0] $b[1]; Start-Sleep -Milliseconds 600
  $q = Query-Box $app; Click-Client $app $q[0] $q[1]; Send-Text 'zephyr'; Start-Sleep -Milliseconds 700; Send-Keys 'enter'; Start-Sleep -Milliseconds 1500
  if (@(Shown $app).Count -gt $shown -and (@(Shown $app)[-1] -split '/')[0] -ne (@(Shown $app)[0] -split '/')[0]) { Pass 'C: with "All workspaces" chosen on purpose it was found and opened in its own workspace' } else { Fail 'C: the wide search did not open it in the other workspace' }
  if (-not $app.Proc.Responding) { Fail 'the window stopped responding' }
}
finally {
  Stop-App $app
  if ($srv) { Stop-Process -Id $srv.Proc.Id -Force -ErrorAction SilentlyContinue }
}
if ((Get-FocusRestores) -gt 0) { Write-Host "note: the app window lost the foreground $(Get-FocusRestores) time(s) during the run and was brought back" -ForegroundColor Yellow }
if ($failures.Count -gt 0) { Write-Host "M2.2 places check: $($failures.Count) failure(s)" -ForegroundColor Red; exit 1 }
Write-Host 'M2.2 places check: all passed' -ForegroundColor Green
