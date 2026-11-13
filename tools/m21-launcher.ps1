# M2.1 checks on the real window (planning/m2-plan.md): the launcher overlay and scoped search, driven with real keys and a real mouse.
#   A  Ctrl+P opens the launcher with the focus in a page, in a frame of a page and in the address bar; Esc closes it and the page has the keys back
#      (and a page never sees Ctrl+P or Esc while it is open).
#   B  Dragging the title bar moves the launcher; the position is remembered.
#   C  Scope (S1, S2, S4 on the real window): a page title that exists only in another workspace is NOT found by a normal search; with
#      "All workspaces" chosen on purpose it is found and opens; after the launcher closes the wide scope is gone again.
#   D  A command run from the launcher (New tab) runs.
# The launcher sits at a fixed place (top-centre, 640 wide) and its controls at fixed distances inside it (ui\src\style.css); if the layout
# changes, update the numbers below. Takes over the mouse and keyboard (about 2 minutes). Needs a build and node. Exit code 0 = pass.
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
function Close-Launcher($app) { $n = Count $app 'launcher hidden'; Send-Keys 'esc'; Wait-Count $app 'launcher hidden' ($n + 1) 8 }
function Last-Launcher-Pos($app) { $l = @(Log $app | Where-Object { $_ -match 'launcher at \((-?\d+), (-?\d+)\)' }); if ($l.Count) { [void]($l[-1] -match 'launcher at \((-?\d+), (-?\d+)\)'); @([int]$Matches[1], [int]$Matches[2]) } }
function Shown($app) { @(Log $app | ForEach-Object { if ($_ -match '\[host\] tab (tab-\S+): shown \(workspace (ws-\S+)\)') { "$($Matches[2])/$($Matches[1])" } }) }
# the launcher: left edge from the logged default (window width 1200 gives 280); the query box and the "All workspaces" button inside it
function Query-Box($app) { $c = Get-Client $app; $x = (($c.W - 640) / 2) + 300; @($x, 76) }
function All-Button($app) { $c = Get-Client $app; $x = (($c.W - 640) / 2) + 171; @($x, 442) }
function Type-Query($app, [string]$text) { $b = Query-Box $app; Click-Client $app $b[0] $b[1]; Send-Text $text }

