# M1.4 checks on the real window (planning/m1-plan.md): the sidebar tree and the workspace accent frame.
#   - every page is framed in its workspace's colour (a 3 px frame around the page; sampled from a screenshot), and the frame follows the
#     workspace when you switch, and changes live when the workspace is recoloured;
#   - the chevron expands a workspace WITHOUT switching to it; clicking a tab of an expanded workspace switches to that workspace;
#   - group and ungroup shortcuts run.
# (The S8 no-flash check is tools\s8-switch.ps1; S11, collapsed workspaces showing only a name and an icon, is a UI test in
# ui\src\Sidebar.test.tsx and a core test in crates\core\src\backend.rs.)
# The sidebar's rows sit at fixed positions from the top (below the navigation bar: workspace row 1 at y 119) and its footer at fixed distances from the bottom edge
# (ui\src\style.css); if the layout changes, update the numbers below. Takes over the mouse and keyboard (about 40 seconds).
. "$PSScriptRoot\lib\app.ps1"
$failures = @()
function Fail($m) { $script:failures += $m; Write-Host "FAIL: $m" -ForegroundColor Red }
function Pass($m) { Write-Host "ok:   $m" -ForegroundColor Green }
function Shown($app) { @(Get-Content $app.Out, $app.Err -ErrorAction SilentlyContinue | ForEach-Object { if ($_ -match '\[host\] tab (tab-\S+): shown \(workspace (ws-\S+)\)') { [pscustomobject]@{ Tab = $Matches[1]; Ws = $Matches[2] } } }) }
# the colour of the frame to the left of the page (3 px wide, starting 220 px in), as r,g,b
function Frame-Colour($app) {
  $f = Join-Path $env:TEMP ('m14_px_' + [guid]::NewGuid().ToString('N').Substring(0, 6) + '.png')
  Save-Window $app $f -ClientOnly
  $b = New-Object System.Drawing.Bitmap $f; $p = $b.GetPixel(221, 300); $b.Dispose(); Remove-Item $f -ErrorAction SilentlyContinue
  @($p.R, $p.G, $p.B)
}
function Near($a, $b) { ([math]::Abs($a[0] - $b[0]) -le 3) -and ([math]::Abs($a[1] - $b[1]) -le 3) -and ([math]::Abs($a[2] - $b[2]) -le 3) }
function Wait-Frame($app, $want, [double]$seconds = 5) {
  $sw = [Diagnostics.Stopwatch]::StartNew(); $got = $null
  while ($sw.Elapsed.TotalSeconds -lt $seconds) { $got = Frame-Colour $app; if (Near $got $want) { return $true }; Start-Sleep -Milliseconds 200 }
  Write-Host "      last frame colour seen: $($got -join ',') (wanted $($want -join ','))"
  $false
}
$BLUE = @(0x2C, 0x5F, 0xA8); $PURPLE = @(0xA7, 0x8B, 0xFA); $ORANGE = @(0xF4, 0xA2, 0x61)

$srv = $null; $app = $null
try {
  $srv = Start-TestServer
  $app = Start-App -Url "http://127.0.0.1:$($srv.Port)/colours.html"
  Start-Sleep -Milliseconds 800
  $first = (Shown $app)[0]
  if (Wait-Frame $app $BLUE) { Pass 'the first workspace page is framed in its colour (blue)' } else { Fail 'the first page has no blue frame' }

  Send-Keys 'alt+n'; Start-Sleep -Milliseconds 800; Send-Text 'Second'; Send-Keys 'enter'; Start-Sleep -Milliseconds 2500
  if (Wait-Frame $app $PURPLE) { Pass 'a new workspace got the next palette colour and its page is framed in it (purple)' } else { Fail 'the new workspace page has no purple frame' }

  # the chevron of the first workspace (row 1, y 119): expands it without switching
  $before = (Shown $app).Count
  Click-Client $app 20 119; Start-Sleep -Milliseconds 1000
  if ((Shown $app).Count -eq $before) { Pass 'the chevron expanded a workspace without switching to it (no page was shown)' } else { Fail 'clicking the chevron switched workspace' }
  if (Near (Frame-Colour $app) $PURPLE) { Pass 'and the displayed page is still the second workspace one' } else { Fail 'the displayed page changed when only a chevron was clicked' }

  # its tab is now listed under it (y 147): clicking it switches to that workspace
  Click-Client $app 100 147; Start-Sleep -Milliseconds 1200
  $last = (Shown $app)[-1]
  if ($last.Ws -eq $first.Ws -and $last.Tab -eq $first.Tab) { Pass 'clicking a tab listed under another workspace switched to that workspace and tab' } else { Fail "clicking the tab showed $($last.Ws), expected $($first.Ws)" }
  if (Wait-Frame $app $BLUE) { Pass 'the frame followed: blue again' } else { Fail 'the frame did not follow the switch back to the first workspace' }

  # recolour the active workspace: footer Colour, then the orange swatch (4th from the left, 54 px above the bottom)
  $c = Get-Client $app
  Click-Client $app 108 ($c.H - 23); Start-Sleep -Milliseconds 600
  Click-Client $app 64 ($c.H - 54); Start-Sleep -Milliseconds 800
  if (Wait-Frame $app $ORANGE) { Pass 'recolouring the workspace changed the frame of its open page at once (orange)' } else { Fail 'the frame did not change when the workspace was recoloured' }

  # group and ungroup shortcuts (the group logic is covered by core tests; this proves they are wired)
  Send-Keys 'ctrl+shift+g'; Start-Sleep -Milliseconds 600
  Send-Keys 'ctrl+shift+u'; Start-Sleep -Milliseconds 600
  $log = Get-Content $app.Out, $app.Err -ErrorAction SilentlyContinue
  if (($log -match 'shortcut Ctrl\+Shift\+G -> tab.group.new') -and ($log -match 'shortcut Ctrl\+Shift\+U -> tab.ungroup')) { Pass 'the group and ungroup shortcuts ran' } else { Fail 'the group shortcuts were not seen' }
  if (-not $app.Proc.Responding) { Fail 'the window stopped responding' }
}
finally {
  Stop-App $app
  if ($srv) { Stop-Process -Id $srv.Proc.Id -Force -ErrorAction SilentlyContinue }
}
if ((Get-FocusRestores) -gt 0) { Write-Host "note: the app window lost the foreground $(Get-FocusRestores) time(s) during the run and was brought back" -ForegroundColor Yellow }
if ($failures.Count -gt 0) { Write-Host "M1.4 sidebar check: $($failures.Count) failure(s)" -ForegroundColor Red; exit 1 }
Write-Host 'M1.4 sidebar check: all passed' -ForegroundColor Green
