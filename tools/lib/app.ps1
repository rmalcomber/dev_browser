# Shared helpers for the scripts in tools/. Dot-source it:  . "$PSScriptRoot\lib\app.ps1"
# Promoted and generalised from the M0 spike's drive/shot/flash scripts. Windows only. Needs an unlocked interactive desktop for
# anything that sends input or takes pictures: leave the mouse and keyboard alone while such a script runs.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
if (-not ('Win' -as [type])) {
  Add-Type @'
using System; using System.Runtime.InteropServices;
public static class Win {
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint f);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr e);
}
'@
}
[void][Win]::SetProcessDPIAware()

$script:RepoRoot = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent

function Get-AppExe {
  $exe = Join-Path $script:RepoRoot 'target\debug\dev_browser.exe'
  if (-not (Test-Path $exe)) { throw "No $exe. Run tools\build.ps1 build first." }
  $exe
}

# Start the app on a throw-away data folder and wait for its window. Returns a hashtable: Proc, Out, Err, Dir, Handle.
function Start-App([string]$Url = 'https://example.com/', [string[]]$ExtraArgs = @(), [int]$WindowSeconds = 40, [switch]$NoFocus, [string]$ReuseDir = '') {
  Stop-App
  # -ReuseDir: start again on a folder an earlier Start-App made (its data survives), to test what a restart keeps
  $dir = if ($ReuseDir) { $ReuseDir } else { Join-Path $env:TEMP ('devb_' + [guid]::NewGuid().ToString('N').Substring(0, 8)) }
  if (-not (Test-Path $dir)) { New-Item -ItemType Directory $dir | Out-Null }
  $stamp = [guid]::NewGuid().ToString('N').Substring(0, 6)
  $o = Join-Path $dir "app-$stamp.out"; $e = Join-Path $dir "app-$stamp.err"
  $p = Start-Process (Get-AppExe) -ArgumentList (@("--data-dir=$(Join-Path $dir 'data')", "--url=$Url") + $ExtraArgs) -RedirectStandardOutput $o -RedirectStandardError $e -PassThru
  $h = [IntPtr]::Zero
  $sw = [Diagnostics.Stopwatch]::StartNew()
  while ($h -eq [IntPtr]::Zero -and $sw.Elapsed.TotalSeconds -lt $WindowSeconds) { Start-Sleep -Milliseconds 250; $p.Refresh(); $h = $p.MainWindowHandle }
  if ($h -eq [IntPtr]::Zero) { Stop-App; throw 'the app showed no window' }
  $app = @{ Proc = $p; Out = $o; Err = $e; Dir = $dir; Handle = $h }
  # input sent before the page has loaded and taken focus is lost (seen on the first start after a build), so wait for it
  [void](Wait-Log $app '\[host\] page (loaded|failed to load)' 30)
  if (-not $NoFocus) { Focus-App $app }
  $app
}

# Bring the window to the front, topmost, so a screen capture holds only the app and input goes to it.
function Focus-App($app) {
  [void][Win]::ShowWindow($app.Handle, 9)
  # a tap on Alt lets SetForegroundWindow succeed from a background script
  [Win]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero); [Win]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
  [void][Win]::SetWindowPos($app.Handle, [IntPtr](-1), 0, 0, 0, 0, 0x43)
  [void][Win]::SetForegroundWindow($app.Handle)
  Start-Sleep -Milliseconds 1000
}

function Stop-App($app = $null) {
  Get-Process dev_browser -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  Start-Sleep -Milliseconds 400
  if ($app -and $app.Dir -and (Test-Path $app.Dir)) { Remove-Item $app.Dir -Recurse -Force -ErrorAction SilentlyContinue }
}

# Client area in screen pixels: X, Y, W, H.
function Get-Client($app) {
  $o = New-Object Win+POINT; [void][Win]::ClientToScreen($app.Handle, [ref]$o)
  $c = New-Object Win+RECT; [void][Win]::GetClientRect($app.Handle, [ref]$c)
  @{ X = $o.X; Y = $o.Y; W = $c.R; H = $c.B }
}

# Wait until the app's stdout or stderr has a line matching $pattern. Returns the line, or $null on timeout.
function Wait-Log($app, [string]$pattern, [int]$seconds = 15) {
  $sw = [Diagnostics.Stopwatch]::StartNew()
  while ($sw.Elapsed.TotalSeconds -lt $seconds) {
    $hit = Get-Content $app.Out, $app.Err -ErrorAction SilentlyContinue | Where-Object { $_ -match $pattern } | Select-Object -First 1
    if ($hit) { return $hit }
    Start-Sleep -Milliseconds 250
  }
  $null
}

