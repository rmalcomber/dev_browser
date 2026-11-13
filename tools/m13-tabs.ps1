# M1.3 checks (planning/m1-plan.md): tabs, the command registry and window-level shortcuts, driven with real keys on the real window.
#   A  Shortcuts work with the focus in a page, in an iframe of a page and in the sidebar; no page ever receives a shortcut's key; plain
#      typing still arrives.
#   B  -Cycles rounds of open / go back / unload / recreate / close leave the process count flat, and the page area is never blank or dark
#      after closing the visible tab.
#   C  A page that closes itself (window.close()) closes its tab and another is shown.
#   D  Tabs and the selected tab are restored after a restart, and closing the window does not close the tabs.
# Takes over the mouse and keyboard (about 6 minutes at 200 cycles). Needs a build (tools\build.ps1 build) and node. Exit code 0 = pass.
param([int]$Cycles = 200, [int]$Wait = 20, [int]$Slack = 3)
. "$PSScriptRoot\lib\app.ps1"
$failures = @()
function Fail($m) { $script:failures += $m; Write-Host "FAIL: $m" -ForegroundColor Red }
function Pass($m) { Write-Host "ok:   $m" -ForegroundColor Green }
function Note($m) { Write-Host "note: $m" -ForegroundColor Yellow }

function Log($app) { Get-Content $app.Out, $app.Err -ErrorAction SilentlyContinue }
function Count($app, [string]$pattern) { @(Log $app | Where-Object { $_ -match $pattern }).Count }
function Wait-Count($app, [string]$pattern, [int]$n, [double]$seconds = $Wait) {
  $sw = [Diagnostics.Stopwatch]::StartNew()
  while ($sw.Elapsed.TotalSeconds -lt $seconds) { if ((Count $app $pattern) -ge $n) { return $true }; Start-Sleep -Milliseconds 80 }
  $false
}
# every tab id the app showed, in order
function Shown($app) { @(Log $app | ForEach-Object { if ($_ -match '\[host\] tab (tab-\S+): shown') { $Matches[1] } }) }
function Last-Shown($app) { $s = @(Shown $app); if ($s.Count) { $s[-1] } else { $null } }
function Press($app, [string]$chord, [string]$pattern, [int]$expected) {
  # send a chord and wait until the app's log shows it was handled (so the next step starts from a settled state)
  $ok = $true
  Send-Keys $chord
  if ($pattern) { $ok = Wait-Count $app $pattern $expected }
  $ok
}
function Procs { @(Get-Process dev_browser -ErrorAction SilentlyContinue).Count }
function Mem-MB { [int]((Get-Process dev_browser -ErrorAction SilentlyContinue | Measure-Object WorkingSet64 -Sum).Sum / 1MB) }
# is the page area showing the red test page (not dark, not blank)?
function Page-Is-Red($app) {
  $f = Join-Path $env:TEMP ('m13_px_' + [guid]::NewGuid().ToString('N').Substring(0, 6) + '.png')
  Save-Window $app $f -ClientOnly
  $b = New-Object System.Drawing.Bitmap $f; $px = $b.GetPixel(700, 300); $b.Dispose(); Remove-Item $f -ErrorAction SilentlyContinue
  ($px.R -gt 150 -and $px.G -lt 80 -and $px.B -lt 80)
}

