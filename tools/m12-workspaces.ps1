# M1.2 exit scenario, automated (planning/m1-plan.md): workspaces are separate profiles, made and deleted through the real shell UI.
#   Phase 1 (fresh data): the first workspace loads a site that stores a login-like token. A second workspace is made by typing into
#           the sidebar (real clicks and keys) and loads the SAME site: it must not see the first workspace's token (separate login).
#           Switching back works. The second workspace is deleted through the UI: its profile folder and database folder must be gone.
#   Phase 2 (restart on the same data): the first workspace must still have its own token (profiles persist), and the deleted workspace
#           must not come back.
# The sidebar's layout is fixed (ui/src/style.css), so the clicks use fixed client positions; if the layout changes, update them here.
# Takes over the mouse and keyboard for about a minute. Needs a build (tools\build.ps1 build) and node. Exit code 0 = pass.
param([int]$Wait = 30)
. "$PSScriptRoot\lib\app.ps1"
$failures = @()
function Fail($m) { $script:failures += $m; Write-Host "FAIL: $m" -ForegroundColor Red }
function Pass($m) { Write-Host "ok:   $m" -ForegroundColor Green }

# the distinct identity tokens the page reported so far, in order (the page logs each title more than once)
function Get-Tokens($app) {
  $seen = @()
  foreach ($l in (Get-Content $app.Out -ErrorAction SilentlyContinue)) {
    if ($l -match '\[id\] had=(\S+)/(\S+) now=(\S+)') { if (-not ($seen | Where-Object { $_.Now -eq $Matches[3] })) { $seen += [pscustomobject]@{ Cookie = $Matches[1]; Local = $Matches[2]; Now = $Matches[3] } } }
  }
  $seen
}
function Wait-Tokens($app, [int]$count, [int]$seconds) {
  $sw = [Diagnostics.Stopwatch]::StartNew()
  while ($sw.Elapsed.TotalSeconds -lt $seconds) { $t = @(Get-Tokens $app); if ($t.Count -ge $count) { return $t }; Start-Sleep -Milliseconds 300 }
  @(Get-Tokens $app)
}
function Count-Log($app, [string]$pattern) { @(Get-Content $app.Out -ErrorAction SilentlyContinue | Where-Object { $_ -match $pattern }).Count }
function Ws-Folders($dir) {
  @{ Chromium = @(Get-ChildItem (Join-Path $dir 'data\chromium-root') -Directory -ErrorAction SilentlyContinue | Where-Object { $_.Name -like 'ws-*' }).Count
     Own = @(Get-ChildItem (Join-Path $dir 'data\workspaces') -Directory -ErrorAction SilentlyContinue | Where-Object { $_.Name -like 'ws-*' }).Count }
}

