# M3.1 checks on the real window (planning/m3-plan.md): the tools in the launcher, the page selection and S9.
#   A  Select a token on the page (a real triple-click), Ctrl+P: the app captured exactly that text (its length is logged, never the text).
#   B  The JWT decoder is the first result, opens with the selection as its input, and Copy puts exactly one chosen value (the "sub"
#      claim) on the clipboard.
#   C  A tool that reads no input (the UUID generator) gives a UUID that can be copied.
#   D  S7 (one pane): with a token selected in one tab, opening the launcher over another tab captures nothing.
#   E  S9: after all that, the token (header and payload parts) is in no file of the data folder and in no line of the app's log.
# Keyboard only inside the launcher: Tab moves to the Copy buttons (that is also the keyboard-first check). Takes over the mouse and keyboard
# (about 2 minutes). Needs a build and node. Exit code 0 = pass.
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
function Open-Launcher($app) { $n = Count $app 'launcher shown'; Send-Keys 'ctrl+p'; Wait-Count $app 'launcher shown' ($n + 1) 8 }
function Query-Box($app) { $c = Get-Client $app; $x = (($c.W - 640) / 2) + 300; @($x, 76) }
function Last-Selection($app) { $l = @(Log $app | Where-Object { $_ -match 'page selection received \((\d+) characters\)' }); if ($l.Count) { [void]($l[-1] -match '\((\d+) characters\)'); [int]$Matches[1] } else { -1 } }

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

  # ---- A and B
  Write-Host 'A token selected on the page'
  Triple-Click-Client $app 400 130
  [void](Open-Launcher $app)
  if (Wait-Count $app 'page selection received' 1 8) { Pass 'A: the app asked the page for its selection when the launcher opened' } else { Fail 'A: no selection was received' }
  if ((Last-Selection $app) -eq $token.Length) { Pass "A: it captured exactly the selected text ($($token.Length) characters)" } else { Fail "A: captured $(Last-Selection $app) characters, expected $($token.Length)" }
  Start-Sleep -Milliseconds 600
  Send-Keys 'enter'; Start-Sleep -Milliseconds 1500       # the first result: the JWT decoder, because the selection looks like a token
  Set-Clipboard -Value 'untouched'
  foreach ($i in 1..5) { Send-Keys 'tab'; Start-Sleep -Milliseconds 150 }   # textarea -> Copy Header, Payload, Algorithm, iss, sub
  Send-Keys 'enter'; Start-Sleep -Milliseconds 800
  $clip = Get-Clipboard -Raw
  if ($clip -and $clip.Trim() -eq 'quokka-9') { Pass 'B: the JWT decoder was first, started from the selection, and Copy put the "sub" claim on the clipboard' } else { Fail "B: the clipboard has '$clip', expected quokka-9" }
  Save-Window $app (Join-Path $env:TEMP 'm31_tool.png')
  Send-Keys 'esc'; Start-Sleep -Milliseconds 800

  # ---- C
  Write-Host 'A tool without input'
  [void](Open-Launcher $app)
  $b = Query-Box $app; Click-Client $app $b[0] $b[1]; Send-Text 'uuid'; Start-Sleep -Milliseconds 700; Send-Keys 'enter'; Start-Sleep -Milliseconds 1200
  Set-Clipboard -Value 'untouched'
  foreach ($i in 1..3) { Send-Keys 'tab'; Start-Sleep -Milliseconds 150 }    # Back, Generate another, Copy UUID
  Send-Keys 'enter'; Start-Sleep -Milliseconds 800
  $clip = (Get-Clipboard -Raw)
  if ($clip -and $clip.Trim() -match '^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$') { Pass 'C: the UUID generator made a version 4 UUID and Copy copied it' } else { Fail "C: the clipboard has '$clip'" }
  Send-Keys 'esc'; Start-Sleep -Milliseconds 800

  # ---- D
  Write-Host 'Another tab'
  Send-Keys 'ctrl+t'; Start-Sleep -Milliseconds 1200
  $before = Count $app 'page selection received'
  [void](Open-Launcher $app)
  if (Wait-Count $app 'page selection received' ($before + 1) 8 -and (Last-Selection $app) -eq 0) { Pass 'D: over another tab nothing was captured (the selection of the first tab stayed in the first tab)' } else { Fail "D: the launcher over a new tab captured $(Last-Selection $app) characters" }
  Send-Keys 'esc'; Start-Sleep -Milliseconds 800
  if (-not $app.Proc.Responding) { Fail 'the window stopped responding' }

  # ---- E (the app is closed first so every database file is flushed)
  [void](Close-App $app)
  $logs = (Get-Content $app.Out, $app.Err -ErrorAction SilentlyContinue) -join "`n"
  $hits = @()
  foreach ($needle in @($parts[0], $parts[1].Substring(0, 24), $parts[2])) {
    if ($logs.Contains($needle)) { $hits += "the log has $needle" }
  }
  Get-ChildItem $dir -Recurse -File -ErrorAction SilentlyContinue | Where-Object { $_.Name -match '\.db(-wal|-shm)?$' } | ForEach-Object {   # the app's own databases (the page itself is in Chromium's cache: that is the page's content, not the tool's)
    try {
      $bytes = [IO.File]::ReadAllBytes($_.FullName)
      $text = [Text.Encoding]::Latin1.GetString($bytes)
      foreach ($needle in @($parts[0], $parts[1].Substring(0, 24))) { if ($text.Contains($needle)) { $hits += "$($_.Name) has $needle" } }
    } catch { }   # a file that is locked cannot be read; the databases are closed by now
  }
  if ($hits.Count -eq 0) { Pass 'E (S9): the token is in none of the app databases and in no line of the log' } else { Fail "E (S9): $($hits -join '; ')" }
}
finally {
  Stop-App $app
  if ($srv) { Stop-Process -Id $srv.Proc.Id -Force -ErrorAction SilentlyContinue }
}
if ((Get-FocusRestores) -gt 0) { Write-Host "note: the app window lost the foreground $(Get-FocusRestores) time(s) during the run and was brought back" -ForegroundColor Yellow }
if ($failures.Count -gt 0) { Write-Host "M3.1 tools check: $($failures.Count) failure(s)" -ForegroundColor Red; exit 1 }
Write-Host 'M3.1 tools check: all passed' -ForegroundColor Green