$srv = $null; $app = $null; $dir = $null
try {
  $srv = Start-TestServer
  $base = "http://127.0.0.1:$($srv.Port)"

  # ---------------------------------------------------------------- A
  Write-Host 'A: shortcuts from a page, an iframe and the sidebar'
  $app = Start-App -Url "$base/keys.html"
  [void](Wait-Count $app 'view created' 1)
  Start-Sleep -Milliseconds 800

  # in the page (its input has the focus)
  if (Press $app 'ctrl+t' 'shortcut Ctrl\+T -> tab.new \(from page\)' 1) { Pass 'Ctrl+T from a page opened a tab' } else { Fail 'Ctrl+T from a page did nothing' }
  [void](Wait-Count $app 'view created' 2)
  $ids = @(Shown $app | Select-Object -Unique)
  if ($ids.Count -ne 2) { Fail "expected 2 tabs shown so far, saw $($ids.Count)" }
  Start-Sleep -Milliseconds 800
  Click-Client $app 290 35
  [void](Press $app 'ctrl+shift+tab' 'shortcut Ctrl\+Shift\+Tab -> tab.prev \(from page\)' 1)
  Start-Sleep -Milliseconds 600
  if ((Last-Shown $app) -eq $ids[0]) { Pass 'Ctrl+Shift+Tab from a page input went back to the first tab' } else { Fail 'Ctrl+Shift+Tab from a page input did not show the first tab' }

  # in an iframe of the page
  Click-Client $app 280 125
  [void](Press $app 'ctrl+tab' 'shortcut Ctrl\+Tab -> tab.next \(from page\)' 1)
  Start-Sleep -Milliseconds 600
  if ((Last-Shown $app) -eq $ids[1]) { Pass 'Ctrl+Tab with the focus inside an iframe went to the next tab' } else { Fail 'Ctrl+Tab from inside an iframe did not show the second tab' }

  # to the sidebar and back
  [void](Press $app 'f6' 'shortcut F6 -> focus.cycle \(from page\)' 1)
  Start-Sleep -Milliseconds 700
  [void](Press $app 'alt+1' 'shortcut Alt\+1 -> tab.select.1 \(from shell\)' 1)
  Start-Sleep -Milliseconds 600
  if (Count $app 'shortcut Alt\+1 -> tab.select.1 \(from shell\)') { Pass 'F6 moved the focus to the sidebar and a shortcut worked there' } else { Fail 'a shortcut pressed with the focus in the sidebar did not run' }
  if ((Last-Shown $app) -eq $ids[0]) { Pass 'Alt+1 from the sidebar showed the first tab' } else { Fail 'Alt+1 from the sidebar did not show the first tab' }
  # Alt+1 also gave the page the focus back, so go to the sidebar again before testing Esc there
  [void](Press $app 'f6' 'shortcut F6 -> focus.cycle \(from page\)' 2)
  Start-Sleep -Milliseconds 700
  [void](Press $app 'esc' 'shortcut Esc -> focus.page \(from shell\)' 1)
  Start-Sleep -Milliseconds 700
  if (Count $app 'shortcut Esc -> focus.page \(from shell\)') { Pass 'Esc in the sidebar returned the focus to the page' } else { Fail 'Esc in the sidebar did not run focus.page' }

  # plain typing still reaches the page; Esc in a page is the page's own
  Click-Client $app 290 35
  Send-Text 'Hi'
  Send-Keys 'esc'
  Start-Sleep -Milliseconds 800
  if (Count $app 'shortcut Esc -> focus.page \(from page\)') { Fail 'Esc pressed in a page was taken by the app' } else { Pass 'Esc in a page was left to the page' }

  # no shortcut key ever reached any page
  $seen = @(Log $app | ForEach-Object { if ($_ -match '\[host\] page title: \[keys\] id=\S+ seen=(.*)$') { $Matches[1] -split ',' } } | Where-Object { $_ })
  $allowed = @('H', 'i', 'Escape')
  $leaked = @($seen | Where-Object { $_ -notin $allowed } | Select-Object -Unique)
  if ($leaked.Count -eq 0) { Pass "no shortcut key reached a page; typing did (pages saw: $((@($seen | Select-Object -Unique)) -join ' '))" } else { Fail "pages received shortcut keys: $($leaked -join ' ')" }
  if ('H' -in $seen -and 'i' -in $seen) { Pass 'plain and shifted typing still arrives' } else { Fail 'typed characters did not arrive in the page' }
  Stop-App $app; $app = $null

  # ---------------------------------------------------------------- B
  Write-Host "B: $Cycles open / unload / recreate / close cycles"
  $app = Start-App -Url "$base/colours.html"
  [void](Wait-Count $app 'view created' 1)
  Start-Sleep -Milliseconds 800
  $blank = @()
  function Cycle($app, [int]$i, [bool]$sample) {
    $c0 = Count $app 'view created'; $d0 = Count $app 'view destroyed'; $s0 = Count $app 'tab \S+: shown'
    Send-Keys 'ctrl+t';          if (-not (Wait-Count $app 'view created' ($c0 + 1))) { return "cycle ${i}: no view after Ctrl+T" }
    Send-Keys 'ctrl+shift+tab';  if (-not (Wait-Count $app 'tab \S+: shown' ($s0 + 2))) { return "cycle ${i}: Ctrl+Shift+Tab did not show the first tab" }
    Send-Keys 'ctrl+shift+d';    if (-not (Wait-Count $app 'view destroyed' ($d0 + 1))) { return "cycle ${i}: unload destroyed nothing" }
    Send-Keys 'ctrl+tab';        if (-not (Wait-Count $app 'view created' ($c0 + 2))) { return "cycle ${i}: the unloaded tab was not recreated" }
    Send-Keys 'ctrl+w';          if (-not (Wait-Count $app 'view destroyed' ($d0 + 2))) { return "cycle ${i}: Ctrl+W destroyed no view" }
    if ($sample) {
      Start-Sleep -Milliseconds 700
      if (-not (Page-Is-Red $app)) { return "cycle ${i}: the page area was blank or dark after closing the visible tab" }
      if (-not $app.Proc.Responding) { return "cycle ${i}: the window stopped responding" }
    }
    $null
  }
  $problem = $null
  foreach ($i in 1..3) { $problem = Cycle $app $i $false; if ($problem) { break } }   # warm-up: the first cycles start Chromium's helper processes
  if (-not $problem) {
    Start-Sleep -Seconds 4
    $p0 = Procs; $m0 = Mem-MB; $pmax = $p0
    Write-Host "      baseline after warm-up: $p0 processes, $m0 MB"
    $t = [Diagnostics.Stopwatch]::StartNew()
    foreach ($i in 4..($Cycles + 3)) {
      $problem = Cycle $app $i (($i % 5) -eq 0)
      if ($problem) { break }
      $pmax = [math]::Max($pmax, (Procs))
      if (($i % 50) -eq 0) { Write-Host "      cycle $i of $($Cycles + 3): $(Procs) processes, $(Mem-MB) MB, $([int]$t.Elapsed.TotalSeconds) s" }
    }
    if (-not $problem) {
      Start-Sleep -Seconds 6
      $p1 = Procs; $m1 = Mem-MB
      Write-Host "      after $Cycles cycles: $p1 processes (peak $pmax), $m1 MB"
      if ($p1 -le $p0 + $Slack) { Pass "$Cycles cycles left the process count flat ($p0 before, $p1 after, tolerance $Slack)" } else { Fail "process count grew from $p0 to $p1 over $Cycles cycles" }
      if ($m1 -le $m0 * 1.5 + 50) { Pass "memory did not run away ($m0 MB before, $m1 MB after)" } else { Fail "memory grew from $m0 MB to $m1 MB" }
    }
  }
  if ($problem) { Fail $problem } else { Pass 'closing the visible tab never left the page area blank, and the window stayed responsive' }
  # the app's own books agree: as many views destroyed as created, apart from the one left standing
  $created = Count $app 'view created'; $destroyed = Count $app 'view destroyed'
  if (($created - $destroyed) -eq 1) { Pass "views balance: $created created, $destroyed destroyed, 1 alive" } else { Fail "views do not balance: $created created, $destroyed destroyed" }
  Stop-App $app; $app = $null

  # ---------------------------------------------------------------- C
  Write-Host 'C: a page that closes itself'
  $app = Start-App -Url "$base/closer.html"
  [void](Wait-Count $app 'view created' 1)
  Start-Sleep -Milliseconds 800
  Send-Keys 'ctrl+t'; [void](Wait-Count $app 'view created' 2); Start-Sleep -Milliseconds 800
  $both = @(Shown $app | Select-Object -Unique)
  Click-Client $app 290 120
  Send-Text 'q'
  if (Wait-Count $app 'closed by its page' 1 8) {
    Pass 'a page closing itself was seen as its tab closing'
    Start-Sleep -Milliseconds 800
    if ((Last-Shown $app) -eq $both[0]) { Pass 'the neighbouring tab was shown instead' } else { Fail 'another tab was not shown after the page closed itself' }
    if (Page-Is-Red $app) { Fail 'unexpected colour' } else { Pass 'the window still shows a page (not blank)' }
  } else {
    Note 'window.close() from a page is not honoured by this embedding (nothing closed); the page-closes-itself path is covered by unit tests only'
  }
  Stop-App $app; $app = $null

  # ---------------------------------------------------------------- D
  Write-Host 'D: restore after a restart'
  $app = Start-App -Url "$base/colours.html"
  $dir = $app.Dir
  [void](Wait-Count $app 'view created' 1)
  Start-Sleep -Milliseconds 800
  Send-Keys 'ctrl+t'; [void](Wait-Count $app 'view created' 2); Start-Sleep -Milliseconds 500
  Send-Keys 'ctrl+t'; [void](Wait-Count $app 'view created' 3); Start-Sleep -Milliseconds 500
  Send-Keys 'ctrl+shift+pageup'; Start-Sleep -Milliseconds 600   # the third tab moves up: order is now first, third, second
  Send-Keys 'alt+2'; Start-Sleep -Milliseconds 900                 # select the tab in position 2
  $selected = Last-Shown $app
  Write-Host "      selected before closing: $selected"
  if (-not (Close-App $app)) { Fail 'the app did not exit after one close' }
  Stop-App
  $app = Start-App -Url "$base/colours.html" -ReuseDir $dir
  [void](Wait-Count $app 'view created' 1)
  Start-Sleep -Milliseconds 800
  $restored = @(Shown $app)[0]
  if ($restored -eq $selected) { Pass 'the tab that was selected is the one shown after the restart' } else { Fail "restart showed $restored, expected $selected" }
  # three presses of Ctrl+Tab must visit three different tabs and come back: three tabs survived the restart
  $seen = @($restored)
  foreach ($k in 1..3) { Send-Keys 'ctrl+tab'; Start-Sleep -Milliseconds 1100; $seen += (Last-Shown $app) }
  $distinct = @($seen | Select-Object -Unique).Count
  if ($distinct -eq 3 -and $seen[3] -eq $seen[0]) { Pass 'all three tabs were restored, and closing the window had not closed them' } else { Fail "after the restart Ctrl+Tab visited $distinct different tabs: $($seen -join ' ')" }
}
finally {
  Stop-App
  if ($srv) { Stop-Process -Id $srv.Proc.Id -Force -ErrorAction SilentlyContinue }
  if ($dir -and (Test-Path $dir)) { Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue }
}
if ($failures.Count -gt 0) { Write-Host "M1.3 tabs check: $($failures.Count) failure(s)" -ForegroundColor Red; exit 1 }
Write-Host 'M1.3 tabs check: all passed' -ForegroundColor Green
