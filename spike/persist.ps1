# Divider position save check: run 1 drags the divider with the real mouse and closes the window with one click on X;
# run 2 starts again (keeping .m0-data) and the log should show the saved position being loaded and applied. Disposable.
param([string]$OutDir = '', [string]$Exe = (Join-Path $PSScriptRoot 'target/debug/m0_spike.exe'))
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class Q { [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
 [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
 [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e); }
'@
[void][Q]::SetProcessDPIAware()
$dir = Split-Path $Exe; Push-Location $dir
Remove-Item -Recurse -Force .m0-data -ErrorAction SilentlyContinue
function Start-App($tag) { $p = Start-Process $Exe -PassThru -RedirectStandardOutput "persist-$tag.out" -RedirectStandardError "persist-$tag.err"; Start-Sleep 10; $p.Refresh(); $h = $p.MainWindowHandle
  [void][Q]::SetWindowPos($h, [IntPtr](-1), 0, 0, 0, 0, 0x43); [void][Q]::SetForegroundWindow($h); Start-Sleep 1; return @($p, $h) }
function Shot($h, $name) { if (-not $OutDir) { return }; New-Item -ItemType Directory -Force $OutDir | Out-Null; $r = New-Object Q+RECT; [void][Q]::GetWindowRect($h, [ref]$r); $b = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T); $g = [System.Drawing.Graphics]::FromImage($b); $g.CopyFromScreen($r.L, $r.T, 0, 0, $b.Size); $g.Dispose(); $b.Save((Join-Path $OutDir "$name.png")); $b.Dispose() }

Write-Host '=== run 1: drag the divider +150px, then close with one click on X'
$p, $h = Start-App 'run1'
Get-Content persist-run1.out | Where-Object { $_ -match 'divider position|relayout' } | Select-Object -Last 2 | ForEach-Object { Write-Host $_ }
$o = New-Object Q+POINT; [void][Q]::ClientToScreen($h, [ref]$o); $c = New-Object Q+RECT; [void][Q]::GetClientRect($h, [ref]$c)
$sx = $o.X + [int]($c.R / 2); $sy = $o.Y + 300
[void][Q]::SetCursorPos($sx, $sy); Start-Sleep -Milliseconds 300; [Q]::mouse_event(2,0,0,0,[UIntPtr]::Zero); Start-Sleep -Milliseconds 300
foreach ($i in 1..15) { [void][Q]::SetCursorPos($sx + 10 * $i, $sy); Start-Sleep -Milliseconds 40 }
[Q]::mouse_event(4,0,0,0,[UIntPtr]::Zero); Start-Sleep -Seconds 1
Get-Content persist-run1.out | Where-Object { $_ -match 'relayout' } | Select-Object -Last 1 | ForEach-Object { Write-Host $_ }
Shot $h 'run1-dragged'
$r = New-Object Q+RECT; [void][Q]::GetWindowRect($h, [ref]$r)
[void][Q]::SetCursorPos($r.R - 24, $r.T + 15); Start-Sleep -Milliseconds 400; [Q]::mouse_event(2,0,0,0,[UIntPtr]::Zero); [Q]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
Start-Sleep -Seconds 4
Write-Host "run 1 exited after one click: $($p.HasExited)"
Write-Host "saved file: $(Get-Content .m0-data\divider.ratio -ErrorAction SilentlyContinue)"
Get-Process m0_spike -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue

Write-Host '=== run 2: restart, no mouse input'
$p, $h = Start-App 'run2'
Get-Content persist-run2.out | Where-Object { $_ -match 'divider position|relayout' } | Select-Object -First 3 | ForEach-Object { Write-Host $_ }
Shot $h 'run2-restarted'
Get-Process m0_spike -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Remove-Item persist-*.out, persist-*.err -ErrorAction SilentlyContinue
Pop-Location