$srv = $null; $app = $null
try {
  $srv = Start-TestServer
  $h = "127.0.0.1:$($srv.Port)"
  $app = Start-App -Url "http://$h/keys.html"
  Start-Sleep -Milliseconds 800

  # ---- A
  Write-Host 'Opening and closing'
  Click-Client $app 270 50   # the page's input
  if (Open-Launcher $app) { Pass 'A: Ctrl+P opened the launcher with the focus in a page input' } else { Fail 'A: Ctrl+P did not open the launcher' }
  if (Close-Launcher $app) { Pass 'A: Esc closed it' } else { Fail 'A: Esc did not close the launcher' }
  Send-Text 'q'; Start-Sleep -Milliseconds 600
  if (Wait-Count $app 'seen=q$' 1 5) { Pass 'A: the page has the keyboard back, and saw only the q (not Ctrl+P or Esc)' } else { Fail 'A: the page did not get the focus back, or saw a launcher key' }
  Click-Client $app 300 130  # the input inside the iframe
  if (Open-Launcher $app) { Pass 'A: Ctrl+P opened it with the focus in a frame' } else { Fail 'A: Ctrl+P did nothing from a frame' }
  [void](Close-Launcher $app)
  Send-Keys 'ctrl+l'; Start-Sleep -Milliseconds 600
  if (Open-Launcher $app) { Pass 'A: Ctrl+P opened it with the focus in the address bar' } else { Fail 'A: Ctrl+P did nothing from the address bar' }
  [void](Close-Launcher $app)
  Send-Keys 'ctrl+p'; Start-Sleep -Milliseconds 600
  $n = Count $app 'launcher hidden'; Send-Keys 'ctrl+p'
  if (Wait-Count $app 'launcher hidden' ($n + 1) 5) { Pass 'A: Ctrl+P again closes it' } else { Fail 'A: Ctrl+P did not close an open launcher' }

  # ---- B
  Write-Host 'Dragging'
  [void](Open-Launcher $app)
  $c = Get-Client $app; $left = [int](($c.W - 640) / 2)
  Drag-Client $app ($left + 200) 39 ($left + 300) 99
  [void](Close-Launcher $app)
  $pos = Last-Launcher-Pos $app
  Write-Host "      left at $($pos -join ', ')"
  if ($pos -and [math]::Abs($pos[0] - ($left + 100)) -le 12 -and [math]::Abs($pos[1] - 84) -le 12) { Pass 'B: a real drag of the title bar moved the launcher' } else { Fail "B: the launcher was at $($pos -join ',') after the drag, expected about $($left + 100),84" }
  [void](Open-Launcher $app); [void](Close-Launcher $app)
  $again = Last-Launcher-Pos $app
  if ($again -and $again[0] -eq $pos[0] -and $again[1] -eq $pos[1]) { Pass 'B: it reopened where it was left' } else { Fail "B: it reopened at $($again -join ',') not $($pos -join ',')" }
  # put it back where the scope checks expect it
  [void](Open-Launcher $app); Drag-Client $app ($left + 300) 99 ($left + 200) 39; [void](Close-Launcher $app)

  # ---- C
  Write-Host 'Scope'
  Send-Keys 'alt+n'; Start-Sleep -Milliseconds 800; Send-Text 'Second'; Send-Keys 'enter'; Start-Sleep -Milliseconds 2500
  Go $app "$h/secret.html"
  if (Wait-Count $app 'page title: Zephyr Ledger' 1) { Pass 'C: the second workspace has a page called Zephyr Ledger' } else { Fail 'C: the second workspace page did not load' }
  Start-Sleep -Milliseconds 800
  Send-Keys 'alt+pageup'; Start-Sleep -Milliseconds 1500    # back to the first workspace
  $shown = @(Shown $app).Count
  [void](Open-Launcher $app); Type-Query $app 'zephyr'; Start-Sleep -Milliseconds 600; Send-Keys 'enter'; Start-Sleep -Milliseconds 1500
  if (@(Shown $app).Count -eq $shown) { Pass 'C: a normal search from the first workspace found nothing of the second workspace (Enter did nothing)' } else { Fail 'C: a search in the first workspace opened a tab of another workspace' }
  [void](Close-Launcher $app)
  [void](Open-Launcher $app)
  $b = All-Button $app; Click-Client $app $b[0] $b[1]; Start-Sleep -Milliseconds 600
  Type-Query $app 'zephyr'; Start-Sleep -Milliseconds 600; Send-Keys 'enter'
  if (Wait-Count $app '\[host\] launcher hidden' 6 6) { }
  Start-Sleep -Milliseconds 1500
  $last = @(Shown $app)[-1]
  $firstWs = (@(Shown $app)[0] -split '/')[0]
  if (@(Shown $app).Count -gt $shown -and ($last -split '/')[0] -ne $firstWs) { Pass 'C: with "All workspaces" chosen on purpose the same search found it and opened it in the second workspace' } else { Fail 'C: the wide search did not open the tab of the other workspace' }
  # back to the first workspace; the wide scope must be gone
  Send-Keys 'alt+pageup'; Start-Sleep -Milliseconds 1500
  $shown = @(Shown $app).Count
  [void](Open-Launcher $app); Type-Query $app 'zephyr'; Start-Sleep -Milliseconds 600; Send-Keys 'enter'; Start-Sleep -Milliseconds 1500
  if (@(Shown $app).Count -eq $shown) { Pass 'C: after closing, the next launcher is back to this workspace only (S4)' } else { Fail 'C: the wide scope survived a close' }
  [void](Close-Launcher $app)

  # ---- D
  Write-Host 'Commands'
  $created = Count $app 'view created'
  [void](Open-Launcher $app); Type-Query $app 'new tab'; Start-Sleep -Milliseconds 600; Send-Keys 'enter'
  if (Wait-Count $app 'view created' ($created + 1) 10) { Pass 'D: "New tab" run from the launcher made a tab' } else { Fail 'D: the command did not run' }
  if (-not $app.Proc.Responding) { Fail 'the window stopped responding' }
}
finally {
  Stop-App $app
  if ($srv) { Stop-Process -Id $srv.Proc.Id -Force -ErrorAction SilentlyContinue }
}
if ((Get-FocusRestores) -gt 0) { Write-Host "note: the app window lost the foreground $(Get-FocusRestores) time(s) during the run and was brought back" -ForegroundColor Yellow }
if ($failures.Count -gt 0) { Write-Host "M2.1 launcher check: $($failures.Count) failure(s)" -ForegroundColor Red; exit 1 }
Write-Host 'M2.1 launcher check: all passed' -ForegroundColor Green
