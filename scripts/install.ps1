# Copies the release build into %LOCALAPPDATA%\Programs\Letronna and adds a Start Menu shortcut,
# so "Letronna" shows up in the Windows search bar.
$ErrorActionPreference = 'Stop'
$src = Join-Path $PSScriptRoot '..\target\release\letronna.exe'
$dir = Join-Path $env:LOCALAPPDATA 'Programs\Letronna'
New-Item -ItemType Directory -Force $dir | Out-Null
Copy-Item $src (Join-Path $dir 'Letronna.exe') -Force
$lnk = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Letronna.lnk'
$s = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk)
$s.TargetPath = Join-Path $dir 'Letronna.exe'
$s.WorkingDirectory = $dir
$s.Description = 'Letronna'
$s.Save()
"Installed to $dir and Start Menu shortcut created."
