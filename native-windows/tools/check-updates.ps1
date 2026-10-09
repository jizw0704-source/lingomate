$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$Root = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
foreach ($Script in Get-ChildItem -LiteralPath $PSScriptRoot | Where-Object { $_.Extension -in @('.ps1', '.psm1') }) {
    $Tokens = $null; $ParseErrors = $null
    [System.Management.Automation.Language.Parser]::ParseFile($Script.FullName, [ref]$Tokens, [ref]$ParseErrors) | Out-Null
    if ($ParseErrors.Count -gt 0) { throw ($ParseErrors | Out-String) }
}
& (Join-Path $PSScriptRoot 'test_updates.ps1')
if (-not $?) { throw 'Update isolation checks failed.' }
$LegacyShell = Join-Path $env:WINDIR 'System32/WindowsPowerShell/v1.0/powershell.exe'
& $LegacyShell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'test_updates.ps1')
if ($LASTEXITCODE -ne 0) { throw 'Windows PowerShell 5.1 update checks failed.' }
$PreviewDirectory = Join-Path $Root 'native-windows/package'
New-Item -ItemType Directory -Path $PreviewDirectory -Force | Out-Null
& $LegacyShell -NoProfile -STA -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'update.ps1') -SmokeTest -PreviewPath (Join-Path $PreviewDirectory 'update-preview.png')
if ($LASTEXITCODE -ne 0) { throw 'Windows update UI smoke check failed.' }
