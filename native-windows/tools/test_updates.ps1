$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot 'update-core.psm1') -Force
Add-Type -AssemblyName System.IO.Compression.FileSystem
Add-Type -AssemblyName System.IO.Compression
$Temporary = Join-Path ([IO.Path]::GetTempPath()) ('lingomate-update-test-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $Temporary | Out-Null
$script:Passed = 0
function Check([bool]$Condition, [string]$Name) {
    if (-not $Condition) { throw "Failed: $Name" }
    $script:Passed++
}
function Reject([scriptblock]$Operation, [string]$Name) {
    $Rejected = $false
    try { & $Operation | Out-Null } catch { $Rejected = $true }
    Check $Rejected $Name
}
function Fixture([string]$Directory, [string]$Version, [switch]$Legacy) {
    New-Item -ItemType Directory -Path $Directory | Out-Null
    $Files = @('lingomate_tsf.dll', 'bilingual-ime-bridge.exe', 'dict.tsv', 'glossary-en.tsv', 'details.json', 'LICENSE', 'GLOSSARY-NOTICE.md', 'LEXICON-NOTICE.md')
    if (-not $Legacy) { $Files += @('update.ps1', 'update-worker.ps1', 'update-core.psm1', 'version.json') }
    foreach ($Name in $Files) { [IO.File]::WriteAllText((Join-Path $Directory $Name), "synthetic-$Version-$Name") }
    if (-not $Legacy) {
        Write-LingoMateJson @{ schema = 1; version = $Version; arch = 'x64'; channel = 'preview'; repository = 'jizw0704-source/lingomate'; minimum_windows_build = 22000 } (Join-Path $Directory 'version.json')
    }
    $Manifest = [ordered]@{}
    foreach ($Name in ($Files | Sort-Object)) { $Manifest[$Name] = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $Directory $Name)).Hash }
    Write-LingoMateJson $Manifest (Join-Path $Directory 'manifest.json')
}
function Zip([string]$Path, [string]$Directory, [string]$Extra = '') {
    $Stream = [IO.File]::Open($Path, [IO.FileMode]::CreateNew)
    $Archive = $null
    try {
        $Archive = New-Object IO.Compression.ZipArchive($Stream, [IO.Compression.ZipArchiveMode]::Create)
        foreach ($File in Get-ChildItem -LiteralPath $Directory -File) { [IO.Compression.ZipFileExtensions]::CreateEntryFromFile($Archive, $File.FullName, $File.Name) | Out-Null }
        if ($Extra) {
            $Entry = $Archive.CreateEntry($Extra)
            $Writer = New-Object IO.StreamWriter($Entry.Open())
            $Writer.Write('synthetic'); $Writer.Dispose()
        }
    } finally { if ($Archive) { $Archive.Dispose() }; $Stream.Dispose() }
}
try {
    Check ((Get-LingoMateVersion '0.10.0') -gt (Get-LingoMateVersion '0.9.9')) 'numeric version ordering'
    foreach ($Bad in @('1.2', '01.2.3', '1.2.3-beta', '../1.2.3', '999999999999.1.1')) { Reject { Get-LingoMateVersion $Bad } "invalid version $Bad" }
    foreach ($Bad in @('http://github.com/jizw0704-source/lingomate/releases/download/x/a.zip', 'https://evil.example/a', 'https://github.com/other/repo/releases/download/x/a.zip', 'https://github.com.evil.example/a', 'https://github.com:444/jizw0704-source/lingomate/releases/download/x/a.zip')) { Reject { Assert-LingoMateUrl $Bad } 'untrusted URL rejected' }
    Reject { Assert-LingoMateUrl 'https://evil.example/a' -Redirect } 'untrusted redirect rejected'
    Check ((Assert-LingoMateUrl 'https://release-assets.githubusercontent.com/test' -Redirect).Scheme -eq 'https') 'approved CDN redirect'
    $Empty = [pscustomobject]@{schema=1; platform="windows"; arch="x64"; channel="preview"; minimum_windows_build=22000; status="unpublished"}
    Check ($null -eq (Assert-LingoMateUpdate $Empty "0.1.1")) "unpublished feed is an empty state"
    $Package = Join-Path $Temporary 'package'
    Fixture $Package '0.2.0'
    Check ((Test-LingoMatePackage $Package).PSObject.Properties.Name.Count -eq 12) 'complete package validation'
    $Archive = Join-Path $Temporary 'valid.zip'
    Zip $Archive $Package
    $Metadata = [pscustomobject]@{
        status = 'published'; notes = 'Synthetic notes'; download_url = 'https://github.com/jizw0704-source/lingomate/releases/download/windows-v0.2.0/lingomate-windows-x64-0.2.0.zip'; schema = 1; platform = 'windows'; arch = 'x64'; channel = 'preview'; version = '0.2.0'; minimum_windows_build = 22000
        asset = 'lingomate-windows-x64-0.2.0.zip'; size = (Get-Item $Archive).Length
        sha256 = (Get-FileHash $Archive -Algorithm SHA256).Hash.ToLowerInvariant()
        manifest_sha256 = (Get-FileHash (Join-Path $Package 'manifest.json') -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    Check ((Assert-LingoMateUpdate $Metadata "0.1.1").metadata.version -eq "0.2.0") "published update selected"
    Check ($null -eq (Assert-LingoMateUpdate $Metadata "0.2.0")) "equal version does not update"
    Check ($null -eq (Assert-LingoMateUpdate $Metadata "0.3.0")) "automatic downgrade rejected"
    $Metadata.arch = "arm64"; Reject { Assert-LingoMateUpdate $Metadata "0.1.1" } "wrong architecture"; $Metadata.arch = "x64"
    $Metadata.platform = "macos"; Reject { Assert-LingoMateUpdate $Metadata "0.1.1" } "wrong platform"; $Metadata.platform = "windows"
    $Extracted = Join-Path $Temporary 'extracted'
    Expand-LingoMateUpdate $Archive $Extracted $Metadata
    Check (Test-Path (Join-Path $Extracted 'version.json')) 'verified extraction'
    $SavedHash = $Metadata.sha256
    $Metadata.sha256 = '0' * 64
    Reject { Expand-LingoMateUpdate $Archive (Join-Path $Temporary 'wrong-hash') $Metadata } 'archive mismatch before extraction'
    $Metadata.sha256 = $SavedHash
    foreach ($BadName in @('../outside.exe', 'LINGOMATE_TSF.DLL', 'unknown.exe', 'dir/update.ps1')) {
        $BadZip = Join-Path $Temporary ([Guid]::NewGuid().ToString('N') + '.zip')
        Zip $BadZip $Package $BadName
        $Metadata.size = (Get-Item $BadZip).Length
        $Metadata.sha256 = (Get-FileHash $BadZip -Algorithm SHA256).Hash.ToLowerInvariant()
        Reject { Expand-LingoMateUpdate $BadZip (Join-Path $Temporary ([Guid]::NewGuid().ToString('N'))) $Metadata } 'unsafe or duplicate archive entry'
    }
    Check (-not (Test-Path (Join-Path $Temporary 'outside.exe'))) 'path traversal did not write outside'

    $Root = Join-Path $Temporary 'installed'
    New-Item -ItemType Directory -Path $Root | Out-Null
    $Old = Join-Path $Root '0.1.0-legacy'
    Fixture $Old '0.1.0' -Legacy
    $OldDll = Join-Path $Old 'lingomate_tsf.dll'
    $Calls = New-Object 'System.Collections.Generic.List[string]'
    $MockRegister = { param($Dll, $Remove) $Calls.Add("$Dll|$Remove") }.GetNewClosure()
    $New = Install-LingoMatePackage $Package $Root $OldDll $MockRegister
    Check ($Calls.Count -eq 1 -and $Calls[0] -eq ((Join-Path $New 'lingomate_tsf.dll') + '|False')) 'only owned new registration attempted'
    Check (Test-Path $OldDll) 'old version retained'
    Check ((Read-LingoMateJson (Join-Path $New 'install-receipt.json')).previous -eq $OldDll) 'rollback receipt retained'
    Restore-LingoMateVersion $New $Root $MockRegister | Out-Null
    Check ($Calls[1] -eq ($OldDll + '|False')) 'explicit rollback uses checked previous package'
    $Calls.Clear()
    $Retry = Install-LingoMatePackage $Package $Root (Join-Path $New 'lingomate_tsf.dll') $MockRegister
    Check ($Retry -eq $New -and $Calls.Count -eq 0) 'same installed package is idempotent'

    $Failing = { param($Dll, $Remove) $Calls.Add("$Dll|$Remove"); if ($Dll -ne $OldDll) { throw 'Synthetic registration failure' } }.GetNewClosure()
    Reject { Install-LingoMatePackage $Package $Root $OldDll $Failing } 'registration failure surfaced'
    Check ($Calls.Count -eq 2 -and $Calls[1] -eq ($OldDll + '|False')) 'registration failure restores previous DLL'
    Check (Test-Path $OldDll) 'failure leaves previous files'
    $Foreign = Join-Path $Temporary 'lingomate_tsf.dll'; [IO.File]::WriteAllText($Foreign, 'synthetic')
    Reject { Install-LingoMatePackage $Package $Root $Foreign $MockRegister } 'foreign registration preserved'
    [IO.File]::AppendAllText((Join-Path $Package 'update.ps1'), 'tampered')
    Reject { Test-LingoMatePackage $Package } 'tampered package rejected'
    $Calls.Clear()
    Reject { Install-LingoMatePackage $Package $Root $OldDll $MockRegister } 'tampering cannot reach registration'
    Check ($Calls.Count -eq 0) 'no registration for invalid package'
    Write-Output "Passed $script:Passed isolated update checks; no network or system registration was used."
} catch { Write-Output ($_ | Out-String); throw }
finally { Remove-Item -LiteralPath $Temporary -Recurse -Force }
