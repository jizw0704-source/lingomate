param([string]$Package = (Join-Path (Split-Path $PSScriptRoot -Parent) 'package/windows-x64'))
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:PROCESSOR_ARCHITECTURE -ne 'AMD64' -or $env:PROCESSOR_ARCHITEW6432 -eq 'ARM64') { throw 'The first preview requires Intel/AMD x64 Windows.' }
if (-not [Environment]::Is64BitProcess) { throw 'Use 64-bit PowerShell.' }
Import-Module (Join-Path $PSScriptRoot 'update-core.psm1') -Force
$Package = (Resolve-Path -LiteralPath $Package).Path
$ClassKey = 'HKCU:\Software\Classes\CLSID\{078A5202-3F7A-4DD5-89CC-322A9011DF81}\InprocServer32'
$Previous = if (Test-Path -LiteralPath $ClassKey) { (Get-Item -LiteralPath $ClassKey).GetValue('') } else { $null }
$Destination = Install-LingoMatePackage $Package (Get-LingoMateInstallRoot) $Previous
Write-Output 'Installed Lingomate preview. Select it manually using the Windows input switcher.'
Write-Output 'Open Software Update from the candidate panel, or press Ctrl+Shift+U while Lingomate is active.'
Write-Output 'Other input methods, applications and ctfmon were not changed or restarted.'
