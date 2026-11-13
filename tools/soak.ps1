# Soak: leave the app open and busy, and watch for growth. Opens -Tabs tabs across two workspaces, then every -StepSeconds visits a tab,
# loads a page and (every few steps) closes one and opens another. Samples the process count and total working set once a minute.
#   tools\soak.ps1 -Minutes 10            a short run
#   tools\soak.ps1 -Minutes 240 -Tabs 30  an overnight-style run
# Bound (M1.7): the process count never exceeds -MaxProcs, and the working set in the last quarter of the run is no more than
# -GrowthPercent above the second quarter's (the first is warm-up). Exit code 0 = within the bound. Takes over the keyboard.
param([int]$Minutes = 10, [int]$Tabs = 14, [int]$StepSeconds = 6, [int]$MaxProcs = 22, [int]$GrowthPercent = 40)
. "$PSScriptRoot\lib\app.ps1"
function Procs { @(Get-Process dev_browser -ErrorAction SilentlyContinue).Count }
function Mem-MB { [int]((Get-Process dev_browser -ErrorAction SilentlyContinue | Measure-Object WorkingSet64 -Sum).Sum / 1MB) }
function Go($text) { Send-Keys 'ctrl+l'; Start-Sleep -Milliseconds 600; Send-Text $text; Send-Keys 'enter' }
$srv = $null; $app = $null; $samples = @(); $code = 0
try {
  $srv = Start-TestServer
  $h = "127.0.0.1:$($srv.Port)"
  $app = Start-App -Url "http://$h/nav-a.html"
  Start-Sleep -Seconds 1
  $pages = 'nav-a.html', 'nav-b.html', 'colours.html', 'identity.html', 'icon.png'
  foreach ($i in 1..($Tabs - 1)) {
    if ($i -eq [int]($Tabs / 2)) { Send-Keys 'alt+n'; Start-Sleep -Milliseconds 800; Send-Text 'Soak two'; Send-Keys 'enter'; Start-Sleep -Seconds 2 }
    Send-Keys 'ctrl+t'; Start-Sleep -Milliseconds 700; Go "$h/$($pages[$i % $pages.Count])"; Start-Sleep -Milliseconds 800
  }
  Write-Host "opened $Tabs tabs; $(Procs) processes, $(Mem-MB) MB"
  $start = Get-Date; $end = $start.AddMinutes($Minutes); $step = 0; $nextSample = $start.AddMinutes(1)
  while ((Get-Date) -lt $end) {
    $step++
    Send-Keys 'ctrl+tab'; Start-Sleep -Milliseconds 800
    Go "$h/$($pages[$step % $pages.Count])?n=$step"
    if ($step % 5 -eq 0) { Send-Keys 'ctrl+w'; Start-Sleep -Milliseconds 700; Send-Keys 'ctrl+t'; Start-Sleep -Milliseconds 700; Go "$h/$($pages[$step % $pages.Count])" }
    if ($step % 17 -eq 0) { Send-Keys 'alt+pagedown'; Start-Sleep -Milliseconds 1200 }
    Start-Sleep -Seconds $StepSeconds
    if ((Get-Date) -ge $nextSample) {
      $samples += [pscustomobject]@{ Minute = [math]::Round(((Get-Date) - $start).TotalMinutes, 1); Procs = Procs; MB = Mem-MB }
      Write-Host ("  minute {0,6}: {1,3} processes, {2,5} MB" -f $samples[-1].Minute, $samples[-1].Procs, $samples[-1].MB)
      $nextSample = (Get-Date).AddMinutes(1)
      if (-not $app.Proc.Responding) { Write-Host 'FAIL: the window stopped responding' -ForegroundColor Red; $code = 1; break }
    }
  }
  if ($samples.Count -ge 8) {
    $q = [int]($samples.Count / 4)
    $early = ($samples[$q..(2 * $q - 1)] | Measure-Object MB -Average).Average
    $late = ($samples[($samples.Count - $q)..($samples.Count - 1)] | Measure-Object MB -Average).Average
    $growth = [math]::Round(($late - $early) / $early * 100, 1)
    $maxp = ($samples | Measure-Object Procs -Maximum).Maximum
    Write-Host "memory: second quarter $([int]$early) MB, last quarter $([int]$late) MB ($growth %); most processes $maxp"
    if ($growth -gt $GrowthPercent) { Write-Host "FAIL: memory grew $growth % (bound $GrowthPercent %)" -ForegroundColor Red; $code = 1 }
    if ($maxp -gt $MaxProcs) { Write-Host "FAIL: $maxp processes (bound $MaxProcs)" -ForegroundColor Red; $code = 1 }
  } else { Write-Host 'note: too short a run to judge growth (needs 8 or more minutes)' -ForegroundColor Yellow }
}
finally {
  Stop-App $app
  if ($srv) { Stop-Process -Id $srv.Proc.Id -Force -ErrorAction SilentlyContinue }
}
if ($code -eq 0) { Write-Host 'soak: within the bound' -ForegroundColor Green }
exit $code
