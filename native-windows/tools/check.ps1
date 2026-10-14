param([switch]$SkipUpdates, [string]$WordNetSource)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$Root = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$Manifest = Join-Path $Root 'native-windows/Cargo.toml'
[string[]]$WordNetArguments = if ($WordNetSource) { @('--wordnet-source', (Resolve-Path -LiteralPath $WordNetSource).Path) } else { @('--wordnet-in-memory') }
$TestEnvironment = @{}
foreach ($Name in @('LINGOMATE_TEST_SESSION', 'LINGOMATE_TEST_RESOURCES', 'LINGOMATE_TEST_BRIDGE', 'LINGOMATE_TEST_CORE')) {
    $TestEnvironment[$Name] = [Environment]::GetEnvironmentVariable($Name, 'Process')
}
try {
$env:LINGOMATE_TEST_SESSION = [Guid]::NewGuid().ToString('N')
& python (Join-Path $Root 'tools/test_part_of_speech.py')
if ($LASTEXITCODE -ne 0) { throw 'POS provenance tests failed.' }
& python (Join-Path $Root 'tools/test_frequency.py')
if ($LASTEXITCODE -ne 0) { throw 'Frequency permission tests failed.' }
& python (Join-Path $Root 'tools/test_prepare_lexicon.py')
if ($LASTEXITCODE -ne 0) { throw 'Data provenance tests failed.' }
& python (Join-Path $Root 'tools/prepare_lexicon.py') --check @WordNetArguments --package (Join-Path $Root 'native-windows/package/windows-x64')
if ($LASTEXITCODE -ne 0) { throw 'Pinned data/package verification failed; see the source, network or checksum error above.' }
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
$env:LINGOMATE_TEST_CORE = Join-Path $env:LINGOMATE_TEST_RESOURCES 'lingomate_engine.dll'
& python (Join-Path $Root 'prototype/tests/engine_test.py')
if ($LASTEXITCODE -ne 0) { throw 'Pinned data candidate quality regression failed.' }
foreach ($Test in @('ranking_test.py', 'memory_test.py')) {
    & python (Join-Path $Root ('prototype/tests/' + $Test))
    if ($LASTEXITCODE -ne 0) { throw 'Shared ranking/memory regression failed.' }
}
$BridgeManifest = Join-Path $Root 'prototype/bridge/Cargo.toml'
& cargo +1.96.0 fmt --manifest-path $BridgeManifest -- --check
if ($LASTEXITCODE -ne 0) { throw 'Bridge format check failed.' }
& cargo +1.96.0 clippy --manifest-path $BridgeManifest --locked --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { throw 'Bridge lint failed.' }
& cargo +1.96.0 test --manifest-path $BridgeManifest --locked
if ($LASTEXITCODE -ne 0) { throw 'Shared CLI/core compatibility tests failed.' }
& cargo +1.96.0 test --manifest-path $Manifest --locked
if ($LASTEXITCODE -ne 0) { throw 'Real-engine integration tests failed.' }
& python (Join-Path $PSScriptRoot 'test_dll.py') (Join-Path $Root 'native-windows/package/windows-x64/lingomate_tsf.dll')
if ($LASTEXITCODE -ne 0) { throw 'Native DLL factory test failed.' }
& (Join-Path $PSScriptRoot 'test_tsf_host.ps1')
if (-not $SkipUpdates) { & (Join-Path $PSScriptRoot 'check-updates.ps1') }
} finally {
    foreach ($Name in $TestEnvironment.Keys) {
        [Environment]::SetEnvironmentVariable($Name, $TestEnvironment[$Name], 'Process')
    }
}
