param(
    [string]$Package = (Join-Path (Split-Path $PSScriptRoot -Parent) 'package/windows-x64'),
    [ValidateSet('sta', 'mta', 'comless')]
    [string[]]$Apartments = @('sta', 'mta', 'comless')
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not [Environment]::Is64BitProcess) { throw 'The synthetic host check requires x64 PowerShell.' }
Import-Module (Join-Path $PSScriptRoot 'update-core.psm1') -Force
$Package = (Resolve-Path -LiteralPath $Package).Path
$Manifest = Test-LingoMatePackage $Package
if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
    $VsWhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    if (-not (Test-Path -LiteralPath $VsWhere)) { throw 'Run this test from an x64 Visual Studio Developer PowerShell.' }
    $VsPath = (& $VsWhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath | Select-Object -First 1)
    if (-not $VsPath) { throw 'MSVC C++ tools are required for the synthetic host test.' }
    Import-Module (Join-Path $VsPath 'Common7/Tools/Microsoft.VisualStudio.DevShell.dll')
    Enter-VsDevShell -VsInstallPath $VsPath -SkipAutomaticLocation -Arch amd64 -HostArch amd64 | Out-Null
}
$Temporary = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) ('lingomate-tsf-host-' + [Guid]::NewGuid().ToString('N'))))
$OwnedTemporary = $Temporary
New-Item -ItemType Directory -Path $Temporary | Out-Null
try {
    # Test an immutable temporary copy, allowing another build to replace its
    # generated output without touching the DLLs loaded by this fixture.
    $FixturePackage = Join-Path $Temporary 'package'
    New-Item -ItemType Directory -Path $FixturePackage | Out-Null
    foreach ($Name in @($Manifest.PSObject.Properties.Name) + 'manifest.json') {
        Copy-Item -LiteralPath (Join-Path $Package $Name) -Destination (Join-Path $FixturePackage $Name)
    }
    Test-LingoMatePackage $FixturePackage | Out-Null
    $Dll = Join-Path $FixturePackage 'lingomate_tsf.dll'
    Write-Output ('Synthetic TSF package SHA-256: ' + (Get-FileHash -LiteralPath (Join-Path $FixturePackage 'manifest.json') -Algorithm SHA256).Hash)
    $Exe = Join-Path $Temporary 'tsf_host.exe'
    $Object = Join-Path $Temporary 'tsf_host.obj'
    $Source = Join-Path (Split-Path $PSScriptRoot -Parent) 'tests/support/tsf_host.cpp'
    & cl.exe /nologo /std:c++17 /EHsc /utf-8 /W4 $Source ('/Fe:' + $Exe) ('/Fo:' + $Object) /link ole32.lib oleaut32.lib user32.lib gdi32.lib uuid.lib
    if ($LASTEXITCODE -ne 0) { throw 'Synthetic TSF host compilation failed.' }
    foreach ($Apartment in $Apartments) {
        $LocalData = Join-Path $Temporary ('local-data-' + $Apartment)
        New-Item -ItemType Directory -Path $LocalData | Out-Null
        Write-Output ('Owned TSF context mode: ' + $Apartment)
        & $Exe $LocalData $Dll $Apartment
        if ($LASTEXITCODE -ne 0) { throw ('Synthetic TSF host failed in ' + $Apartment + ' mode. Exit code: ' + $LASTEXITCODE) }
    }
    Write-Output 'Synthetic TSF host checks passed. No registration, global source changes, physical keys or external host text were used.'
    Write-Output 'The owned subscription shim does not validate actual ChatGPT, Codex or DingTalk profile loading and keyboard input.'
} finally {
    $TemporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    $ResolvedTemporary = [IO.Path]::GetFullPath($Temporary)
    if ($ResolvedTemporary -ine $OwnedTemporary -or -not $ResolvedTemporary.StartsWith($TemporaryRoot, [StringComparison]::OrdinalIgnoreCase) -or [IO.Path]::GetFileName($ResolvedTemporary) -notmatch '^lingomate-tsf-host-[a-f0-9]{32}$') { throw 'Unexpected temporary test directory; files preserved.' }
    Remove-Item -LiteralPath $ResolvedTemporary -Recurse -Force
}
