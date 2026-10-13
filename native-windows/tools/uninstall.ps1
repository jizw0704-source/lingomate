$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not [Environment]::Is64BitProcess) { throw 'Use 64-bit PowerShell.' }
$ClassKey = 'HKCU:\Software\Classes\CLSID\{078A5202-3F7A-4DD5-89CC-322A9011DF81}\InprocServer32'
if (-not (Test-Path -LiteralPath $ClassKey)) { Write-Output 'Lingomate is not registered for this user.'; return }
$Dll = (Get-Item -LiteralPath $ClassKey).GetValue('')
$Allowed = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'LingoMate/Windows')) + [IO.Path]::DirectorySeparatorChar
if (-not [IO.Path]::GetFullPath($Dll).StartsWith($Allowed, [StringComparison]::OrdinalIgnoreCase) -or [IO.Path]::GetFileName($Dll) -ne 'lingomate_tsf.dll') { throw 'Unexpected registration path; refusing removal.' }
Import-Module (Join-Path $PSScriptRoot 'update-core.psm1') -Force
Uninstall-LingoMateRegistration $Dll (Get-LingoMateInstallRoot)
Write-Output 'Lingomate unregistered. Version files remain available for rollback.'
