param([ValidateSet('capture','click','send','measure')][string]$Action='capture', [int]$X=0, [int]$Y=0, [string]$Text='', [string]$Name='window')
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class NativeAgentWindow {
 [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
}
"@
$buildPath = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..	argetelease\letronna.exe'))
$installedPath = Join-Path $env:LOCALAPPDATA 'Programs\Letronna\Letronna.exe'
$app = Get-Process -Name letronna | Where-Object { $_.Path -eq $buildPath -or $_.Path -eq $installedPath } | Select-Object -First 1
if (-not $app) { throw 'Native Agent is not running from this project.' }
$handle = $app.MainWindowHandle
if ($Action -eq 'click') {
 $point = [IntPtr](($Y -shl 16) -bor ($X -band 65535))
 [void][NativeAgentWindow]::PostMessage($handle,0x0200,[IntPtr]::Zero,$point)
 [void][NativeAgentWindow]::PostMessage($handle,0x0201,[IntPtr]1,$point)
 [void][NativeAgentWindow]::PostMessage($handle,0x0202,[IntPtr]::Zero,$point)
} elseif ($Action -eq 'send') {
 foreach ($character in $Text.ToCharArray()) { [void][NativeAgentWindow]::PostMessage($handle,0x0102,[IntPtr][int]$character,[IntPtr]1) }
 [void][NativeAgentWindow]::PostMessage($handle,0x0100,[IntPtr]13,[IntPtr]1)
 [void][NativeAgentWindow]::PostMessage($handle,0x0101,[IntPtr]13,[IntPtr]1)
} elseif ($Action -eq 'measure') {
 $startCpu = $app.TotalProcessorTime.TotalSeconds
 Start-Sleep -Seconds 5
 $app.Refresh()
 [pscustomobject]@{WorkingSetMB=[math]::Round($app.WorkingSet64/1MB,1);PrivateMB=[math]::Round($app.PrivateMemorySize64/1MB,1);IdleCpuSeconds=[math]::Round($app.TotalProcessorTime.TotalSeconds-$startCpu,3);IntervalSeconds=5}
} else {
 $rect = New-Object NativeAgentWindow+RECT
 [void][NativeAgentWindow]::GetWindowRect($handle,[ref]$rect)
 $bitmap = New-Object Drawing.Bitmap(($rect.Right-$rect.Left),($rect.Bottom-$rect.Top))
 $graphics = [Drawing.Graphics]::FromImage($bitmap)
 $dc = $graphics.GetHdc()
 try { $ok=[NativeAgentWindow]::PrintWindow($handle,$dc,2) } finally { $graphics.ReleaseHdc($dc) }
 $destination=Join-Path $PSScriptRoot "..\artifacts\$Name.png"
 $bitmap.Save($destination,[Drawing.Imaging.ImageFormat]::Png)
 $graphics.Dispose(); $bitmap.Dispose()
 [pscustomobject]@{Captured=$ok;Path=[IO.Path]::GetFullPath($destination);Width=$rect.Right-$rect.Left;Height=$rect.Bottom-$rect.Top}
}
