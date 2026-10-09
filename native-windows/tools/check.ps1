$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$Root = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$Manifest = Join-Path $Root 'native-windows/Cargo.toml'
foreach ($Script in Get-ChildItem -LiteralPath $PSScriptRoot | Where-Object { $_.Extension -in @('.ps1', '.psm1') }) {
    $Tokens = $null; $ParseErrors = $null
    [System.Management.Automation.Language.Parser]::ParseFile($Script.FullName, [ref]$Tokens, [ref]$ParseErrors) | Out-Null
    if ($ParseErrors.Count -gt 0) { throw ($ParseErrors | Out-String) }
}
& cargo +1.96.0 fmt --manifest-path $Manifest -- --check
if ($LASTEXITCODE -ne 0) { throw 'Rust format check failed.' }
& cargo +1.96.0 clippy --manifest-path $Manifest --locked --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { throw 'Rust lint failed.' }
$env:LINGOMATE_TEST_RESOURCES = Join-Path $Root 'native-windows/package/windows-x64'
$env:LINGOMATE_TEST_BRIDGE = Join-Path $env:LINGOMATE_TEST_RESOURCES 'bilingual-ime-bridge.exe'
& cargo +1.96.0 test --manifest-path $Manifest --locked
if ($LASTEXITCODE -ne 0) { throw 'Real-engine integration tests failed.' }
& python (Join-Path $PSScriptRoot 'test_dll.py') (Join-Path $Root 'native-windows/package/windows-x64/lingomate_tsf.dll')
if ($LASTEXITCODE -ne 0) { throw 'Native DLL factory test failed.' }
& (Join-Path $PSScriptRoot 'test_updates.ps1')
if (-not $?) { throw 'Update isolation checks failed.' }
$LegacyShell = Join-Path $env:WINDIR 'System32/WindowsPowerShell/v1.0/powershell.exe'
& $LegacyShell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'test_updates.ps1')
if ($LASTEXITCODE -ne 0) { throw 'Windows PowerShell 5.1 update checks failed.' }
$Preview = Join-Path $Root 'native-windows/package/update-preview.png'
& $LegacyShell -NoProfile -STA -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'update.ps1') -SmokeTest -PreviewPath $Preview
if ($LASTEXITCODE -ne 0) { throw 'Windows update UI smoke check failed.' }
