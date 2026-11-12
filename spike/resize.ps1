# Moves the divider with the real mouse, then resizes the window, and prints the [m0] relayout lines. Takes over the mouse briefly.
param([string]$OutDir = '', [string]$Exe = (Join-Path $PSScriptRoot 'target/debug/m0_spike.exe'))
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class R { [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
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
[void][R]::SetProcessDPIAware()
$dir = Split-Path $Exe; Push-Location $dir
Remove-Item -Recurse -Force .m0-data -ErrorAction SilentlyContinue
$p = Start-Process $Exe -PassThru -RedirectStandardOutput resize.out -RedirectStandardError resize.err
Start-Sleep 10; $p.Refresh(); $h = $p.MainWindowHandle
[void][R]::SetWindowPos($h, [IntPtr](-1), 0, 0, 0, 0, 0x43); [void][R]::SetForegroundWindow($h); Start-Sleep 1
function Lines($tag) { Write-Host "--- $tag"; Get-Content resize.out | Where-Object { $_ -match 'relayout' } | Select-Object -Last 2 | ForEach-Object { Write-Host $_ } }
function Shot($name) { if (-not $OutDir) { return }; New-Item -ItemType Directory -Force $OutDir | Out-Null; $r = New-Object R+RECT; [void][R]::GetWindowRect($h, [ref]$r); $b = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T); $g = [System.Drawing.Graphics]::FromImage($b); $g.CopyFromScreen($r.L, $r.T, 0, 0, $b.Size); $g.Dispose(); $b.Save((Join-Path $OutDir "$name.png")); $b.Dispose() }
Lines 'start'; Shot 'a-start'
# drag the divider +150px (divider is ~6px wide at the middle of the client area)
$o = New-Object R+POINT; [void][R]::ClientToScreen($h, [ref]$o); $c = New-Object R+RECT; [void][R]::GetClientRect($h, [ref]$c)
$sx = $o.X + [int]($c.R / 2); $sy = $o.Y + 300
[void][R]::SetCursorPos($sx, $sy); Start-Sleep -Milliseconds 300; [R]::mouse_event(2,0,0,0,[UIntPtr]::Zero); Start-Sleep -Milliseconds 300
foreach ($i in 1..15) { [void][R]::SetCursorPos($sx + 10 * $i, $sy); Start-Sleep -Milliseconds 40 }
[R]::mouse_event(4,0,0,0,[UIntPtr]::Zero); Start-Sleep -Seconds 1
Lines 'after dragging the divider +150'; Shot 'b-dragged'
# resize the window: narrower, then wider
$r0 = New-Object R+RECT; [void][R]::GetWindowRect($h, [ref]$r0)
[void][R]::SetWindowPos($h, [IntPtr](-1), $r0.L, $r0.T, 900, 600, 0x40); Start-Sleep -Seconds 2
Lines 'after resizing the window to 900x600'; Shot 'c-narrow'
[void][R]::SetWindowPos($h, [IntPtr](-1), $r0.L, $r0.T, 1500, 800, 0x40); Start-Sleep -Seconds 2
Lines 'after resizing the window to 1500x800'; Shot 'd-wide'
Get-Process m0_spike -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Remove-Item resize.out, resize.err -ErrorAction SilentlyContinue
Pop-Location
