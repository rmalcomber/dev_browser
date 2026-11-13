# M3.2 checks on the real window (planning/m3-plan.md): the right-hand widget panel.
#   A  Pin a tool from the launcher (keyboard only): the panel opens, the page is narrowed by exactly its width (nothing is covered).
#   B  Ctrl+Shift+B hides and shows it; showing it puts the keyboard in the pinned tool; text pasted there is decoded and one value copies.
#   C  Dragging the panel's edge with the mouse resizes it.
#   D  After a clean restart the panel is open at the same width with the same tool pinned (the layout is stored).
#   E  S9: the token pasted into the widget is in none of the app's databases and in no line of the log.
# Pages are measured by the colour of the workspace frame at their right edge. Takes over the mouse and keyboard (about 2 minutes).
# Needs a build and node. Exit code 0 = pass.
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
function Last-Panel($app) { $l = @(Log $app | Where-Object { $_ -match 'panel (open|closed) \(width (\d+)\)' }); if ($l.Count) { [void]($l[-1] -match 'panel (open|closed) \(width (\d+)\)'); @($Matches[1], [int]$Matches[2]) } }
# the colour of the pixel at a client position, as r,g,b
function Pixel($app, [int]$x, [int]$y) {
  $f = Join-Path $env:TEMP ('m32_px_' + [guid]::NewGuid().ToString('N').Substring(0, 6) + '.png')
  Save-Window $app $f -ClientOnly
  $b = New-Object System.Drawing.Bitmap $f; $p = $b.GetPixel($x, $y); $b.Dispose(); Remove-Item $f -ErrorAction SilentlyContinue
  @($p.R, $p.G, $p.B)
}
function Is-Blue($c) { ([math]::Abs($c[0] - 0x2C) -le 6) -and ([math]::Abs($c[1] - 0x5F) -le 6) -and ([math]::Abs($c[2] - 0xA8) -le 6) }
function Open-Launcher($app) { $n = Count $app 'launcher shown'; Send-Keys 'ctrl+p'; Wait-Count $app 'launcher shown' ($n + 1) 8 }
function Copy-Sub($app, $token) {
  # the focus is in the widget's input: paste the token, then Tab to "Copy sub" (Header, Payload, Algorithm, iss, sub) and press it
  Set-Clipboard -Value $token; Send-Keys 'ctrl+v'; Start-Sleep -Milliseconds 800
  Set-Clipboard -Value 'untouched'
  foreach ($i in 1..5) { Send-Keys 'tab'; Start-Sleep -Milliseconds 150 }
  Send-Keys 'enter'; Start-Sleep -Milliseconds 800
  (Get-Clipboard -Raw)
}

