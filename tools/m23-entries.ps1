# M2.3 checks on the real window (planning/m2-plan.md): Home Screen entries and ranking by how often and how lately results were opened.
#   A  "Save this page as an entry" (with an environment, or without) from the launcher; an entry is found and opens in a new tab.
#   B  Ranking: two entries match "orchid" equally and Alpha comes first; after Beta has been opened a few times Beta comes first.
#   C  "Clear this workspace's history" also clears how often things were opened (Alpha is first again), and the entries themselves stay.
# Pages with unique titles (Orchid Alpha, Orchid Beta). Takes over the mouse and keyboard (about 2 minutes). Needs a build and node.
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
function Query-Box($app) { $c = Get-Client $app; $x = (($c.W - 640) / 2) + 300; @($x, 76) }
function Search-Run($app, [string]$text) {
  [void](Open-Launcher $app)
  $b = Query-Box $app; Click-Client $app $b[0] $b[1]; Send-Text $text; Start-Sleep -Milliseconds 700; Send-Keys 'enter'; Start-Sleep -Milliseconds 1500
  if ((Count $app 'launcher shown') -gt (Count $app 'launcher hidden')) { Send-Keys 'esc'; Start-Sleep -Milliseconds 500 }
}
# the last page title the app saw
function Last-Title($app) { $l = @(Log $app | Where-Object { $_ -match '\[host\] page title: (Orchid \w+)' }); if ($l.Count) { [void]($l[-1] -match 'page title: (Orchid \w+)'); $Matches[1] } }

$srv = $null; $app = $null
try {
  $srv = Start-TestServer
  $h = "127.0.0.1:$($srv.Port)"
  $app = Start-App -Url "http://$h/colours.html"
  Start-Sleep -Milliseconds 800

  # ---- A
  Write-Host 'Saving entries'
  Go $app "$h/orchid-a.html"; if (-not (Wait-Count $app 'page title: Orchid Alpha' 1)) { Fail 'A: the first page did not load' }
  Start-Sleep -Milliseconds 800
  Search-Run $app 'entry prod'          # Save this page as an entry (prod)
  Go $app "$h/orchid-b.html"; if (-not (Wait-Count $app 'page title: Orchid Beta' 1)) { Fail 'A: the second page did not load' }
  Start-Sleep -Milliseconds 800
  Search-Run $app 'save entry'          # Save this page as an entry (the plain one ranks first: shortest)
  Go $app "$h/colours.html"; Start-Sleep -Milliseconds 1500    # the tab moves on: neither page is open
  Search-Run $app 'orchid alpha'
  if (Wait-Count $app 'page title: Orchid Alpha' 2 10) { Pass 'A: a saved entry was found by its name and opened in a new tab' } else { Fail 'A: the entry was not found' }
  Send-Keys 'ctrl+w'; Start-Sleep -Milliseconds 800

  # ---- B
  Write-Host 'Ranking'
  Search-Run $app 'orchid'
  if ((Last-Title $app) -eq 'Orchid Alpha') { Pass 'B: with nothing opened yet, equal matches keep their order (Alpha first)' } else { Fail "B: first result was $(Last-Title $app), expected Alpha" }
  Send-Keys 'ctrl+w'; Start-Sleep -Milliseconds 800
  foreach ($i in 1..3) { Search-Run $app 'orchid beta'; Send-Keys 'ctrl+w'; Start-Sleep -Milliseconds 800 }
  Search-Run $app 'orchid'
  if ((Last-Title $app) -eq 'Orchid Beta') { Pass 'B: after Beta was opened three times it ranks first' } else { Fail "B: first result was $(Last-Title $app), expected Beta" }
  Send-Keys 'ctrl+w'; Start-Sleep -Milliseconds 800

  # ---- C
  Write-Host 'Clearing'
  Search-Run $app 'clear history'; Start-Sleep -Milliseconds 800
  Search-Run $app 'orchid'
  if ((Last-Title $app) -eq 'Orchid Alpha') { Pass 'C: after "Clear history" the counts are gone (Alpha first again) and the entries are still there' } else { Fail "C: first result was $(Last-Title $app), expected Alpha" }
  if (-not $app.Proc.Responding) { Fail 'the window stopped responding' }
}
finally {
  Stop-App $app
  if ($srv) { Stop-Process -Id $srv.Proc.Id -Force -ErrorAction SilentlyContinue }
}
if ((Get-FocusRestores) -gt 0) { Write-Host "note: the app window lost the foreground $(Get-FocusRestores) time(s) during the run and was brought back" -ForegroundColor Yellow }
if ($failures.Count -gt 0) { Write-Host "M2.3 entries check: $($failures.Count) failure(s)" -ForegroundColor Red; exit 1 }
Write-Host 'M2.3 entries check: all passed' -ForegroundColor Green
