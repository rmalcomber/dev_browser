# M1.5 checks on the real window (planning/m1-plan.md): navigation chrome, normal browsing in two workspaces, and S10.
#   1  Typing an address in the address bar (Ctrl+L, type, Enter) loads it; an IP and port gets http://.
#   2  The page's icon is fetched, and a link click, Back (Alt+Left), Forward (Alt+Right) and Reload (F5) work.
#   3  Text that is not an address is sent to the search engine; an address the app cannot open (javascript:) loads nothing.
#   4  Stop (Ctrl+.) cancels a slow load; the same load, left alone, finishes (the control).
#   5  Browsing works in a second workspace, too.
#   6  S10: "open this page in <other workspace>" loads the page in the DESTINATION's own profile, never with the source's cookies.
#      Workspace one has a login-like token (cookie and localStorage) from identity.html. Opening that page in workspace two must not see
#      it; opening workspace two's page in workspace one must see workspace one's own token and not workspace two's.
# The "open in" buttons sit in the sidebar footer, a fixed distance from the bottom edge (ui\src\style.css). Takes over the mouse and keyboard
# (about 2 minutes). Needs a build and node. Typing assumes a US keyboard layout. Exit code 0 = pass.
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
# type into the address bar: Ctrl+L focuses it, then the text and Enter
function Go($app, [string]$text) { Send-Keys 'ctrl+l'; Start-Sleep -Milliseconds 700; Send-Text $text; Send-Keys 'enter' }
# the identity page's reports, in order, one per load (it logs each title twice in a row)
function Identities($app) {
  $out = @(); $prev = ''
  foreach ($l in (Log $app)) { if ($l -match '\[id\] had=(\S+)/(\S+) now=(\S+)') { if ($Matches[0] -ne $prev) { $out += [pscustomobject]@{ Cookie = $Matches[1]; Local = $Matches[2]; Now = $Matches[3] }; $prev = $Matches[0] } } }
  $out
}

