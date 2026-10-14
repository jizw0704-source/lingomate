<#
.SYNOPSIS
Opt-in fixed-metadata helper identity probe using an explicitly trusted installed package.
.DESCRIPTION
Without -Run this script performs no actions. With -Run it copies only the two local
test executables into a unique ignored workspace directory and launches OWN fixture
processes using Invoke-CommandInDesktopPackage. No package/app UI is activated.
No registration/removal, manifest writes, certificate or Developer Mode changes,
TIP operations, input-source changes, clipboard, configuration, credentials or
user input reads occur. Only the selected package's identity/ApplicationIds metadata
is read before and after the probe; its exact name/publisher/version/AppId are required.

Both default child policy and -PreventBreakaway are tested. The packaged parent must
match the selected full identity, then the production helper's relay/final --identity
path must report unpacked=true. A successful debugger-context probe does not accept
the real application's keyboard, focus, package VFS or translation/credential behavior.
The script is manual only; it is never invoked by check.ps1 or CI.

Primary references:
https://learn.microsoft.com/powershell/module/appx/invoke-commandindesktoppackage
https://learn.microsoft.com/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute
.EXAMPLE
./native-windows/tools/test_helper_identity.ps1 -Run -ExistingPackageName '<exact name>' -ExpectedPublisher '<exact publisher>' -ExpectedVersion '<exact version>' -AppId '<declared app id>'
#>
param(
    [switch]$Run,
    [string]$ExistingPackageName,
    [string]$ExpectedPublisher,
    [string]$ExpectedVersion,
    [string]$AppId,
    [ValidateSet('debug', 'release')]
    [string]$Build = 'debug'
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not $Run) {
    Write-Output 'Manual opt-in only. Supply -Run and exact ExistingPackageName/ExpectedPublisher/ExpectedVersion/AppId. No action was performed.'
    return
}
if (-not [Environment]::Is64BitProcess -or $env:PROCESSOR_ARCHITECTURE -ne 'AMD64' -or [Environment]::OSVersion.Version.Build -lt 19041) {
    throw 'The helper identity probe requires x64 Windows 10 build 19041 or later and x64 PowerShell.'
}
if ($ExistingPackageName -notmatch '^[A-Za-z0-9.-]{3,50}$' -or [string]::IsNullOrWhiteSpace($ExpectedPublisher) -or $ExpectedVersion -notmatch '^\d+\.\d+\.\d+\.\d+$' -or $AppId -notmatch '^[A-Za-z][A-Za-z0-9.]{0,63}$') {
    throw 'Supply exact package name, publisher, four-part version and declared AppId; wildcards are refused.'
}
Get-Command Get-AppxPackage, Get-AppxPackageManifest, Invoke-CommandInDesktopPackage -ErrorAction Stop | Out-Null
$Root = [IO.Path]::GetFullPath((Split-Path (Split-Path $PSScriptRoot -Parent) -Parent))
$Evidence = Join-Path $Root 'evidence/helper-identity'
$RunId = [Guid]::NewGuid().ToString('N')
$Work = Join-Path $Evidence $RunId
$Bin = Join-Path $Work 'bin'
$Results = Join-Path $Work 'results'
$ReportPath = Join-Path $Evidence ($RunId + '.json')
$TargetDirectory = if ($Build -eq 'release') { 'native-windows/target/x86_64-pc-windows-msvc/release' } else { 'native-windows/target/debug' }
$FixtureSource = Join-Path $Root ($TargetDirectory + '/lingomate-fixture.exe')
$DesktopSource = Join-Path $Root ($TargetDirectory + '/lingomate-desktop.exe')
$Fixture = Join-Path $Bin 'lingomate-fixture.exe'
$Desktop = Join-Path $Bin 'lingomate-desktop.exe'

