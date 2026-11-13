# Start the app, optionally click somewhere, save a picture of its window, close it.
#   tools\shot.ps1 -Out C:\temp\a.png [-Url https://example.com/] [-Page colours.html] [-Click 45,92] [-Keys ctrl+3] [-Wait 6] [-ClientOnly]
# -Click is in client pixels at 100% scale. -Page serves a file from tools\test-pages over http instead of -Url. Keep pictures out of the repo.
param(
  [Parameter(Mandatory)][string]$Out,
  [string]$Url = 'https://example.com/',
  [string]$Page = '',
  [string]$Click = '',
  [string]$Keys = '',
  [int]$Wait = 6,
  [double]$Scale = 1.0,
  [switch]$ClientOnly,
  [string[]]$AppArgs = @()
)
. "$PSScriptRoot\lib\app.ps1"
$srv = $null; $app = $null
try {
  if ($Page) { $srv = Start-TestServer; $Url = "http://127.0.0.1:$($srv.Port)/$Page" }
  $app = Start-App -Url $Url -ExtraArgs $AppArgs
  Start-Sleep -Seconds $Wait
  if ($Click) { $xy = $Click -split ','; Click-Client $app ([double]$xy[0]) ([double]$xy[1]) $Scale; Start-Sleep -Milliseconds 800 }
  if ($Keys) { foreach ($k in $Keys -split ' ') { Send-Keys $k; Start-Sleep -Milliseconds 700 } }
  Save-Window $app $Out -ClientOnly:$ClientOnly
  $b = New-Object System.Drawing.Bitmap $Out; Write-Host "saved $Out ($($b.Width)x$($b.Height))"; $b.Dispose()
} finally {
  Stop-App $app
  if ($srv) { Stop-Process -Id $srv.Proc.Id -Force -ErrorAction SilentlyContinue }
}