$srv = $null; $app = $null
try {
  $srv = Start-TestServer
  $host1 = "127.0.0.1:$($srv.Port)"
  $app = Start-App -Url "http://$host1/colours.html"
  Start-Sleep -Milliseconds 800

  # ---- 1 and 2
  Write-Host 'Navigation in the first workspace'
  Go $app "$host1/nav-a.html"
  if (Wait-Count $app 'page loaded.*http://127.0.0.1:\d+/nav-a.html' 1) { Pass 'typing an address loaded it (an IP and port was given http://)' } else { Fail 'the typed address did not load' }
  if (Wait-Count $app 'favicon ready' 1) { Pass 'the page icon was fetched in the tab''s own browser and handed to the app' } else { Fail 'no favicon was fetched' }
  Start-Sleep -Milliseconds 500
  Click-Client $app 290 118   # the link "go to page B": 20 px in and 100 px down in the page, which starts 220 px from the left
  if (Wait-Count $app 'page loaded.*nav-b.html' 1) { Pass 'a click on a link navigated' } else { Fail 'the link click did not navigate' }
  Send-Keys 'alt+left'
  if (Wait-Count $app 'page loaded.*nav-a.html' 2) { Pass 'Back (Alt+Left) returned to the first page' } else { Fail 'Back did nothing' }
  Send-Keys 'alt+right'
  if (Wait-Count $app 'page loaded.*nav-b.html' 2) { Pass 'Forward (Alt+Right) went to the second page again' } else { Fail 'Forward did nothing' }
  Send-Keys 'f5'
  if (Wait-Count $app 'page loaded.*nav-b.html' 3) { Pass 'Reload (F5) reloaded the page' } else { Fail 'Reload did nothing' }

  # ---- 3
  $nav = Count $app 'navigating'
  Go $app 'hello world'
  if (Wait-Count $app 'page (loaded|failed to load).*duckduckgo.com/\?q=hello(%20|\+)world' 1) { Pass 'text that is not an address went to the search engine (loaded, or failed to load if offline)' } else { Fail 'the search was not sent to the search engine' }
  $nav = Count $app 'navigating'
  Go $app 'javascript:alert(1)'
  Start-Sleep -Seconds 2
  if ((Count $app 'navigating') -eq $nav) { Pass 'javascript: was refused: nothing was loaded' } else { Fail 'an address with the javascript: scheme was loaded' }

  # ---- 4
  Write-Host 'Loading and stopping'
  $slow = "$host1/slow.html?delay=7000"
  Go $app $slow
  Start-Sleep -Milliseconds 1500
  Send-Keys 'ctrl+.'
  Start-Sleep -Seconds 9
  if ((Count $app 'page loaded.*slow.html') -eq 0) { Pass 'Stop (Ctrl+.) cancelled a slow load: it never finished' } else { Fail 'the slow page loaded although it was stopped' }
  Go $app $slow
  if (Wait-Count $app 'page loaded.*slow.html' 1 20) { Pass 'the same slow load, left alone, finished (so the stop above was real)' } else { Fail 'the control load never finished' }

  # ---- 5 and 6
  Write-Host 'A second workspace, and S10'
  Go $app "$host1/identity.html"
  if (Wait-Count $app '\[id\] had=none/none' 1) { Pass 'workspace one set its own token on the identity page' } else { Fail 'the identity page did not report in workspace one' }
  Start-Sleep -Milliseconds 800
  Send-Keys 'alt+n'; Start-Sleep -Milliseconds 800; Send-Text 'Second'; Send-Keys 'enter'
  Start-Sleep -Milliseconds 2500
  Go $app "$host1/nav-a.html"
  if (Wait-Count $app 'page loaded.*nav-a.html' 3) { Pass 'normal browsing works in the second workspace too' } else { Fail 'the second workspace could not browse' }
  Start-Sleep -Milliseconds 800
  Send-Keys 'alt+pageup'; Start-Sleep -Milliseconds 1500    # back to workspace one, showing its identity page
  $c = Get-Client $app
  $before = @(Identities $app).Count
  Click-Client $app 47 ($c.H - 58)                            # "-> Second": open this page in the other workspace
  Start-Sleep -Milliseconds 500
  $ok = $false
  $sw = [Diagnostics.Stopwatch]::StartNew(); while ($sw.Elapsed.TotalSeconds -lt $Wait) { if (@(Identities $app).Count -gt $before) { $ok = $true; break }; Start-Sleep -Milliseconds 150 }
  $ids = @(Identities $app)
  if (-not $ok) { Fail 'S10: the page was not opened in the other workspace' }
  else {
    $one = $ids[0]; $inTwo = $ids[-1]
    if ($inTwo.Cookie -eq 'none' -and $inTwo.Local -eq 'none') { Pass 'S10: the page opened in workspace two did NOT see workspace one''s cookie or localStorage' } else { Fail "S10: workspace two saw $($inTwo.Cookie)/$($inTwo.Local) from workspace one" }
    if ($inTwo.Now -ne $one.Now) { Pass 'S10: workspace two made its own token' } else { Fail 'S10: both workspaces ended up with the same token' }
    # and the other way: from workspace two (now active) open its identity page in workspace one
    Start-Sleep -Milliseconds 1200
    $before = @(Identities $app).Count
    Click-Client $app 47 ($c.H - 58)                          # "-> Default"
    $ok2 = $false
    $sw = [Diagnostics.Stopwatch]::StartNew(); while ($sw.Elapsed.TotalSeconds -lt $Wait) { if (@(Identities $app).Count -gt $before) { $ok2 = $true; break }; Start-Sleep -Milliseconds 150 }
    $ids = @(Identities $app)
    if (-not $ok2) { Fail 'S10: the page was not opened back in workspace one' }
    else {
      $back = $ids[-1]
      if ($back.Cookie -eq $one.Now -and $back.Local -eq $one.Now) { Pass 'S10: opened in workspace one, the page saw workspace one''s OWN token (its own profile), not workspace two''s' } else { Fail "S10: workspace one saw $($back.Cookie)/$($back.Local), expected its own $($one.Now)" }
      if ($back.Cookie -ne $inTwo.Now) { Pass 'S10: and not the token workspace two had just made' } else { Fail 'S10: workspace one saw workspace two''s token' }
    }
  }
  if (-not $app.Proc.Responding) { Fail 'the window stopped responding' }
}
finally {
  Stop-App $app
  if ($srv) { Stop-Process -Id $srv.Proc.Id -Force -ErrorAction SilentlyContinue }
}
if ((Get-FocusRestores) -gt 0) { Write-Host "note: the app window lost the foreground $(Get-FocusRestores) time(s) during the run and was brought back" -ForegroundColor Yellow }
if ($failures.Count -gt 0) { Write-Host "M1.5 navigation check: $($failures.Count) failure(s)" -ForegroundColor Red; exit 1 }
Write-Host 'M1.5 navigation check: all passed' -ForegroundColor Green