# Click at a point given in client-area pixels at 100% scale; -Scale is the window's device scale factor.
function Click-Client($app, [double]$x, [double]$y, [double]$Scale = 1.0) {
  $c = Get-Client $app
  [void][Win]::SetCursorPos($c.X + [int]($x * $Scale), $c.Y + [int]($y * $Scale)); Start-Sleep -Milliseconds 150
  [Win]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); [Win]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 400
}

$script:VK = @{ esc = 0x1B; enter = 0x0D; tab = 0x09; space = 0x20; backspace = 0x08; left = 0x25; up = 0x26; right = 0x27; down = 0x28;
  ctrl = 0x11; shift = 0x10; alt = 0x12; '[' = 0xDB; ']' = 0xDD; ',' = 0xBC; '.' = 0xBE; '/' = 0xBF; '-' = 0xBD; '=' = 0xBB }
foreach ($i in 1..12) { $script:VK["f$i"] = 0x6F + $i }

function Get-Vk([string]$name) {
  $n = $name.ToLower()
  if ($script:VK.ContainsKey($n)) { return [byte]$script:VK[$n] }
  if ($n -match '^[a-z0-9]$') { return [byte][char]$n.ToUpper() }
  throw "unknown key '$name'"
}

# Press a chord such as "ctrl+3", "ctrl+shift+p", "esc" or "f6".
function Send-Keys([string]$chord) {
  $parts = $chord.Split('+'); $key = Get-Vk $parts[-1]
  $mods = @(); if ($parts.Count -gt 1) { $mods = $parts[0..($parts.Count - 2)] | ForEach-Object { Get-Vk $_ } }
  foreach ($m in $mods) { [Win]::keybd_event($m, 0, 0, [UIntPtr]::Zero) }
  [Win]::keybd_event($key, 0, 0, [UIntPtr]::Zero); [Win]::keybd_event($key, 0, 2, [UIntPtr]::Zero)
  for ($i = $mods.Count - 1; $i -ge 0; $i--) { [Win]::keybd_event($mods[$i], 0, 2, [UIntPtr]::Zero) }
}

# Type text as real key presses (letters, digits, space, and shift for capitals).
function Send-Text([string]$text) {
  foreach ($ch in $text.ToCharArray()) {
    $s = [string]$ch
    if ($s -cmatch '[A-Z]') { Send-Keys ("shift+" + $s.ToLower()) }
    elseif ($s -eq ' ') { Send-Keys 'space' }
    else { Send-Keys $s }
    Start-Sleep -Milliseconds 40
  }
}

# Close the app the way a user does (one click on X), wait for it to exit. Returns $true if it exited within $seconds.
function Close-App($app, [int]$seconds = 20) {
  [void]$app.Proc.CloseMainWindow()
  $sw = [Diagnostics.Stopwatch]::StartNew()
  while ($sw.Elapsed.TotalSeconds -lt $seconds) { if (-not (Get-Process dev_browser -ErrorAction SilentlyContinue)) { return $true }; Start-Sleep -Milliseconds 200 }
  $false
}

# Save a picture of the window (or only its client area) to a PNG file.
function Save-Window($app, [string]$path, [switch]$ClientOnly) {
  if ($ClientOnly) { $c = Get-Client $app; $x = $c.X; $y = $c.Y; $w = $c.W; $h = $c.H }
  else { $r = New-Object Win+RECT; [void][Win]::GetWindowRect($app.Handle, [ref]$r); $x = $r.L; $y = $r.T; $w = $r.R - $r.L; $h = $r.B - $r.T }
  $bmp = New-Object System.Drawing.Bitmap $w, $h
  $g = [System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($x, $y, 0, 0, $bmp.Size); $g.Dispose()
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
}

# Serve tools/test-pages over http on a free port. Returns Proc and Port; stop it with Stop-Process -Id.
function Start-TestServer {
  $out = Join-Path $env:TEMP ('devb_srv_' + [guid]::NewGuid().ToString('N').Substring(0, 8) + '.txt')
  $p = Start-Process node -ArgumentList (Join-Path $script:RepoRoot 'tools\test-server.js'), '0' -RedirectStandardOutput $out -PassThru -WindowStyle Hidden
  $port = $null
  for ($i = 0; $i -lt 40 -and -not $port; $i++) { Start-Sleep -Milliseconds 200; $l = Get-Content $out -ErrorAction SilentlyContinue | Select-Object -First 1; if ($l -match 'listening (\d+)') { $port = $Matches[1] } }
  if (-not $port) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue; throw 'the test server did not start' }
  @{ Proc = $p; Port = $port }
}