$srv = $null; $app = $null; $dir = $null
try {
  $srv = Start-TestServer
  $url = "http://127.0.0.1:$($srv.Port)/identity.html"

  Write-Host 'Phase 1: fresh data'
  $app = Start-App -Url $url
  $dir = $app.Dir
  $t = @(Wait-Tokens $app 1 $Wait)
  if ($t.Count -lt 1) { throw 'the first workspace never reported its identity' }
  $first = $t[0]
  if ($first.Cookie -eq 'none' -and $first.Local -eq 'none') { Pass "first workspace is new to the site (token $($first.Now))" } else { Fail 'the first workspace already had a token on a fresh data folder' }

  # make a second workspace through the sidebar: "+ New workspace", type a name, Enter
  Click-Client $app 110 92; Start-Sleep -Milliseconds 500
  Send-Text 'Second'; Send-Keys 'enter'
  $t = @(Wait-Tokens $app 2 $Wait)
  if ($t.Count -lt 2) { Fail 'the second workspace never loaded the site' }
  else {
    $second = $t[1]
    if ($second.Cookie -eq 'none' -and $second.Local -eq 'none') { Pass 'the second workspace did not see the first one''s cookie or localStorage on the same site' } else { Fail "the second workspace saw the first one's login: $($second.Cookie)/$($second.Local)" }
    if ($second.Now -ne $first.Now) { Pass 'each workspace made its own token' } else { Fail 'both workspaces ended up with the same token' }
  }
  $f = Ws-Folders $dir
  if ($f.Chromium -eq 2 -and $f.Own -eq 2) { Pass 'each workspace has its own profile folder and database folder (2 + 2)' } else { Fail "expected 2 profile and 2 database folders, found $($f.Chromium) and $($f.Own)" }

  # switch back to the first workspace (click it) and make sure the app showed it again
  $shownBefore = Count-Log $app 'tab tab-\S+: shown \(workspace ws-'
  Click-Client $app 60 53; Start-Sleep -Milliseconds 1200
  if ((Count-Log $app 'tab tab-\S+: shown \(workspace ws-') -gt $shownBefore) { Pass 'clicking the first workspace switched back to it' } else { Fail 'clicking the first workspace did nothing' }
  Click-Client $app 60 86; Start-Sleep -Milliseconds 1200   # and to the second, which is the one to delete

  # delete the second (active) workspace through the UI: Delete, then confirm
  Click-Client $app 170 173; Start-Sleep -Milliseconds 600
  Click-Client $app 37 236
  if (Wait-Log $app 'workspace ws-\S+: deleting' 10) { Pass 'the app started deleting the workspace' } else { Fail 'no delete reached the app' }
  $removed = Wait-Log $app 'workspace ws-\S+: data removed' $Wait
  if ($removed) { Pass 'the deleted workspace''s data was removed while the app was running' } else { Write-Host 'note: data was not removable while running (Chromium holds the profile open); it is removed when the app exits' -ForegroundColor Yellow }
  if (-not (Close-App $app)) { Fail 'the app did not exit after one close' } else { Pass 'one click on X closed the app' }
  Stop-App
  # Chromium holds the deleted profile open for the rest of the run, so it goes when the app exits (swept after CEF shuts down)
  $f = Ws-Folders $dir
  if ($f.Chromium -eq 1 -and $f.Own -eq 1) { Pass 'when the app exited only the first workspace''s folders remained (1 + 1): the deleted data is gone' } else { Fail "expected 1 + 1 folders once the app had exited, found $($f.Chromium) and $($f.Own)" }

  Write-Host 'Phase 2: restart on the same data'
  $app = Start-App -Url $url -ReuseDir $dir
  $t = @(Wait-Tokens $app 1 $Wait)
  if ($t.Count -lt 1) { Fail 'the first workspace never reported after the restart' }
  elseif ($t[0].Cookie -eq $first.Now -and $t[0].Local -eq $first.Now) { Pass 'after a restart the first workspace still has its own login (cookie and localStorage)' }
  else { Fail "after a restart the first workspace had $($t[0].Cookie)/$($t[0].Local), expected $($first.Now)" }
  if (Count-Log $app 'removed leftover data') { Fail 'the exit sweep missed the deleted workspace: the start had to remove its data' }
  $f = Ws-Folders $dir
  if ($f.Chromium -eq 1 -and $f.Own -eq 1) { Pass 'after the restart only the first workspace''s folders exist (1 + 1)' } else { Fail "expected 1 + 1 folders after the restart, found $($f.Chromium) and $($f.Own)" }
  if (@(Get-Tokens $app).Count -ne 1) { Fail 'the deleted workspace came back (a second page loaded)' } else { Pass 'the deleted workspace did not come back' }
}
finally {
  Stop-App
  if ($srv) { Stop-Process -Id $srv.Proc.Id -Force -ErrorAction SilentlyContinue }
  if ($dir -and (Test-Path $dir)) { Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue }
}
if ($failures.Count -gt 0) { Write-Host "M1.2 workspaces check: $($failures.Count) failure(s)" -ForegroundColor Red; exit 1 }
Write-Host 'M1.2 workspaces check: all passed' -ForegroundColor Green
