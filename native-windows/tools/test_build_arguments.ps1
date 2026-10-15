$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$FixturePath = Join-Path ([IO.Path]::GetTempPath()) ('lingomate-argument fixture-' + [Guid]::NewGuid().ToString('N') + '.bin')
[IO.File]::WriteAllBytes($FixturePath, [byte[]]@(0))
$Verified = 0
function Invoke-ArgumentProbe([string]$ScriptPath, [bool]$UseOverride) {
    $Observed = [Collections.Generic.List[string]]::new()
    $ExpectedStop = 'LINGOMATE_ARGUMENT_PROBE_COMPLETE'
    # These local function mocks end before any Cargo/data preparation or cleanup.
    # Only the real entry-point's parameter binding and native argument splatting run.
    function git {
        param([Parameter(ValueFromRemainingArguments = $true)][string[]]$ProbeArguments)
        & $env:ComSpec /c exit 0
        if ($ProbeArguments[-2] -eq 'rev-parse' -and $ProbeArguments[-1] -eq 'HEAD') { return 'c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e' }
        if ($ProbeArguments[-2] -eq 'status' -and $ProbeArguments[-1] -eq '--porcelain') { return }
        throw 'Unexpected Git call in argument probe.'
    }
    function python {
        param([Parameter(ValueFromRemainingArguments = $true)][string[]]$ProbeArguments)
        & $env:ComSpec /c exit 0
        $ScriptName = [IO.Path]::GetFileName($ProbeArguments[0])
        if ($ScriptName -eq 'prepare_lexicon.py') {
            foreach ($Value in $ProbeArguments[1..($ProbeArguments.Count - 1)]) { $Observed.Add($Value) }
            throw $ExpectedStop
        }
        if ($ScriptName -notin @('test_part_of_speech.py', 'test_frequency.py', 'test_prepare_lexicon.py')) { throw 'Unexpected Python call in argument probe.' }
    }
    try {
        if ($UseOverride) { & $ScriptPath -WordNetSource $FixturePath }
        else { & $ScriptPath }
        throw 'Entry point did not reach the intercepted data preparation call.'
    } catch {
        if ($_.Exception.Message -ne $ExpectedStop) { throw }
    }
    $Values = $Observed.ToArray()
    if ($UseOverride) {
        $Index = [Array]::IndexOf($Values, '--wordnet-source')
        if ($Index -lt 0 -or $Index + 1 -ge $Values.Length -or $Values[$Index + 1] -cne $FixturePath -or '--wordnet-in-memory' -in $Values) { throw 'Pinned source override was not forwarded as two intact arguments.' }
    } elseif (@($Values | Where-Object { $_ -ceq '--wordnet-in-memory' }).Count -ne 1 -or '--wordnet-source' -in $Values -or '-' -in $Values) {
        throw 'Single in-memory option was split or omitted.'
    }
}
try {
    foreach ($Name in @('build.ps1', 'check.ps1')) {
        foreach ($Override in @($false, $true)) {
            Invoke-ArgumentProbe (Join-Path $PSScriptRoot $Name) $Override
            $Verified++
        }
    }
    Write-Output ("PASS build/check argument forwarding: $Verified cases; PowerShell " + $PSVersionTable.PSVersion)
} finally {
    # Delete this one owned fixture file only; no directories, production data or upstream files.
    if (Test-Path -LiteralPath $FixturePath) { Remove-Item -LiteralPath $FixturePath -Force }
}
