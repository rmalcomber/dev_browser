# S6 check (security-acceptance-tests.md): an ordinary page must not get the app's privileged binding, and must not reach the shell.
#   Part A: a hostile http page looks for window.m0 and tries to fetch, XHR and frame app://shell. It must see nothing.
#   Part B: a page view is pointed straight at app://shell. Page views use a workspace request context, where the app scheme is not
#           registered, so the load must fail with ERR_UNKNOWN_URL_SCHEME. (The browser-side gate, ipc::trusted, is the second line
#           of defence and is covered by unit tests; this check guards the first.)
# Needs a built app (tools\build.ps1 build) and node. Uses a throw-away data folder. Exit code 0 = both parts pass.
param([int]$Wait = 30)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$exe = Join-Path $root 'target\debug\dev_browser.exe'
if (-not (Test-Path $exe)) { throw "No $exe. Run tools\build.ps1 build first." }
$tmp = Join-Path $env:TEMP ("s6_" + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory $tmp | Out-Null
$failures = @()
function Fail($m) { $script:failures += $m; Write-Host "FAIL: $m" -ForegroundColor Red }
function Pass($m) { Write-Host "ok:   $m" -ForegroundColor Green }
function Stop-App { Get-Process dev_browser -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue; Start-Sleep -Milliseconds 500 }

function Start-App([string]$name, [string]$url) {
  $o = Join-Path $tmp "$name.out"; $e = Join-Path $tmp "$name.err"
  $p = Start-Process $exe -ArgumentList "--data-dir=$(Join-Path $tmp $name)", "--url=$url" -RedirectStandardOutput $o -RedirectStandardError $e -PassThru
  return @{ Proc = $p; Out = $o; Err = $e }
}
function Wait-Log($app, [string]$pattern, [int]$seconds) {
  $sw = [Diagnostics.Stopwatch]::StartNew()
  while ($sw.Elapsed.TotalSeconds -lt $seconds) {
    $hit = Get-Content $app.Out, $app.Err -ErrorAction SilentlyContinue | Where-Object { $_ -match $pattern } | Select-Object -First 1
    if ($hit) { return $hit }
    Start-Sleep -Milliseconds 300
  }
  return $null
}

$server = $null
try {
  Stop-App
  # ---------- Part A ----------
  $server = Start-Process node -ArgumentList (Join-Path $PSScriptRoot 'test-server.js'), '0' -RedirectStandardOutput (Join-Path $tmp 'server.out') -PassThru -WindowStyle Hidden
  $port = $null
  for ($i = 0; $i -lt 30 -and -not $port; $i++) { Start-Sleep -Milliseconds 200; $l = Get-Content (Join-Path $tmp 'server.out') -ErrorAction SilentlyContinue | Select-Object -First 1; if ($l -match 'listening (\d+)') { $port = $Matches[1] } }
  if (-not $port) { throw 'test server did not start' }
  Write-Host "Part A: hostile page on http://127.0.0.1:$port/hostile.html"
  $app = Start-App 'a' "http://127.0.0.1:$port/hostile.html"
  $line = Wait-Log $app '\[host\] page title: \[s6\] ' $Wait
  if (-not $line) { Fail 'A: the hostile page never reported (did it load?)' }
  else {
    $r = ($line -replace '^.*\[s6\] ', '') | ConvertFrom-Json
    if ($r.m0Type -eq 'undefined') { Pass 'A: typeof window.m0 is undefined' } else { Fail "A: window.m0 is $($r.m0Type)" }
    if (@($r.m0Keys).Count -eq 0) { Pass 'A: no m0-like property on window' } else { Fail "A: found $($r.m0Keys -join ',')" }
    if ($r.topM0 -eq 'undefined') { Pass 'A: top.m0 is undefined' } else { Fail "A: top.m0 is $($r.topM0)" }
    foreach ($k in 'fetch', 'xhr', 'iframe') { if ($r.$k -eq 'blocked') { Pass "A: $k to app://shell blocked" } else { Fail "A: $k to app://shell gave: $($r.$k)" } }
  }
  Stop-App
  Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue; $server = $null

  # ---------- Part B ----------
  Write-Host 'Part B: a page view is pointed straight at app://shell/index.html'
  $app = Start-App 'b' 'app://shell/index.html'
  $fail = Wait-Log $app 'page failed to load.*app://shell/index.html.*ERR_UNKNOWN_URL_SCHEME' $Wait
  if ($fail) { Pass 'B: a workspace page cannot load app://shell (unknown scheme in its request context)' }
  else { Fail 'B: the page view did not fail with ERR_UNKNOWN_URL_SCHEME: it may have loaded the shell. Look at the log.' }
  # If it ever did load, the page view would get window.m0; the browser gate (ipc::trusted) would then be the only defence.
  $loaded = Get-Content $app.Out -ErrorAction SilentlyContinue | Where-Object { $_ -like '*page loaded (status 200): app://shell*' }
  if ($loaded) { Fail 'B: the shell page loaded inside a page view' } else { Pass 'B: the shell page never loaded inside a page view' }
}
finally {
  Stop-App
  if ($server) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
  Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
if ($failures.Count -gt 0) { Write-Host "S6 hostile-page check: $($failures.Count) failure(s)" -ForegroundColor Red; exit 1 }
Write-Host 'S6 hostile-page check: all passed' -ForegroundColor Green
