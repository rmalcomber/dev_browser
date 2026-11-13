# M1.6 checks on the real window (planning/m1-plan.md): the safety nets, and the M0 "destroy while risky" checks against the production code.
#   1  Risky pages: dialogs (alert, confirm, prompt), permission prompts (geolocation, notifications, camera), a page with a beforeunload
#      handler, and a page stuck in an endless loop. Each is closed (Ctrl+W) while it is doing its thing: the app must keep responding, the
#      view must be destroyed, the process count must not grow, and the dialogs/prompts must have been answered by the app (cancel, deny).
#   2  Popups: a popup without a user gesture is blocked; a popup from a click opens a tab in the same workspace.
#   3  Error pages: a refused connection shows the app's own page and Reload retries the failed address.
#   4  S12: a self-signed https site is refused with the app's own page, and no request ever reaches the server.
#   5  A crashed page (renderer processes killed): the app shows its crash page, the shell comes back, and browsing still works.
#   6  Downloads: held until the user answers; Save writes the file (safe name, not overwriting), Cancel writes nothing.
# Takes over the mouse and keyboard (about 3 minutes). Needs a build, node and openssl. Typing assumes a US keyboard layout. Exit code 0 = pass.
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
function Procs { @(Get-Process -Name 'dev_browser' -ErrorAction SilentlyContinue).Count }

