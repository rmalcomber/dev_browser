# Launches the spike, waits, saves a screenshot of its window to -Out, then closes it. Disposable.
param([string]$Out, [int]$Wait = 10, [string]$AppArgs = "--exit-after=90", [string]$Click = '', [string]$Resize = '', [string]$Exe = (Join-Path $PSScriptRoot 'target/debug/m0_spike.exe'))
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class S { [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint f); }
'@
[void][S]::SetProcessDPIAware()
$dir = Split-Path $Exe; Push-Location $dir
$p = Start-Process $Exe -ArgumentList ($AppArgs -split " ") -RedirectStandardOutput (Join-Path $dir 'shot.out') -RedirectStandardError (Join-Path $dir 'shot.err') -PassThru
Start-Sleep -Seconds $Wait
$p.Refresh(); $h = $p.MainWindowHandle
if ($h -eq [IntPtr]::Zero) { Write-Host 'no window'; Get-Process m0_spike -ErrorAction SilentlyContinue | Stop-Process -Force; exit 1 }
# Topmost so the capture contains only the spike (a plain screen copy also grabs whatever is above it).
[void][S]::SetWindowPos($h, [IntPtr](-1), 0, 0, 0, 0, 0x43); [void][S]::SetForegroundWindow($h); Start-Sleep -Milliseconds 1000
if ($Click) { $xy = $Click -split ','; $rr = New-Object S+RECT; [void][S]::GetWindowRect($h, [ref]$rr); [void][S]::SetCursorPos($rr.L + [int]$xy[0], $rr.T + [int]$xy[1]); Start-Sleep -Milliseconds 300; [S]::mouse_event(2,0,0,0,[UIntPtr]::Zero); [S]::mouse_event(4,0,0,0,[UIntPtr]::Zero); Start-Sleep -Milliseconds 1500 }
$r = New-Object S+RECT; [void][S]::GetWindowRect($p.MainWindowHandle, [ref]$r)
$bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
$g = [System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size); $g.Dispose()
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
Get-Process m0_spike -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Pop-Location
Write-Host "saved $Out ($($r.R - $r.L)x$($r.B - $r.T))"