function Assert-WorkspacePath([string]$Path) {
    $Resolved = [IO.Path]::GetFullPath($Path)
    if (-not $Resolved.StartsWith($Root.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Refused a path outside this workspace.'
    }
    $Current = $Resolved
    while ($Current -and $Current.StartsWith($Root, [StringComparison]::OrdinalIgnoreCase)) {
        if (Test-Path -LiteralPath $Current) {
            if ((Get-Item -LiteralPath $Current -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw 'Refused a reparse point in the probe path.'
            }
        }
        if ($Current -ieq $Root) { break }
        $Current = Split-Path $Current -Parent
    }
    return $Resolved
}
function Get-SelectedPackage {
    $Packages = @(Get-AppxPackage -Name $ExistingPackageName | Where-Object { $_.Name -ceq $ExistingPackageName })
    if ($Packages.Count -ne 1) { throw 'The exact selected package must be registered once for this user.' }
    $Package = $Packages[0]
    $Ids = @((Get-AppxPackageManifest -Package $Package.PackageFullName).Package.Applications.Application | ForEach-Object { [string]$_.Id } | Sort-Object)
    if ($Package.Publisher -cne $ExpectedPublisher -or $Package.Version.ToString() -cne $ExpectedVersion -or $AppId -cnotin $Ids) {
        throw 'Selected package publisher/version/AppId does not match the explicit trusted metadata.'
    }
    return [pscustomobject]@{
        Name = [string]$Package.Name
        PackageFullName = [string]$Package.PackageFullName
        Publisher = [string]$Package.Publisher
        Version = $Package.Version.ToString()
        ApplicationIds = $Ids
        Family = [string]$Package.PackageFamilyName
    }
}
function Stop-OwnCopies {
    foreach ($Process in @(Get-Process -Name 'lingomate-fixture', 'lingomate-desktop' -ErrorAction SilentlyContinue)) {
        try { $Path = $Process.Path } catch { continue }
        if ($Path -ine $Fixture -and $Path -ine $Desktop) { continue }
        Assert-WorkspacePath $Path | Out-Null
        # The unique copied absolute executable path identifies only this run's process.
        Stop-Process -InputObject $Process -ErrorAction Stop
        if (-not $Process.WaitForExit(5000)) { throw 'An owned probe process did not exit.' }
    }
}
function Remove-OwnWorkspace {
    Assert-WorkspacePath $Work | Out-Null
    if ([IO.Path]::GetFullPath($Work) -ine [IO.Path]::GetFullPath((Join-Path $Evidence $RunId)) -or $RunId -notmatch '^[0-9a-f]{32}$') {
        throw 'Refused cleanup outside the exact owned probe directory.'
    }
    if (-not (Test-Path -LiteralPath $Work)) { return }
    $Marker = Get-Content -LiteralPath (Join-Path $Work 'owner.json') -Raw | ConvertFrom-Json
    if ($Marker.run_id -cne $RunId -or $Marker.kind -cne 'lingomate-helper-identity') { throw 'Probe cleanup ownership marker mismatch.' }
    $Pending = New-Object 'System.Collections.Generic.Stack[string]'
    $Pending.Push($Work)
    while ($Pending.Count -gt 0) {
        foreach ($Entry in @(Get-ChildItem -LiteralPath $Pending.Pop() -Force)) {
            Assert-WorkspacePath $Entry.FullName | Out-Null
            if ($Entry.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Refused probe cleanup through a reparse point.' }
            if ($Entry.PSIsContainer) { $Pending.Push($Entry.FullName) }
        }
    }
    Remove-Item -LiteralPath $Work -Recurse -Force -ErrorAction Stop
}

$Before = Get-SelectedPackage
$BeforeJson = $Before | ConvertTo-Json -Depth 4 -Compress
foreach ($Path in @($FixtureSource, $DesktopSource, $Work, $ReportPath)) { Assert-WorkspacePath $Path | Out-Null }
foreach ($Source in @($FixtureSource, $DesktopSource)) {
    if (-not (Test-Path -LiteralPath $Source -PathType Leaf)) { throw 'Build both local test executables before this opt-in probe.' }
}
$Hashes = [ordered]@{
    fixture = (Get-FileHash -LiteralPath $FixtureSource -Algorithm SHA256).Hash
    desktop = (Get-FileHash -LiteralPath $DesktopSource -Algorithm SHA256).Hash
}
$Failure = $null
$CleanupFailure = $null
$Cases = @()
$Unchanged = $false
$OwnFilesRemoved = $false
try {
    if (Test-Path -LiteralPath $Work) { throw 'The unique owned probe directory already exists.' }
    New-Item -ItemType Directory -Path $Bin, $Results -ErrorAction Stop | Out-Null
    [IO.File]::WriteAllText((Join-Path $Work 'owner.json'), (@{run_id = $RunId; kind = 'lingomate-helper-identity'} | ConvertTo-Json -Compress), [Text.UTF8Encoding]::new($false))
    Copy-Item -LiteralPath $FixtureSource -Destination $Fixture
    Copy-Item -LiteralPath $DesktopSource -Destination $Desktop
    if ((Get-FileHash -LiteralPath $Fixture -Algorithm SHA256).Hash -cne $Hashes.fixture -or (Get-FileHash -LiteralPath $Desktop -Algorithm SHA256).Hash -cne $Hashes.desktop) { throw 'Owned executable copy hash mismatch.' }
    foreach ($Mode in @('allow', 'prevent')) {
        if ((Get-SelectedPackage | ConvertTo-Json -Depth 4 -Compress) -cne $BeforeJson) { throw 'Selected package metadata changed before the probe.' }
        $Result = Join-Path $Results ($Mode + '.json')
        $Arguments = 'identity-chain "' + $Before.PackageFullName + '" ' + $Mode + ' "' + $Result + '"'
        $Parameters = @{
            PackageFamilyName = $Before.Family
            AppId = $AppId
            Command = $Fixture
            Args = $Arguments
            ErrorAction = 'Stop'
        }
        if ($Mode -eq 'prevent') { $Parameters.PreventBreakaway = $true }
        Invoke-CommandInDesktopPackage @Parameters | Out-Null
        $Deadline = [DateTime]::UtcNow.AddSeconds(20)
        $Value = $null
        while ([DateTime]::UtcNow -lt $Deadline) {
            if (Test-Path -LiteralPath $Result -PathType Leaf) {
                try {
                    if ((Get-Item -LiteralPath $Result).Length -gt 8192) { throw 'Probe metadata exceeded its size bound.' }
                    $Value = Get-Content -LiteralPath $Result -Raw | ConvertFrom-Json
                    break
                } catch { $Value = $null }
            }
            Start-Sleep -Milliseconds 100
        }
        if ($null -eq $Value) { throw 'Owned packaged fixture did not produce bounded identity metadata in time.' }
        $Cases += $Value
        if ($Value.schema -ne 1 -or $Value.kind -cne 'existing_debug_package_helper_identity' -or $Value.mode -cne $Mode -or $Value.parent_packaged -ne $true -or $Value.parent_identity_matches -ne $true -or $Value.parent_package_full_name -cne $Before.PackageFullName -or $null -eq $Value.final_identity -or $Value.final_identity.unpacked -ne $true -or $Value.final_identity.version -cne $Value.version -or $Value.passed -ne $true) {
            throw 'Packaged parent to relay to final unpacked helper identity was not established.'
        }
        $Process = Get-Process -Id ([int]$Value.parent_pid) -ErrorAction SilentlyContinue
        if ($null -ne $Process -and $Process.Path -ieq $Fixture -and -not $Process.WaitForExit(5000)) { throw 'Owned identity fixture did not finish.' }
    }
} catch {
    $Failure = $_.Exception
} finally {
    try {
        Stop-OwnCopies
        $AfterJson = Get-SelectedPackage | ConvertTo-Json -Depth 4 -Compress
        $Unchanged = $AfterJson -ceq $BeforeJson
        if (-not $Unchanged) { throw 'Selected package metadata changed; no package restoration was attempted.' }
        Remove-OwnWorkspace
        $OwnFilesRemoved = -not (Test-Path -LiteralPath $Work)
    } catch { $CleanupFailure = $_.Exception }
}
$Summary = [ordered]@{
    schema = 1
    kind = 'existing_debug_package_helper_identity'
    run_id = $RunId
    package = $Before
    app_id = $AppId
    binary_sha256 = $Hashes
    cases = $Cases
    package_metadata_unchanged = $Unchanged
    own_temporary_files_removed = $OwnFilesRemoved
    passed = ($null -eq $Failure -and $null -eq $CleanupFailure -and $Cases.Count -eq 2 -and $Unchanged -and $OwnFilesRemoved)
    limit = 'Debugger package identity and fixed-metadata helper chain only; not real application keyboard, VFS, credentials or translation acceptance.'
}
Assert-WorkspacePath $ReportPath | Out-Null
[IO.Directory]::CreateDirectory($Evidence) | Out-Null
$Stream = [IO.File]::Open($ReportPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write)
try {
    $Bytes = [Text.UTF8Encoding]::new($false).GetBytes(($Summary | ConvertTo-Json -Depth 8))
    $Stream.Write($Bytes, 0, $Bytes.Length)
    $Stream.Flush()
} finally { $Stream.Dispose() }
Write-Output ('Identity probe metadata: ' + $ReportPath)
if ($null -ne $CleanupFailure) { throw $CleanupFailure }
if ($null -ne $Failure) { throw $Failure }
Write-Output 'Passed both packaged debugger parent policies; selected package metadata unchanged and owned temporary copies removed.'