$srv = $null; $app = $null; $https = $null
$dl = Join-Path $env:TEMP ('devb_dl_' + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory $dl | Out-Null
try {
  $srv = Start-TestServer
  $host1 = "127.0.0.1:$($srv.Port)"
  $app = Start-App -Url "http://$host1/colours.html" -ExtraArgs @("--downloads-dir=$dl")
  Start-Sleep -Milliseconds 800

  # ---- 1  risky pages, destroyed while risky
  Write-Host 'Closing risky pages while they are doing their thing'
  $baseline = Procs
  foreach ($kind in 'alert', 'confirm', 'prompt', 'geo', 'notify', 'camera', 'unload', 'busy') {
    $destroyed = Count $app 'view destroyed'
    Send-Keys 'ctrl+t'; Start-Sleep -Milliseconds 800
    Go $app "$host1/risky.html?kind=$kind"
    $marker = switch ($kind) { 'alert' { 'risky-alert-after' } 'confirm' { 'risky-confirm-answer=false' } 'prompt' { 'risky-prompt-answer=null' } 'geo' { 'risky-geo-(asked|denied)' } 'notify' { 'risky-notify-(asked|perm=)' } 'camera' { 'risky-camera-(asked|denied)' } 'unload' { 'risky-unload-armed' } 'busy' { 'risky-busy-busy' } }
    if (Wait-Count $app "page title: $marker" 1) { Pass "$kind page reached its risky step" } else { Fail "$kind page never got going" }
    Start-Sleep -Milliseconds 700
    Send-Keys 'ctrl+w'
    if (Wait-Count $app 'view destroyed' ($destroyed + 1) 12) { Pass "$kind page destroyed while risky" } else { Fail "$kind page's view was not destroyed" }
    if (-not $app.Proc.Responding) { Fail "the window stopped responding after $kind" }
  }
  if ((Count $app 'dialog from .* cancelled by the app') -ge 3) { Pass 'alert, confirm and prompt were each answered by the app (cancelled)' } else { Fail 'the dialogs were not all answered by the app' }
  if ((Count $app 'permission prompt from|camera/microphone request from') -ge 2) { Pass 'permission requests were denied by the app' } else { Fail 'permission requests were not all denied' }
  Start-Sleep -Seconds 6
  $after = Procs
  if ($after -le $baseline + 1) { Pass "process count did not grow ($baseline before, $after after)" } else { Fail "process count grew: $baseline -> $after" }

  # ---- 2  popups
  Write-Host 'Popups'
  $created = Count $app 'view created'
  Send-Keys 'ctrl+t'; Start-Sleep -Milliseconds 800
  Go $app "$host1/risky.html?kind=popup"
  Start-Sleep -Seconds 3   # Chromium's own popup blocker may stop it before the app is asked at all; either way no tab may appear
  if ((Count $app 'popup requested \(user gesture: False\)') -ge 1) { Pass 'the app was asked about a popup the page opened by itself, and refused it' } else { Write-Host '      (Chromium blocked the unrequested popup before the app was asked)' }
  if ((Count $app 'view created') -eq ($created + 1)) { Pass 'and it was blocked: no extra tab was made' } else { Fail 'a popup without a user gesture made a tab' }
  Go $app "$host1/risky.html?kind=popup-click"
  Start-Sleep -Seconds 2
  $created = Count $app 'view created'
  Click-Client $app 500 400
  if (Wait-Count $app 'popup requested \(user gesture: True\)' 1) { Pass 'a popup from a click was seen with a user gesture' } else { Fail 'the click popup was not seen' }
  if (Wait-Count $app 'view created' ($created + 1) 10) { Pass 'and it opened as a new tab in this workspace' } else { Fail 'the click popup did not open a tab' }
  Send-Keys 'ctrl+w'; Start-Sleep -Milliseconds 800

  # ---- 3  error page
  Write-Host 'Error pages'
  Go $app '127.0.0.1:1'
  if (Wait-Count $app 'page title: This page could not be loaded' 1) { Pass 'a refused connection shows the app''s own error page' } else { Fail 'no error page for a refused connection' }
  Start-Sleep -Milliseconds 800
  Send-Keys 'f5'
  if (Wait-Count $app 'reloading the address that failed' 1) { Pass 'Reload retries the address that failed' } else { Fail 'Reload did not retry the failed address' }

  # ---- 4  S12
  Write-Host 'S12: certificate errors'
  $key = Join-Path $dl 'k.pem'; $crt = Join-Path $dl 'c.pem'
  Start-Process openssl -ArgumentList 'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', $key, '-out', $crt, '-days', '1', '-subj', '/CN=localhost' -Wait -WindowStyle Hidden
  $hout = Join-Path $env:TEMP ('devb_https_' + [guid]::NewGuid().ToString('N').Substring(0, 8) + '.txt')
  $https = Start-Process node -ArgumentList (Join-Path $PSScriptRoot 'test-https-server.js'), '0', $key, $crt -RedirectStandardOutput $hout -PassThru -WindowStyle Hidden
  $hport = $null
  for ($i = 0; $i -lt 40 -and -not $hport; $i++) { Start-Sleep -Milliseconds 200; $l = Get-Content $hout -ErrorAction SilentlyContinue | Select-Object -First 1; if ($l -match 'listening (\d+)') { $hport = $Matches[1] } }
  if (-not $hport) { Fail 'the https test server did not start' }
  else {
    Go $app "https://127.0.0.1:$hport/"
    if (Wait-Count $app 'page title: This site''s certificate is not trusted' 1) { Pass 'a self-signed site shows the certificate error page' } else { Fail 'no certificate error page' }
    if ((Count $app 'certificate error .* the request is cancelled') -ge 1) { Pass 'the app cancelled the request itself (no bypass)' } else { Fail 'no certificate error was logged' }
    Send-Keys 'f5'; Start-Sleep -Seconds 3
    $reached = @(Get-Content $hout -ErrorAction SilentlyContinue | Where-Object { $_ -match '^request' }).Count
    if ($reached -eq 0) { Pass 'no request ever reached the server, even after a reload' } else { Fail "the server saw $reached request(s) although the certificate is not trusted" }
  }

  # ---- 6  downloads (before the crash test, which kills the renderers)
  Write-Host 'Downloads'
  Go $app "$host1/nav-a.html"
  Wait-Count $app 'page loaded.*nav-a.html' 1 | Out-Null
  Go $app "$host1/download.bin"
  if (Wait-Count $app 'download \d+ offered: report.txt' 1) { Pass 'a download is held until the user answers' } else { Fail 'the download was not offered' }
  Start-Sleep -Seconds 1
  if (-not (Test-Path (Join-Path $dl 'report.txt'))) { Pass 'nothing was written before the answer' } else { Fail 'a file appeared before the user answered' }
  Send-Keys 'ctrl+shift+s'
  $sw = [Diagnostics.Stopwatch]::StartNew(); while ($sw.Elapsed.TotalSeconds -lt $Wait -and -not (Test-Path (Join-Path $dl 'report.txt'))) { Start-Sleep -Milliseconds 200 }
  if ((Test-Path (Join-Path $dl 'report.txt')) -and ((Get-Content (Join-Path $dl 'report.txt') -Raw) -match 'downloaded content')) { Pass 'Save wrote the file into the downloads folder' } else { Fail 'Save did not write the file' }
  Go $app "$host1/download.bin"
  Wait-Count $app 'download \d+ offered' 2 | Out-Null
  Start-Sleep -Milliseconds 500
  Send-Keys 'ctrl+shift+x'; Start-Sleep -Seconds 2
  if (-not (Test-Path (Join-Path $dl 'report (2).txt'))) { Pass 'Cancel wrote nothing' } else { Fail 'a cancelled download was saved' }
  Go $app "$host1/download.bin"
  Wait-Count $app 'download \d+ offered' 3 | Out-Null
  Start-Sleep -Milliseconds 500
  Send-Keys 'ctrl+shift+s'
  $sw = [Diagnostics.Stopwatch]::StartNew(); while ($sw.Elapsed.TotalSeconds -lt $Wait -and -not (Test-Path (Join-Path $dl 'report (2).txt'))) { Start-Sleep -Milliseconds 200 }
  if (Test-Path (Join-Path $dl 'report (2).txt')) { Pass 'a second save did not overwrite the first (report (2).txt)' } else { Fail 'the second save overwrote or failed' }

  # ---- 5  crash
  Write-Host 'A crashed renderer'
  Go $app "$host1/nav-a.html"
  Wait-Count $app 'page loaded.*nav-a.html' 2 | Out-Null
  Start-Sleep -Seconds 1
  $victims = @(Get-CimInstance Win32_Process -Filter "Name='dev_browser.exe'" | Where-Object { $_.CommandLine -match '--type=renderer' })
  foreach ($v in $victims) { Stop-Process -Id $v.ProcessId -Force -ErrorAction SilentlyContinue }
  Write-Host "      killed $($victims.Count) renderer process(es)"
  if (Wait-Count $app 'showing an error page \(Crashed\)' 1) { Pass 'the crashed page shows the app''s crash page' } else { Fail 'no crash page' }
  if (Wait-Count $app 'reloading the shell \(attempt 1\)' 1) { Pass 'the crashed shell was reloaded' } else { Fail 'the shell was not reloaded' }
  Start-Sleep -Seconds 3
  $nav = Count $app 'page loaded.*nav-b.html'
  Go $app "$host1/nav-b.html"
  if (Wait-Count $app 'page loaded.*nav-b.html' ($nav + 1)) { Pass 'after the crash the shell works and browsing continues' } else { Fail 'browsing did not work after the crash' }
  if (-not $app.Proc.Responding) { Fail 'the window stopped responding' }
}
finally {
  Stop-App $app
  if ($srv) { Stop-Process -Id $srv.Proc.Id -Force -ErrorAction SilentlyContinue }
  if ($https) { Stop-Process -Id $https.Id -Force -ErrorAction SilentlyContinue }
  Remove-Item $dl -Recurse -Force -ErrorAction SilentlyContinue
}
if ((Get-FocusRestores) -gt 0) { Write-Host "note: the app window lost the foreground $(Get-FocusRestores) time(s) during the run and was brought back" -ForegroundColor Yellow }
if ($failures.Count -gt 0) { Write-Host "M1.6 safety check: $($failures.Count) failure(s)" -ForegroundColor Red; exit 1 }
Write-Host 'M1.6 safety check: all passed' -ForegroundColor Green
