# Record the app's client area at 60 fps while sending timed keys, then report every colour state the regions went through.
# The engine of the S8 check ("switching never shows a frame of the old workspace") and of any flicker or flash question.
#   tools\record.ps1 -Page colours.html -Keys "3@1500,2@3000" -Seconds 5 -Expect "K|R,K|B,K|G"
# -Keys is a comma list of chord@milliseconds-after-recording-starts (see Send-Keys in lib\app.ps1: ctrl+3, esc, f6 ...).
# -Regions are width fractions as in analyze-frames.js (default: the shell strip, then the page). Needs ffmpeg and node on PATH.
# Exit code 1 if a frame was not one of -Expect. Pictures and raw frames go to the temp folder and are deleted.
param(
  [string]$Url = 'https://example.com/',
  [string]$Page = '',
  [string]$Keys = '',
  [int]$Seconds = 6,
  [string]$Regions = '0.02-0.12,0.3-0.95',
  [string]$Expect = '',
  [string[]]$AppArgs = @()
)
. "$PSScriptRoot\lib\app.ps1"
foreach ($t in 'ffmpeg', 'node') { if (-not (Get-Command $t -ErrorAction SilentlyContinue)) { throw "$t is not on PATH" } }
$srv = $null; $app = $null; $raw = Join-Path $env:TEMP ('devb_frames_' + [guid]::NewGuid().ToString('N').Substring(0, 8) + '.raw')
$code = 0
try {
  if ($Page) { $srv = Start-TestServer; $Url = "http://127.0.0.1:$($srv.Port)/$Page" }
  $app = Start-App -Url $Url -ExtraArgs $AppArgs
  Start-Sleep -Seconds 3   # let the first page settle before recording
  $c = Get-Client $app
  $w = $c.W - ($c.W % 2); $h = $c.H - ($c.H % 2)
  Write-Host "recording client area ${w}x${h} at ($($c.X),$($c.Y)) for $Seconds s"
  $ff = Start-Process ffmpeg -ArgumentList '-y', '-loglevel', 'error', '-f', 'gdigrab', '-framerate', '60', '-offset_x', $c.X, '-offset_y', $c.Y, '-video_size', "${w}x${h}",
    '-i', 'desktop', '-t', $Seconds, '-vf', 'scale=64:36', '-pix_fmt', 'rgb24', '-f', 'rawvideo', $raw -PassThru -WindowStyle Hidden
  $sw = [Diagnostics.Stopwatch]::StartNew()
  if ($Keys) {
    foreach ($k in ($Keys -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ })) {
      $chord, $at = $k -split '@'
      $wait = [int]$at - $sw.ElapsedMilliseconds; if ($wait -gt 0) { Start-Sleep -Milliseconds $wait }
      Send-Keys $chord
    }
  }
  $ff | Wait-Process -Timeout ($Seconds + 15) -ErrorAction SilentlyContinue
  $args2 = @((Join-Path $PSScriptRoot 'analyze-frames.js'), $raw, 64, 36, '--regions', $Regions)
  if ($Expect) { $args2 += @('--expect', $Expect) }
  & node @args2
  $code = $LASTEXITCODE
} finally {
  Stop-App $app
  if ($srv) { Stop-Process -Id $srv.Proc.Id -Force -ErrorAction SilentlyContinue }
  Remove-Item $raw -ErrorAction SilentlyContinue
}
exit $code
