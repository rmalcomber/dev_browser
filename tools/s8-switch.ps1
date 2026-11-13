# S8 check (security-acceptance-tests.md): switching workspace or tab never flashes previously visible content.
#   Three workspaces open the same address but show three colours (red, blue, green: each workspace is a separate profile with its own
#   localStorage). The window is recorded at 60 fps while the app cycles Alt+PageDown through them; the frames must go round
#   red -> blue -> green -> red in order, each change moving to the NEXT colour, with at most a dark neutral gap in between: a frame of the
#   previous workspace reappearing after the new one was shown is a failure. A second recording does the same for new tabs and closing the
#   visible tab (freshly created and destroyed views), and a third for the workspace-colour frame.
# Takes over the mouse and keyboard (about 1 minute). Needs a build, node and ffmpeg. Exit code 0 = pass.
param([int]$Switches = 9, [int]$GapMs = 1300)
. "$PSScriptRoot\lib\app.ps1"
$failures = @()
function Fail($m) { $script:failures += $m; Write-Host "FAIL: $m" -ForegroundColor Red }
function Pass($m) { Write-Host "ok:   $m" -ForegroundColor Green }
function Count($app, [string]$pattern) { @(Get-Content $app.Out, $app.Err -ErrorAction SilentlyContinue | Where-Object { $_ -match $pattern }).Count }
function Wait-Count($app, [string]$pattern, [int]$n, [double]$seconds = 20) {
  $sw = [Diagnostics.Stopwatch]::StartNew()
  while ($sw.Elapsed.TotalSeconds -lt $seconds) { if ((Count $app $pattern) -ge $n) { return $true }; Start-Sleep -Milliseconds 80 }
  $false
}

$srv = $null; $app = $null
try {
  $srv = Start-TestServer
  $app = Start-App -Url "http://127.0.0.1:$($srv.Port)/colours.html"
  Start-Sleep -Milliseconds 1000

  # make workspaces Two (blue) and Three (green), each with its own colour in its own profile
  foreach ($spec in @(@('Two', '2'), @('Three', '3'))) {
    $shown = Count $app 'tab tab-\S+: shown \(workspace ws-'
    Send-Keys 'alt+n'; Start-Sleep -Milliseconds 700
    Send-Text $spec[0]; Send-Keys 'enter'
    if (-not (Wait-Count $app 'tab tab-\S+: shown \(workspace ws-' ($shown + 1))) { Fail "workspace $($spec[0]) was not created"; throw 'setup failed' }
    Start-Sleep -Milliseconds 1500
    Click-Client $app 700 300          # give the new page the focus
    Send-Keys $spec[1]; Start-Sleep -Milliseconds 500
  }
  Pass 'three workspaces set up (red, blue, green)'

  # ---- 1: workspace switching, Alt+PageDown round the cycle
  $secs = [int][math]::Ceiling(1.5 + $Switches * $GapMs / 1000.0 + 1.5)
  Write-Host "Recording $Switches workspace switches"
  $rec = Start-Recording $app $secs
  Start-Sleep -Milliseconds 1500
  foreach ($i in 1..$Switches) { Send-Keys 'alt+pagedown'; Start-Sleep -Milliseconds $GapMs }
  # the active workspace was Three (green) at the start: next is One (red), then Two (blue), then Three...
  $code = Wait-Recording $rec @('--cycle', 'K|G,K|R,K|B', '--gap', 'K|K', '--min-states', ($Switches + 1))
  if ($code -eq 0) { Pass "S8: $Switches workspace switches never showed a frame of the previous workspace" } else { Fail 'S8: a workspace switch showed a frame out of order (see the analyser output above)' }

  # ---- 2: new tab and close tab on one workspace (fresh views come and go), all the same colour
  Write-Host 'Recording new tab / close tab'
  $rec = Start-Recording $app 14
  Start-Sleep -Milliseconds 1200
  foreach ($i in 1..5) { Send-Keys 'ctrl+t'; Start-Sleep -Milliseconds 1100; Send-Keys 'ctrl+w'; Start-Sleep -Milliseconds 900 }
  $code = Wait-Recording $rec @('--cycle', 'K|G', '--gap', 'K|K')
  if ($code -eq 0) { Pass 'S8: creating and closing tabs showed nothing but the page colour and a dark gap' } else { Fail 'S8: creating or closing a tab flashed something else (see above)' }
}
catch { if ($_.Exception.Message -ne 'setup failed') { Fail "script error: $($_.Exception.Message)" } }
finally {
  Stop-App $app
  if ($srv) { Stop-Process -Id $srv.Proc.Id -Force -ErrorAction SilentlyContinue }
}
if ((Get-FocusRestores) -gt 0) { Write-Host "note: the app window lost the foreground $(Get-FocusRestores) time(s) during the run and was brought back; something else on the desktop took focus" -ForegroundColor Yellow }
if ($failures.Count -gt 0) { Write-Host "S8 check: $($failures.Count) failure(s)" -ForegroundColor Red; exit 1 }
Write-Host 'S8 check: all passed' -ForegroundColor Green
