# S6 check (security-acceptance-tests.md): an ordinary page must not get the app's privileged binding, and must not reach the shell.
#   Part A: a hostile http page looks for window.m0 and tries to fetch, XHR and frame app://shell. It must see nothing.
#   Part B: a real click on a link on that page to app://shell. Nothing may happen: the scheme is display-isolated, so a page cannot
#           navigate to it, and the shell never loads in a workspace profile. A second click on an ordinary link is the positive control.
#   Part C: the app is started with app://shell as the tab address. Core refuses it (only web addresses are stored or opened in a tab),
#           so the tab shows the blank page and the shell never appears in a page view.
# Part B takes over the mouse for a moment. Needs a built app (tools\build.ps1 build) and node. Throw-away data folder. Exit code 0 = pass.
param([int]$Wait = 30)
. "$PSScriptRoot\lib\app.ps1"
$failures = @()
function Fail($m) { $script:failures += $m; Write-Host "FAIL: $m" -ForegroundColor Red }
function Pass($m) { Write-Host "ok:   $m" -ForegroundColor Green }

$srv = $null; $app = $null
try {
  $srv = Start-TestServer
  Write-Host "Part A: hostile page on http://127.0.0.1:$($srv.Port)/hostile.html"
  $app = Start-App -Url "http://127.0.0.1:$($srv.Port)/hostile.html"
  $line = Wait-Log $app '\[host\] page title: \[s6\] ' $Wait
  if (-not $line) { Fail 'A: the hostile page never reported (did it load?)' }
  else {
    $r = ($line -replace '^.*\[s6\] ', '') | ConvertFrom-Json
    if ($r.m0Type -eq 'undefined') { Pass 'A: typeof window.m0 is undefined' } else { Fail "A: window.m0 is $($r.m0Type)" }
    if (@($r.m0Keys).Count -eq 0) { Pass 'A: no m0-like property on window' } else { Fail "A: found $($r.m0Keys -join ',')" }
    if ($r.topM0 -eq 'undefined') { Pass 'A: top.m0 is undefined' } else { Fail "A: top.m0 is $($r.topM0)" }
    foreach ($k in 'fetch', 'xhr', 'iframe') { if ($r.$k -eq 'blocked') { Pass "A: $k to app://shell blocked" } else { Fail "A: $k to app://shell gave: $($r.$k)" } }
  }

  Write-Host 'Part B: a real click on a link to app://shell'
  $before = @(Get-Content $app.Out, $app.Err -ErrorAction SilentlyContinue | Where-Object { $_ -like '*[[]host] page loaded*' -or $_ -like '*[[]host] page failed to load*' }).Count
  if ($before -lt 1) { Fail 'B: the hostile page has no load line yet, so the comparison below would prove nothing' }
  Click-Client $app 290 318   # the page starts 220 px from the left; the link is 20 px in and 300 px down
  Start-Sleep -Seconds 3
  $lines = @(Get-Content $app.Out, $app.Err -ErrorAction SilentlyContinue)
  if ($lines | Where-Object { $_ -like '*page loaded*app://shell*' }) { Fail 'B: the shell page loaded inside a page view' } else { Pass 'B: the shell page never loaded inside a page view' }
  $after = @($lines | Where-Object { $_ -like '*[[]host] page loaded*' -or $_ -like '*[[]host] page failed to load*' }).Count
  if ($after -eq $before) { Pass 'B: the click on the link to app://shell did nothing (the scheme is display-isolated: no page can navigate to it)' }
  else { Fail 'B: the click on the link to app://shell started a navigation' }
  # the control: the same kind of click on an ordinary link must be followed, so a miss above cannot pass for a refusal
  Click-Client $app 290 398
  if (Wait-Log $app 'page loaded.*colours.html' 10) { Pass 'B: a click on an ordinary link was followed (the clicks do land)' } else { Fail 'B: the control click on an ordinary link did nothing, so the result above proves nothing' }
  Stop-App $app; $app = $null

  Write-Host 'Part C: the start address is app://shell/index.html'
  $app = Start-App -Url 'app://shell/index.html'
  Start-Sleep -Seconds 2
  $blank = Get-Content $app.Out, $app.Err -ErrorAction SilentlyContinue | Where-Object { $_ -like '*page loaded*about:blank*' }
  if ($blank) { Pass 'C: a tab was never opened on app://shell: it got the blank page' } else { Fail 'C: the tab did not get the blank page' }
  $loaded = Get-Content $app.Out -ErrorAction SilentlyContinue | Where-Object { $_ -like '*page loaded*app://shell*' }
  if ($loaded) { Fail 'C: the shell page loaded inside a page view' } else { Pass 'C: the shell page never loaded inside a page view' }
}
finally {
  Stop-App
  if ($srv) { Stop-Process -Id $srv.Proc.Id -Force -ErrorAction SilentlyContinue }
}
if ($failures.Count -gt 0) { Write-Host "S6 hostile-page check: $($failures.Count) failure(s)" -ForegroundColor Red; exit 1 }
Write-Host 'S6 hostile-page check: all passed' -ForegroundColor Green
