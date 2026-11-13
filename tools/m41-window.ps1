# Checks on the real window for the title-bar buttons, the collapsible panels and the extra tools (after M3):
#   A  The window has minimise and maximise commands (F11 maximises and restores, Ctrl+Shift+M minimises); the title bar buttons are the same calls.
#   B  Ctrl+B collapses the sidebar to a rail and the page takes the space; the rail's button expands it; the state survives a restart.
#   C  The widget panel collapses to a rail (Ctrl+Shift+B) and expands again.
#   D  One of the later tools from the Home Screen project (the calculator) works from the launcher, by keyboard, and copies its result.
# Pages are measured by the colour of the workspace frame at their left edge. Takes over the mouse and keyboard (about 1 minute).
# Needs a build and node. Exit code 0 = pass.
param([int]$Wait = 15)
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
function Pixel($app, [int]$x, [int]$y) {
  $f = Join-Path $env:TEMP ('m41_px_' + [guid]::NewGuid().ToString('N').Substring(0, 6) + '.png')
  Save-Window $app $f -ClientOnly
  $b = New-Object System.Drawing.Bitmap $f; $p = $b.GetPixel($x, $y); $b.Dispose(); Remove-Item $f -ErrorAction SilentlyContinue
  @($p.R, $p.G, $p.B)
}
function Is-Blue($c) { ([math]::Abs($c[0] - 0x2C) -le 6) -and ([math]::Abs($c[1] - 0x5F) -le 6) -and ([math]::Abs($c[2] - 0xA8) -le 6) }

$srv = $null; $app = $null; $dir = $null
try {
  $srv = Start-TestServer
  $h = "127.0.0.1:$($srv.Port)"
  $app = Start-App -Url "http://$h/colours.html"
  $dir = $app.Dir
  Start-Sleep -Milliseconds 800

  # ---- A
  Write-Host 'The window'
  $w0 = (Get-Client $app).W
  Send-Keys 'f11'; Start-Sleep -Milliseconds 1200
  if ([Win]::IsZoomed($app.Handle)) { Pass 'A: F11 maximised the window' } else { Fail 'A: F11 did not maximise the window' }
  Send-Keys 'f11'; Start-Sleep -Milliseconds 1200
  if (-not [Win]::IsZoomed($app.Handle) -and (Get-Client $app).W -eq $w0) { Pass 'A: F11 again restored it to its size' } else { Fail 'A: F11 did not restore the window' }
  Send-Keys 'ctrl+shift+m'; Start-Sleep -Milliseconds 1200
  if ([Win]::IsIconic($app.Handle)) { Pass 'A: Ctrl+Shift+M minimised the window' } else { Fail 'A: Ctrl+Shift+M did not minimise the window' }
  [void][Win]::ShowWindow($app.Handle, 9); Start-Sleep -Milliseconds 800   # restore, so the rest can be driven

  # ---- B
  Write-Host 'The sidebar'
  if (Is-Blue (Pixel $app 221 300)) { Pass 'B: with the sidebar open the page starts 220 px in' } else { Fail 'B: setup: the page does not start at 220' }
  Send-Keys 'ctrl+b'
  if (Wait-Count $app 'sidebar collapsed' 1 6) { Pass 'B: Ctrl+B collapsed the sidebar' } else { Fail 'B: the sidebar did not collapse' }
  Start-Sleep -Milliseconds 1000
  if (Is-Blue (Pixel $app 45 300)) { Pass 'B: and the page moved left to make room (it starts 44 px in)' } else { Fail 'B: the page did not take the space' }
  Click-Client $app 22 20                       # the rail's expand button
  if (Wait-Count $app 'sidebar expanded' 2 6) { Pass 'B: the button on the rail expanded it again' } else { Fail 'B: the rail button did nothing' }
  Start-Sleep -Milliseconds 800
  Send-Keys 'ctrl+b'; Start-Sleep -Milliseconds 800
  [void](Close-App $app)
  Stop-App
  $app = Start-App -Url "http://$h/colours.html" -ReuseDir $dir
  Start-Sleep -Seconds 1
  if (Wait-Count $app 'sidebar collapsed' 1 6) { Pass 'B: after a restart the sidebar is still collapsed' } else { Fail 'B: the collapsed state was not remembered' }
  Send-Keys 'ctrl+b'; Start-Sleep -Milliseconds 800

  # ---- C
  Write-Host 'The widget panel'
  $c = Get-Client $app; $W = $c.W
  if (-not (Is-Blue (Pixel $app ($W - 321) 300))) { Pass 'C: collapsed, the panel is only a rail: the page runs almost to the edge' } else { Fail 'C: the page ends 320 px from the edge with the panel collapsed' }
  Send-Keys 'ctrl+shift+b'
  if (Wait-Count $app 'panel open' 1 6) { Pass 'C: Ctrl+Shift+B expanded it' } else { Fail 'C: the panel did not expand' }
  Start-Sleep -Milliseconds 800
  if (Is-Blue (Pixel $app ($W - 321) 300)) { Pass 'C: and the page made room for it' } else { Fail 'C: the page did not make room' }
  Send-Keys 'ctrl+shift+b'; Start-Sleep -Milliseconds 800

  # ---- D
  Write-Host 'A later tool'
  Send-Keys 'ctrl+p'; Start-Sleep -Milliseconds 1200
  $qx = (($W - 32 - 640) / 2) + 300; Click-Client $app $qx 76; Send-Text 'calculator'; Start-Sleep -Milliseconds 700; Send-Keys 'enter'; Start-Sleep -Milliseconds 1200
  Send-Text '100/4'; Start-Sleep -Milliseconds 800
  Set-Clipboard -Value 'untouched'
  Send-Keys 'tab'; Start-Sleep -Milliseconds 200; Send-Keys 'enter'; Start-Sleep -Milliseconds 800     # from the input to Copy Result
  $clip = Get-Clipboard -Raw
  if ($clip -and $clip.Trim() -eq '25') { Pass 'D: the calculator is in the launcher, worked from the keyboard and copied 25' } else { Fail "D: the clipboard has '$clip', expected 25" }
  if (-not $app.Proc.Responding) { Fail 'the window stopped responding' }
}
finally {
  Stop-App $app
  if ($srv) { Stop-Process -Id $srv.Proc.Id -Force -ErrorAction SilentlyContinue }
}
if ((Get-FocusRestores) -gt 0) { Write-Host "note: the app window lost the foreground $(Get-FocusRestores) time(s) during the run and was brought back" -ForegroundColor Yellow }
if ($failures.Count -gt 0) { Write-Host "Window and panels check: $($failures.Count) failure(s)" -ForegroundColor Red; exit 1 }
Write-Host 'Window and panels check: all passed' -ForegroundColor Green
