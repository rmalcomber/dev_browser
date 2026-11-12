# Launches the spike, takes -Frames screenshots of its window ~300ms apart and reports how many distinct frames there were.
# A steady window gives 1. Frames are saved to -OutDir as f1.png.. (keep them out of the repo). Disposable.
param([string]$OutDir, [string]$AppArgs = '', [int]$Frames = 12, [string]$Exe = (Join-Path $PSScriptRoot 'target/debug/m0_spike.exe'))
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class F { [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
 [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h); }
'@
[void][F]::SetProcessDPIAware()
New-Item -ItemType Directory -Force $OutDir | Out-Null
$dir = Split-Path $Exe; Push-Location $dir
$argv = @(); if ($AppArgs) { $argv = $AppArgs -split ' ' }
$p = if ($argv.Count) { Start-Process $Exe -ArgumentList $argv -PassThru -RedirectStandardOutput f.out -RedirectStandardError f.err } else { Start-Process $Exe -PassThru -RedirectStandardOutput f.out -RedirectStandardError f.err }
Start-Sleep 10; $p.Refresh(); $h = $p.MainWindowHandle
[void][F]::SetWindowPos($h, [IntPtr](-1), 0, 0, 0, 0, 0x43); [void][F]::SetForegroundWindow($h); Start-Sleep 2
$hashes = @()
foreach ($i in 1..$Frames) {
  $r = New-Object F+RECT; [void][F]::GetWindowRect($h, [ref]$r)
  $b = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T); $g = [System.Drawing.Graphics]::FromImage($b)
  $g.CopyFromScreen($r.L, $r.T, 0, 0, $b.Size); $g.Dispose(); $f = Join-Path $OutDir "f$i.png"; $b.Save($f); $b.Dispose()
  $hashes += (Get-FileHash $f).Hash.Substring(0, 8); Start-Sleep -Milliseconds 300
}
Get-Process m0_spike -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Remove-Item f.out, f.err -ErrorAction SilentlyContinue
Pop-Location
Write-Host "$Frames frames, distinct: $(($hashes | Sort-Object -Unique).Count)  ($($hashes -join ' '))"