$srv = $null; $app = $null; $dir = $null
$page = Get-Content (Join-Path $PSScriptRoot 'test-pages\token.html') -Raw
$token = [regex]::Match($page, '<p id="t"[^>]*>([^<]+)</p>').Groups[1].Value.Trim()
$parts = $token.Split('.')
try {
  $srv = Start-TestServer
  $h = "127.0.0.1:$($srv.Port)"
  $app = Start-App -Url "http://$h/token.html"
  $dir = $app.Dir
  Start-Sleep -Milliseconds 800
  $c = Get-Client $app
  $W = $c.W

  # ---- A
  Write-Host 'Pinning'
  $before = Pixel $app ($W - 321) 300
  if (Is-Blue $before) { Fail 'A: the page already ended before the panel (setup)' }
  Triple-Click-Client $app 400 130
  [void](Open-Launcher $app); Start-Sleep -Milliseconds 800
  Send-Keys 'enter'; Start-Sleep -Milliseconds 1500       # the JWT decoder, started from the selection
  Send-Keys 'shift+tab'; Start-Sleep -Milliseconds 300     # from the input back to Pin
  Send-Keys 'enter'; Start-Sleep -Milliseconds 1500
  Send-Keys 'esc'; Start-Sleep -Milliseconds 800     # the panel appears when the launcher closes (showing it would take the keyboard from the launcher)
  if (Wait-Count $app 'panel open \(width 320\)' 1 8) { Pass 'A: pinning from the launcher (keyboard only) opened the panel' } else { Fail 'A: the panel did not open' }
  Start-Sleep -Milliseconds 500
  if (Is-Blue (Pixel $app ($W - 321) 300)) { Pass 'A: the page ends exactly where the panel begins (its frame is at the edge)' } else { Fail 'A: the page was not narrowed by the width of the panel' }

  # ---- B
  Write-Host 'Hiding and showing'
  Send-Keys 'ctrl+shift+b'
  if (Wait-Count $app 'panel closed' 1 6) { Pass 'B: Ctrl+Shift+B hid the panel' } else { Fail 'B: the panel did not close' }
  Start-Sleep -Milliseconds 800
  if (-not (Is-Blue (Pixel $app ($W - 321) 300))) { Pass 'B: and the page took the space back' } else { Fail 'B: the page stayed narrow after the panel closed' }
  Send-Keys 'ctrl+shift+b'; Start-Sleep -Milliseconds 1200
  $clip = Copy-Sub $app $token
  if ($clip -and $clip.Trim() -eq 'quokka-9') { Pass 'B: showing the panel focused the pinned tool; the pasted token was decoded and "sub" copied' } else { Fail "B: the clipboard has '$clip', expected quokka-9" }

  # ---- C
  Write-Host 'Resizing'
  $x = $W - 320 + 2
  Drag-Client $app $x 300 ($x - 100) 300
  Start-Sleep -Milliseconds 600
  $p = Last-Panel $app
  if ($p -and $p[0] -eq 'open' -and [math]::Abs($p[1] - 420) -le 14) { Pass "C: dragging the edge made the panel $($p[1]) wide" } else { Fail "C: the panel is $($p -join ' ') after the drag, expected about 420" }
  $width = $p[1]

  # ---- D
  Write-Host 'Restarting'
  $dir = $app.Dir
  [void](Close-App $app)
  Stop-App
  $app = Start-App -Url "http://$h/colours.html" -ReuseDir $dir
  Start-Sleep -Seconds 2
  $p2 = Last-Panel $app
  if ($p2 -and $p2[0] -eq 'open' -and $p2[1] -eq $width) { Pass "D: after a restart the panel is open, $width wide" } else { Fail "D: after a restart the panel is $($p2 -join ' '), expected open $width" }
  Send-Keys 'ctrl+shift+b'; Start-Sleep -Milliseconds 800; Send-Keys 'ctrl+shift+b'; Start-Sleep -Milliseconds 1200   # hide, show: the focus goes into the pinned tool
  $clip = Copy-Sub $app $token
  if ($clip -and $clip.Trim() -eq 'quokka-9') { Pass 'D: the pinned JWT decoder was still there and works' } else { Fail "D: after the restart the clipboard has '$clip'" }

  # ---- F  a second widget, moved above the first with the keyboard, and the order kept across a restart (M3.3)
  Write-Host 'Reordering'
  Send-Keys 'esc'; Start-Sleep -Milliseconds 500
  Send-Keys 'ctrl+p'; Start-Sleep -Milliseconds 1200
  $qx = (($c.W - 640) / 2) + 300; Click-Client $app $qx 76; Send-Text 'uuid'; Start-Sleep -Milliseconds 700; Send-Keys 'enter'; Start-Sleep -Milliseconds 1200
  Send-Keys 'tab'; Send-Keys 'tab'; Send-Keys 'enter'; Start-Sleep -Milliseconds 1000      # Back, Pin: pin the UUID generator
  Send-Keys 'esc'; Start-Sleep -Milliseconds 800
  Send-Keys 'ctrl+shift+b'; Start-Sleep -Milliseconds 600; Send-Keys 'ctrl+shift+b'; Start-Sleep -Milliseconds 1200   # hide, show: the focus is in the first widget (the decoder's input)
  Send-Keys 'tab'; Start-Sleep -Milliseconds 200; Send-Keys 'tab'; Start-Sleep -Milliseconds 200   # the UUID widget's fold button, then its "move up"
  Send-Keys 'enter'; Start-Sleep -Milliseconds 1000
  [void](Close-App $app)
  Stop-App
  $app = Start-App -Url "http://$h/colours.html" -ReuseDir $dir
  Start-Sleep -Seconds 2
  Send-Keys 'ctrl+shift+b'; Start-Sleep -Milliseconds 600; Send-Keys 'ctrl+shift+b'; Start-Sleep -Milliseconds 1200   # the focus goes to the first widget: now the UUID generator's button
  Set-Clipboard -Value 'untouched'
  Send-Keys 'enter'; Start-Sleep -Milliseconds 600; Send-Keys 'tab'; Start-Sleep -Milliseconds 200; Send-Keys 'enter'; Start-Sleep -Milliseconds 800   # Generate, then Copy UUID
  $clip = Get-Clipboard -Raw
  if ($clip -and $clip.Trim() -match '^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$') { Pass 'F: the UUID widget was moved above the decoder with the keyboard and it is first after a restart' } else { Fail "F: the first widget after the restart was not the UUID generator (the clipboard has '$clip')" }

  # ---- E
  [void](Close-App $app)
  $hits = @()
  foreach ($file in @($app.Out, $app.Err)) { $t = Get-Content $file -Raw -ErrorAction SilentlyContinue; foreach ($n in @($parts[0], $parts[1].Substring(0, 24), $parts[2])) { if ($t -and $t.Contains($n)) { $hits += "log has $n" } } }
  Get-ChildItem $dir -Recurse -File -ErrorAction SilentlyContinue | Where-Object { $_.Name -match '\.db(-wal|-shm)?$' } | ForEach-Object {
    try { $text = [Text.Encoding]::Latin1.GetString([IO.File]::ReadAllBytes($_.FullName)); foreach ($n in @($parts[0], $parts[1].Substring(0, 24))) { if ($text.Contains($n)) { $hits += "$($_.Name) has $n" } } } catch { }
  }
  if ($hits.Count -eq 0) { Pass 'E (S9): the token typed into the widget is in none of the app databases and in no log line' } else { Fail "E (S9): $($hits -join '; ')" }
}
finally {
  Stop-App $app
  if ($srv) { Stop-Process -Id $srv.Proc.Id -Force -ErrorAction SilentlyContinue }
}
if ((Get-FocusRestores) -gt 0) { Write-Host "note: the app window lost the foreground $(Get-FocusRestores) time(s) during the run and was brought back" -ForegroundColor Yellow }
if ($failures.Count -gt 0) { Write-Host "M3.2 panel check: $($failures.Count) failure(s)" -ForegroundColor Red; exit 1 }
Write-Host 'M3.2 panel check: all passed' -ForegroundColor Green
