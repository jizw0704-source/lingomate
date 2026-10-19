$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot 'update-core.psm1') -Force
Add-Type -AssemblyName System.IO.Compression.FileSystem
Add-Type -AssemblyName System.IO.Compression
$Temporary = Join-Path ([IO.Path]::GetTempPath()) ('lingomate-update-test-' + [Guid]::NewGuid().ToString('N'))
$OwnedTemporary = [IO.Path]::GetFullPath($Temporary)
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
function Fixture([string]$Directory, [string]$Version, [switch]$Legacy, [switch]$Stdio) {
    New-Item -ItemType Directory -Path $Directory | Out-Null
    $Files = @('lingomate_tsf.dll', 'bilingual-ime-bridge.exe', 'dict.tsv', 'glossary-en.tsv', 'details.json', 'LICENSE', 'GLOSSARY-NOTICE.md', 'LEXICON-NOTICE.md')
    if (-not $Legacy) { $Files += @('update.ps1', 'update-worker.ps1', 'update-core.psm1', 'version.json') }
    if (-not $Legacy -and -not $Stdio) { $Files += 'lingomate_engine.dll'; if ((Get-LingoMateVersion $Version) -ge (Get-LingoMateVersion '0.1.17')) { $Files += 'lingomate-desktop.exe' } }
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
    Check ((Test-LingoMatePackagedProcess) -is [bool]) 'package identity query is available'
    Reject { Get-LingoMateInstallRoot { $true } } 'MSIX package context cannot derive the shared install root'
    Check ((Get-LingoMateInstallRoot { $false } { param($Path) }) -eq (Join-Path $env:LOCALAPPDATA 'LingoMate/Windows')) 'unpackaged process derives the expected install root after physical validation'
    Reject { Get-LingoMateInstallRoot { $false } { param($Path) throw 'synthetic redirected view' } } 'missing package identity does not bypass physical view validation'
    Assert-LingoMatePhysicalPath $Temporary
    $PhysicalTemporary = Get-LingoMatePhysicalPath $Temporary
    Check ($PhysicalTemporary -ieq [IO.Path]::GetFullPath($Temporary)) 'real directory handle resolves to the expected test directory'
    Reject { Assert-LingoMatePhysicalPath $Temporary { param($Path) Join-Path $Path 'private-view' } } 'redirected directory handle is rejected'
    Reject { Assert-LingoMatePhysicalPath $Temporary { param($Path) '' } } 'missing physical path is rejected'
    Reject { Assert-LingoMatePhysicalPath (Join-Path $Temporary 'not-created/child') { param($Path) Join-Path $Path 'private-view' } } 'missing destination checks its existing ancestor'
    $Empty = [pscustomobject]@{schema=1; platform="windows"; arch="x64"; channel="preview"; minimum_windows_build=22000; status="unpublished"}
    Check ($null -eq (Assert-LingoMateUpdate $Empty "0.1.1")) "unpublished feed is an empty state"
    $Package = Join-Path $Temporary 'package'
    Fixture $Package '0.2.0'
    Check ((Test-LingoMatePackage $Package).PSObject.Properties.Name.Count -eq 14) 'complete package validates the engine and desktop helper'
    $MissingCore = Join-Path $Temporary 'missing-core'
    Fixture $MissingCore '0.2.0'
    Remove-Item -LiteralPath (Join-Path $MissingCore 'lingomate_engine.dll') -Force
    Reject { Test-LingoMatePackage $MissingCore } 'missing in-process engine file is rejected'
    $StdioPackage = Join-Path $Temporary 'stdio-package'
    Fixture $StdioPackage '0.1.10' -Stdio
    Reject { Test-LingoMatePackage $StdioPackage } 'new installation requires the in-process engine manifest entry'
    Check ((Test-LingoMatePackage $StdioPackage -Legacy).PSObject.Properties.Name.Count -eq 12) 'complete historical stdio package remains valid for rollback'
    $PartialLegacy = Join-Path $Temporary 'partial-stdio-package'
    Fixture $PartialLegacy '0.1.10' -Stdio
    $PartialManifest = Read-LingoMateJson (Join-Path $PartialLegacy 'manifest.json')
    $PartialManifest.PSObject.Properties.Remove('update-worker.ps1')
    Write-LingoMateJson $PartialManifest (Join-Path $PartialLegacy 'manifest.json')
    Reject { Test-LingoMatePackage $PartialLegacy -Legacy } 'partial historical package cannot pass rollback validation'
    $MixedCore = Join-Path $PartialLegacy 'lingomate_engine.dll'
    [IO.File]::WriteAllText($MixedCore, 'synthetic-mixed-core')
    $PartialManifest | Add-Member -MemberType NoteProperty -Name 'lingomate_engine.dll' -Value (Get-FileHash -Algorithm SHA256 -LiteralPath $MixedCore).Hash
    Write-LingoMateJson $PartialManifest (Join-Path $PartialLegacy 'manifest.json')
    Reject { Test-LingoMatePackage $PartialLegacy -Legacy } '12-file mixture cannot substitute engine for a historical updater file'
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
    $ProfileCalls = New-Object 'System.Collections.Generic.List[string]'
    $ProfileFixtureState = [pscustomobject]@{ Registered = $true; Enabled = $true; UserSid = 'synthetic-user' }
    $MockProfile = {
        param($Operation, $State)
        $ProfileCalls.Add($Operation)
        switch ($Operation) {
            'Snapshot' { return [pscustomobject]@{ Registered = $ProfileFixtureState.Registered; Enabled = $ProfileFixtureState.Enabled; UserSid = $ProfileFixtureState.UserSid } }
            'Install' { $ProfileFixtureState.Registered = $true; $ProfileFixtureState.Enabled = $true }
            'Uninstall' { $ProfileFixtureState.Enabled = $false }
            'Restore' { $ProfileFixtureState.Enabled = $State.Enabled }
            default { throw 'Unexpected synthetic profile operation' }
        }
    }.GetNewClosure()
    $MockRegister = { param($Dll, $Remove) $Calls.Add("$Dll|$Remove"); $ProfileFixtureState.Registered = -not $Remove; $ProfileFixtureState.Enabled = -not $Remove }.GetNewClosure()
    $RedirectedRoot = Join-Path $Temporary 'redirected-install'
    $RejectStaging = { param($Path) if ((Split-Path $Path -Leaf).StartsWith('.staging-')) { throw 'synthetic child redirected despite shared parent' }; Assert-LingoMatePhysicalPath $Path }
    Reject { Install-LingoMatePackage $Package $RedirectedRoot $null $MockRegister $MockProfile -Physical $RejectStaging } 'redirected staging cannot proceed to registration'
    Check ($Calls.Count -eq 0 -and ($ProfileCalls -join ',') -eq 'Snapshot') 'redirected staging changes no COM or profile state'
    Check (@(Get-ChildItem -LiteralPath $RedirectedRoot -Force).Count -eq 0) 'rejected redirected staging leaves no receipt or version files'
    $ProfileCalls.Clear()
    $New = Install-LingoMatePackage $Package $Root $OldDll $MockRegister $MockProfile
    Check ($Calls.Count -eq 1 -and $Calls[0] -eq ((Join-Path $New 'lingomate_tsf.dll') + '|False')) 'only owned new registration attempted'
    Check (Test-Path $OldDll) 'old version retained'
    Check ((Read-LingoMateJson (Join-Path $New 'install-receipt.json')).previous -eq $OldDll) 'rollback receipt retained'
    $OldStdio = Join-Path $Root '0.1.10-stdio'
    Fixture $OldStdio '0.1.10' -Stdio
    $OldStdioDll = Join-Path $OldStdio 'lingomate_tsf.dll'
    Write-LingoMateJson @{ previous = $OldDll } (Join-Path $OldStdio 'install-receipt.json')
    $LegacyCalls = New-Object 'System.Collections.Generic.List[string]'
    $LegacyRegister = { param($Dll, $Remove) $LegacyCalls.Add("$Dll|$Remove") }.GetNewClosure()
    Restore-LingoMateVersion $OldStdio $Root $LegacyRegister $MockProfile | Out-Null
    Check ($LegacyCalls.Count -eq 1 -and $LegacyCalls[0] -eq ($OldDll + '|False')) 'historical 12-file current version can still restore historical 8-file previous version'
    Write-LingoMateJson @{ previous = $OldStdioDll } (Join-Path $New 'install-receipt.json')
    Restore-LingoMateVersion $New $Root $LegacyRegister $MockProfile | Out-Null
    Check ($LegacyCalls.Count -eq 2 -and $LegacyCalls[1] -eq ($OldStdioDll + '|False')) 'new version can restore the checked 12-file historical version'
    $OldStdioScript = Join-Path $OldStdio 'update.ps1'
    $SavedOldStdioScript = [IO.File]::ReadAllBytes($OldStdioScript)
    [IO.File]::AppendAllText($OldStdioScript, 'tampered')
    Reject { Restore-LingoMateVersion $New $Root $LegacyRegister $MockProfile } 'tampered historical stdio package cannot be restored'
    Check ($LegacyCalls.Count -eq 2) 'historical stdio tampering cannot reach registration'
    [IO.File]::WriteAllBytes($OldStdioScript, $SavedOldStdioScript)
    Write-LingoMateJson @{ previous = $OldDll } (Join-Path $New 'install-receipt.json')
    Reject { Install-LingoMatePackage $StdioPackage $Root $OldDll $LegacyRegister $MockProfile } 'historical 12-file format cannot be installed as a new package'
    Check ($LegacyCalls.Count -eq 2) 'missing engine cannot reach registration'
    Restore-LingoMateVersion $New $Root $MockRegister $MockProfile | Out-Null
    Check ($Calls[1] -eq ($OldDll + '|False')) 'explicit rollback uses checked previous package'
    $Calls.Clear()
    $FailRestore = { param($Dll, $Remove) $Calls.Add("$Dll|$Remove"); if ($Dll -eq $OldDll) { throw 'Synthetic restore failure' } }.GetNewClosure()
    Reject { Restore-LingoMateVersion $New $Root $FailRestore $MockProfile } 'rollback failure surfaced'
    Check ($Calls.Count -eq 2 -and $Calls[1] -eq ((Join-Path $New 'lingomate_tsf.dll') + '|False')) 'rollback failure restores current registration'
    $Calls.Clear()
    $FailBoth = { param($Dll, $Remove) $Calls.Add("$Dll|$Remove"); throw 'Synthetic registration failure' }.GetNewClosure()
    Reject { Restore-LingoMateVersion $New $Root $FailBoth $MockProfile } 'dual restore failure surfaced'
    Check ((Test-Path $OldDll) -and (Test-Path (Join-Path $New 'lingomate_tsf.dll'))) 'dual restore failure preserves both versions'
    $Calls.Clear()
    $ProfileCalls.Clear()
    $Retry = Install-LingoMatePackage $Package $Root (Join-Path $New 'lingomate_tsf.dll') $MockRegister $MockProfile
    Check ($Retry -eq $New -and $Calls.Count -eq 1) 'same installed package requests owned mapping repair'
    Check (($ProfileCalls -join ',') -eq 'Snapshot,Install') 'same package retries owned user installation'
    Check ((Read-LingoMateJson (Join-Path $New 'install-receipt.json')).previous -eq $OldDll) 'same package repair retains original rollback receipt'
    $Calls.Clear()
    $CurrentScript = Join-Path $New 'update.ps1'
    $SavedCurrentScript = [IO.File]::ReadAllBytes($CurrentScript)
    [IO.File]::AppendAllText($CurrentScript, 'tampered')
    Reject { Restore-LingoMateVersion $New $Root $MockRegister $MockProfile } 'tampered current package cannot reach rollback registration'
    Check ($Calls.Count -eq 0) 'tampered current package performs no registration'
    [IO.File]::WriteAllBytes($CurrentScript, $SavedCurrentScript)

    $Failing = { param($Dll, $Remove) $Calls.Add("$Dll|$Remove"); if ($Dll -ne $OldDll) { throw 'Synthetic registration failure' } }.GetNewClosure()
    Reject { Install-LingoMatePackage $Package $Root $OldDll $Failing $MockProfile } 'registration failure surfaced'
    Check ($Calls.Count -eq 2 -and $Calls[1] -eq ($OldDll + '|False')) 'registration failure restores previous DLL'
    Check (Test-Path $OldDll) 'failure leaves previous files'

    # Compile the bindings without invoking any OS query or installation API.
    Initialize-LingoMateInputProfileApi
    $ApiType = 'LingoMate.TrustedInputProfileApi' -as [type]
    Check ($ApiType.GetField('ProfileId').GetValue($null) -ceq '0804:{078A5202-3F7A-4DD5-89CC-322A9011DF81}{5C46B182-7E32-4B6E-AB5D-2EE4F17D7695}') 'input API is limited to the single owned profile'
    $PrivateStatic = [Reflection.BindingFlags]::NonPublic -bor [Reflection.BindingFlags]::Static
    Check ($ApiType.GetField('LoadLibrarySearchSystem32', $PrivateStatic).GetValue($null) -eq 0x800) 'input DLL uses the system-only search flag'
    $Delegate = $ApiType.GetNestedType('InstallLayoutOrTipDelegate', [Reflection.BindingFlags]::NonPublic)
    $NativeInvoke = $Delegate.GetMethod('Invoke')
    $ReturnMarshal = @($NativeInvoke.ReturnParameter.GetCustomAttributes([Runtime.InteropServices.MarshalAsAttribute], $false))
    Check ($NativeInvoke.ReturnType -eq [bool] -and $ReturnMarshal.Count -eq 1 -and $ReturnMarshal[0].Value -eq [Runtime.InteropServices.UnmanagedType]::Bool) 'InstallLayoutOrTip returns native BOOL rather than HRESULT'

    $Lifecycle = New-Object 'System.Collections.Generic.List[string]'
    $TraceProfile = { param($Operation, $State) $Lifecycle.Add("profile:$Operation"); & $MockProfile $Operation $State }.GetNewClosure()
    $TraceRegister = { param($Dll, $Remove) $Lifecycle.Add("register:$Remove"); & $MockRegister $Dll $Remove }.GetNewClosure()
    $FailInstallProfile = {
        param($Operation, $State)
        $Lifecycle.Add("profile:$Operation")
        & $MockProfile $Operation $State
        if ($Operation -eq 'Install') { throw 'Synthetic input installation failure after partial application' }
    }.GetNewClosure()
    $ProfileFixtureState.Registered = $true; $ProfileFixtureState.Enabled = $true
    $Lifecycle.Clear()
    Reject { Install-LingoMatePackage $Package $Root $OldDll $TraceRegister $FailInstallProfile } 'user installation failure is surfaced'
    Check (($Lifecycle -join ',') -eq 'profile:Snapshot,register:False,profile:Install,register:False,profile:Restore') 'enabled upgrade failure restores the old DLL and never uninstalls its profile'
    Check $ProfileFixtureState.Enabled 'enabled legacy profile remains enabled after a partial installation failure'

    $ProfileFixtureState.Registered = $false; $ProfileFixtureState.Enabled = $false
    $Lifecycle.Clear()
    Reject { Install-LingoMatePackage $Package $Root '' $TraceRegister $FailInstallProfile } 'new input installation failure is surfaced'
    Check (($Lifecycle -join ',') -eq 'profile:Snapshot,register:False,profile:Install,profile:Uninstall,register:True') 'failed first installation removes only its newly introduced user item before unregistering'
    Check (-not $ProfileFixtureState.Enabled -and -not $ProfileFixtureState.Registered) 'failed first installation restores absence of the owned profile'
    Check ((Test-Path $OldDll) -and (Test-Path (Join-Path $New 'lingomate_tsf.dll'))) 'input installation failure keeps immutable version files'

    $ProfileFixtureState.Registered = $true; $ProfileFixtureState.Enabled = $true
    $Lifecycle.Clear()
    Reject { Install-LingoMatePackage $Package $Root '' $TraceRegister $FailInstallProfile } 'pre-existing profile repair failure is surfaced'
    Check (($Lifecycle -join ',') -eq 'profile:Snapshot,register:False,profile:Install,profile:Restore') 'first package retry never unregisters a pre-existing input profile'
    Check $ProfileFixtureState.Enabled 'pre-existing profile enablement survives repair failure'

    $ProfileFixtureState.Registered = $true; $ProfileFixtureState.Enabled = $false
    $Lifecycle.Clear()
    Install-LingoMatePackage $Package $Root $OldDll $TraceRegister $TraceProfile | Out-Null
    Check (($Lifecycle -join ',') -eq 'profile:Snapshot,register:False,profile:Restore') 'disabled upgrade skips InstallLayoutOrTip and restores only owned enablement'
    Check (-not $ProfileFixtureState.Enabled) 'disabled legacy profile remains disabled after upgrading'
    $Lifecycle.Clear()
    Restore-LingoMateVersion $New $Root $TraceRegister $TraceProfile | Out-Null
    Check (($Lifecycle -join ',') -eq 'profile:Snapshot,register:False,profile:Restore') 'disabled version restore never installs or uninstalls the user item'
    Check (-not $ProfileFixtureState.Enabled) 'disabled profile remains disabled after restoring the previous DLL'
    $Lifecycle.Clear()
    Install-LingoMatePackage $Package $Root (Join-Path $New 'lingomate_tsf.dll') $TraceRegister $TraceProfile | Out-Null
    Check (($Lifecycle -join ',') -eq 'profile:Snapshot,register:False,profile:Restore') 'same-package mapping repair retains a deliberately disabled profile'
    Check (-not $ProfileFixtureState.Enabled) 'same-package retry keeps disabled enablement'
    $Lifecycle.Clear()
    Install-LingoMatePackage $Package $Root '' $TraceRegister $TraceProfile | Out-Null
    Check (($Lifecycle -join ',') -eq 'profile:Snapshot,register:False,profile:Restore') 'pre-existing disabled profile without a previous owned DLL is not implicitly enabled'
    Check (-not $ProfileFixtureState.Enabled) 'pre-existing disabled repair retains its original enablement'
    Write-LingoMateJson @{ previous = $OldDll } (Join-Path $New 'install-receipt.json')
    $PartialFailRegister = {
        param($Dll, $Remove)
        $Lifecycle.Add("register:$Remove")
        & $MockRegister $Dll $Remove
        if ($Dll -ne $OldDll) { throw 'Synthetic partial registration failure' }
    }.GetNewClosure()
    $Lifecycle.Clear()
    Reject { Install-LingoMatePackage $Package $Root $OldDll $PartialFailRegister $TraceProfile } 'partial disabled upgrade failure is surfaced'
    Check (-not $ProfileFixtureState.Enabled -and ($Lifecycle -join ',') -eq 'profile:Snapshot,register:False,register:False,profile:Restore') 'partial registration cannot re-enable a previously disabled profile'
    $FailDisabledRestore = {
        param($Dll, $Remove)
        $Lifecycle.Add("register:$Remove")
        & $MockRegister $Dll $Remove
        if ($Dll -eq $OldDll) { throw 'Synthetic partial version restore failure' }
    }.GetNewClosure()
    $Lifecycle.Clear()
    Reject { Restore-LingoMateVersion $New $Root $FailDisabledRestore $TraceProfile } 'partial disabled version restore failure is surfaced'
    Check (-not $ProfileFixtureState.Enabled -and ($Lifecycle -join ',') -eq 'profile:Snapshot,register:False,register:False,profile:Restore') 'failed version restore preserves a deliberately disabled current profile'

    $ProfileFixtureState.Registered = $true; $ProfileFixtureState.Enabled = $true
    $Lifecycle.Clear()
    Reject { Install-LingoMatePackage $Package $Root (Join-Path $New 'lingomate_tsf.dll') $TraceRegister $FailInstallProfile } 'same-package user installation failure is surfaced'
    Check (($Lifecycle -join ',') -eq 'profile:Snapshot,register:False,profile:Install,register:False,profile:Restore' -and $ProfileFixtureState.Enabled) 'failed same-package repair restores registration and enabled state'
    Check ((Read-LingoMateJson (Join-Path $New 'install-receipt.json')).previous -eq $OldDll) 'failed same-package repair retains the original rollback receipt'

    $FailCleanupProfile = {
        param($Operation, $State)
        $Lifecycle.Add("profile:$Operation")
        & $MockProfile $Operation $State
        if ($Operation -in @('Install', 'Uninstall')) { throw 'Synthetic installation and cleanup failure' }
    }.GetNewClosure()
    $ProfileFixtureState.Registered = $false; $ProfileFixtureState.Enabled = $false
    $Lifecycle.Clear()
    Reject { Install-LingoMatePackage $Package $Root '' $TraceRegister $FailCleanupProfile } 'incomplete first-install cleanup is surfaced'
    Check (($Lifecycle -join ',') -eq 'profile:Snapshot,register:False,profile:Install,profile:Uninstall,register:True') 'failed user-item cleanup still attempts owned DLL unregistration'

    $ProfileFixtureState.Registered = $false; $ProfileFixtureState.Enabled = $false
    $Lifecycle.Clear()
    Install-LingoMatePackage $Package $Root '' $TraceRegister $TraceProfile | Out-Null
    Check (($Lifecycle -join ',') -eq 'profile:Snapshot,register:False,profile:Install') 'successful first installation registers before enabling the owned user item'
    Check $ProfileFixtureState.Enabled 'new owned profile becomes enabled after successful installation'
    # Restore the synthetic receipt used by the version rollback cases.
    Write-LingoMateJson @{ previous = $OldDll } (Join-Path $New 'install-receipt.json')

    $Lifecycle.Clear()
    Uninstall-LingoMateRegistration (Join-Path $New 'lingomate_tsf.dll') $Root $TraceRegister $TraceProfile
    Check (($Lifecycle -join ',') -eq 'profile:Snapshot,profile:Uninstall,register:True') 'owned user item is removed before service unregistration'
    Check (-not $ProfileFixtureState.Enabled -and (Test-Path (Join-Path $New 'lingomate_tsf.dll'))) 'uninstall disables the owned item while retaining version files'
    $FailRemove = {
        param($Dll, $Remove)
        $Lifecycle.Add("register:$Remove")
        & $MockRegister $Dll $Remove
        if ($Remove) { throw 'Synthetic partial unregistration failure' }
    }.GetNewClosure()
    foreach ($WasEnabled in @($true, $false)) {
        $ProfileFixtureState.Registered = $true; $ProfileFixtureState.Enabled = $WasEnabled
        $Lifecycle.Clear()
        Reject { Uninstall-LingoMateRegistration (Join-Path $New 'lingomate_tsf.dll') $Root $FailRemove $TraceProfile } 'unregistration failure is surfaced'
        Check (($Lifecycle -join ',') -eq 'profile:Snapshot,profile:Uninstall,register:True,register:False,profile:Restore') 'failed uninstall restores registration and prior owned enablement'
        Check ($ProfileFixtureState.Enabled -eq $WasEnabled) 'failed uninstall restores both originally enabled and originally disabled states'
    }
    $ProfileFixtureState.Registered = $false; $ProfileFixtureState.Enabled = $false
    $Lifecycle.Clear()
    Uninstall-LingoMateRegistration (Join-Path $New 'lingomate_tsf.dll') $Root $TraceRegister $TraceProfile
    Check (($Lifecycle -join ',') -eq 'profile:Snapshot,register:True') 'already absent owned profile does not block orphaned DLL unregistration'

    $InvalidProfile = { param($Operation, $State) return [pscustomobject]@{ Registered = $false; Enabled = $true; UserSid = 'synthetic-user' } }
    $Calls.Clear()
    Reject { Install-LingoMatePackage $Package $Root $OldDll $MockRegister $InvalidProfile } 'inconsistent profile snapshot is rejected before registration'
    Check ($Calls.Count -eq 0) 'invalid user snapshot cannot mutate registration'
    $ProfileFixtureState.Registered = $true; $ProfileFixtureState.Enabled = $true
    $Foreign = Join-Path $Temporary 'lingomate_tsf.dll'; [IO.File]::WriteAllText($Foreign, 'synthetic')
    Reject { Install-LingoMatePackage $Package $Root $Foreign $MockRegister $MockProfile } 'foreign registration preserved'

    $MakeClass = {
        param([string]$Dll)
        $Values = @(); $Subkeys = @()
        if ($Dll) {
            $Values = @([pscustomobject]@{ Name = ''; Kind = 'String'; Value = $Dll }, [pscustomobject]@{ Name = 'ThreadingModel'; Kind = 'String'; Value = 'Apartment' })
            $Subkeys = @('InprocServer32')
        }
        return [pscustomobject]@{ ClassExists = [bool]$Dll; InprocExists = [bool]$Dll; ClassValues = @(); InprocValues = $Values; ClassSubkeys = $Subkeys; InprocSubkeys = @() }
    }
    $ClassStore = @{ User = (& $MakeClass $OldDll); Machine = (& $MakeClass '') }
    $ClassCalls = New-Object 'System.Collections.Generic.List[string]'
    $RegistrationCalls = New-Object 'System.Collections.Generic.List[string]'
    $MockClass = {
        param($Operation, $Scope, $Dll, $State)
        $ClassCalls.Add("$Operation|$Scope")
        switch ($Operation) {
            'Read' { return $ClassStore[$Scope] }
            'Write' { $ClassStore[$Scope] = & $MakeClass $Dll }
            'Remove' { $ClassStore[$Scope] = & $MakeClass '' }
            'Restore' { $ClassStore[$Scope] = $State }
            default { throw 'Unexpected class operation' }
        }
    }.GetNewClosure()
    # Simulate old DLLs that register/unregister only HKCU.
    $LegacyRegistration = {
        param($Dll, $Remove)
        $RegistrationCalls.Add("$Dll|$Remove")
        $ClassStore.User = & $MakeClass $(if ($Remove) { '' } else { $Dll })
    }.GetNewClosure()
    $AdminContext = { return [pscustomobject]@{ UserSid = 'synthetic-user'; Is64Bit = $true; IsAdministrator = $true } }
    $UserContext = { return [pscustomobject]@{ UserSid = 'synthetic-user'; Is64Bit = $true; IsAdministrator = $false } }
    $NeverElevate = { throw 'Unexpected real elevation path in isolated test' }
    $NewDll = Join-Path $New 'lingomate_tsf.dll'
    $RegistrationParameters = @{ InstallRoot = $Root; Context = $AdminContext; Registry = $MockClass; Register = $LegacyRegistration; Elevate = $NeverElevate }
    Invoke-LingoMateRegistration $NewDll $false @RegistrationParameters
    Check ((Test-LingoMateClassMapping $ClassStore.User $NewDll) -and (Test-LingoMateClassMapping $ClassStore.Machine $NewDll)) 'legacy DLL registration gains matching owned HKCU and HKLM64 mappings'
    Invoke-LingoMateRegistration $OldDll $false @RegistrationParameters
    Check ((Test-LingoMateClassMapping $ClassStore.User $OldDll) -and (Test-LingoMateClassMapping $ClassStore.Machine $OldDll)) 'legacy rollback gains the same machine mapping as its user mapping'
    $RegistrationCalls.Clear()
    $RegistrationParameters.Context = $UserContext
    Invoke-LingoMateRegistration $OldDll $false @RegistrationParameters
    Check ($RegistrationCalls.Count -eq 0) 'consistent same-DLL mappings require neither regsvr32 nor UAC'
    $RegistrationParameters.Context = $AdminContext
    Invoke-LingoMateRegistration $OldDll $false -ForceRegistration @RegistrationParameters
    Check ($RegistrationCalls.Count -eq 1) 'missing TSF profile can force registration even when both COM mappings already match'

    $Elevations = New-Object 'System.Collections.Generic.List[string]'
    $MockElevate = {
        param($Dll, $Remove, $Sid, $InstallRoot, $Force)
        $Elevations.Add("$Sid|$Remove|$Force")
        Invoke-LingoMateRegistration $Dll $Remove -InstallRoot $InstallRoot -ExpectedUserSid $Sid -RequireAdministrator -ForceRegistration:$Force -Context $AdminContext -Registry $MockClass -Register $LegacyRegistration -Elevate $NeverElevate
    }.GetNewClosure()
    $ClassStore.Machine = & $MakeClass ''
    $RegistrationParameters.Context = $UserContext; $RegistrationParameters.Elevate = $MockElevate
    Invoke-LingoMateRegistration $OldDll $false @RegistrationParameters
    Check ($Elevations.Count -eq 1 -and $Elevations[0] -eq 'synthetic-user|False|False') 'missing machine mapping dispatches only the original user to elevation'
    Check (Test-LingoMateClassMapping $ClassStore.Machine $OldDll) 'same-DLL repair fills the missing machine mapping'
    Invoke-LingoMateRegistration $OldDll $false -ForceRegistration @RegistrationParameters
    Check ($Elevations.Count -eq 2 -and $Elevations[1] -eq 'synthetic-user|False|True') 'force-registration flag reaches the same-user elevated registration'

    $RegistrationParameters.Context = $AdminContext; $RegistrationParameters.Elevate = $NeverElevate
    $ClassCalls.Clear(); $RegistrationCalls.Clear()
    $ClassStore.Machine = & $MakeClass $Foreign
    Reject { Invoke-LingoMateRegistration $NewDll $false @RegistrationParameters } 'foreign owned-CLSID machine target is preserved'
    Check ($RegistrationCalls.Count -eq 0 -and ($ClassCalls -join ',') -eq 'Read|User,Read|Machine') 'foreign machine mapping cannot reach DLL registration or a registry write'
    $ClassStore.Machine = & $MakeClass $OldDll
    $ClassCalls.Clear()
    Reject { Invoke-LingoMateRegistration $NewDll $false -ExpectedUserSid 'other-user' @RegistrationParameters } 'another-user elevated token is rejected'
    Check ($ClassCalls.Count -eq 0) 'SID mismatch is rejected before registry access'
    $Not64 = { return [pscustomobject]@{ UserSid = 'synthetic-user'; Is64Bit = $false; IsAdministrator = $true } }
    $RegistrationParameters.Context = $Not64
    Reject { Invoke-LingoMateRegistration $NewDll $false @RegistrationParameters } '32-bit registration helper is rejected'
    $RegistrationParameters.Context = $UserContext
    Reject { Invoke-LingoMateRegistration $NewDll $false -RequireAdministrator @RegistrationParameters } 'non-admin elevated helper cannot recurse into UAC'
    $RegistrationParameters.Context = $AdminContext
    Reject { Invoke-LingoMateRegistration $NewDll $false -ExpectedManifestHash ('0' * 64) @RegistrationParameters } 'changed package manifest is rejected before registration'

    $FailureRegistry = {
        param($Operation, $Scope, $Dll, $State)
        & $MockClass $Operation $Scope $Dll $State
        if ($Operation -eq 'Write' -and $Scope -eq 'Machine') { throw 'Synthetic partial machine mapping failure' }
    }.GetNewClosure()
    $RegistrationParameters.Registry = $FailureRegistry
    $ClassCalls.Clear()
    Reject { Invoke-LingoMateRegistration $NewDll $false @RegistrationParameters } 'partial machine mapping failure is surfaced'
    Check ((Test-LingoMateClassMapping $ClassStore.User $OldDll) -and (Test-LingoMateClassMapping $ClassStore.Machine $OldDll)) 'partial write failure restores both original COM mappings'
    Check (($ClassCalls -join ',').EndsWith('Restore|User,Restore|Machine')) 'mapping recovery attempts both owned hives'
    $RegistrationParameters.Registry = $MockClass
    Invoke-LingoMateRegistration $OldDll $true @RegistrationParameters
    Check (-not $ClassStore.User.ClassExists -and -not $ClassStore.Machine.ClassExists) 'legacy DLL uninstall also removes its owned machine mapping'
    $ClassStore.User = & $MakeClass $OldDll; $ClassStore.Machine = & $MakeClass $OldDll
    $FailureRemove = {
        param($Operation, $Scope, $Dll, $State)
        & $MockClass $Operation $Scope $Dll $State
        if ($Operation -eq 'Remove' -and $Scope -eq 'Machine') { throw 'Synthetic partial machine removal failure' }
    }.GetNewClosure()
    $RegistrationParameters.Registry = $FailureRemove
    Reject { Invoke-LingoMateRegistration $OldDll $true @RegistrationParameters } 'partial machine removal failure is surfaced'
    Check ((Test-LingoMateClassMapping $ClassStore.User $OldDll) -and (Test-LingoMateClassMapping $ClassStore.Machine $OldDll)) 'failed uninstall preserves both original COM mappings'

    $Capture = @{}
    $FakeLaunch = {
        param($Request)
        $Capture.Request = $Request
        $Capture.HelperSource = [IO.File]::ReadAllText($Request.Helper)
        $Capture.ModuleSource = [IO.File]::ReadAllText((Join-Path (Split-Path $Request.Helper -Parent) 'trusted-core.psm1'))
        $Capture.HashValid = (Get-FileHash -LiteralPath (Join-Path (Split-Path $Request.Helper -Parent) 'trusted-core.psm1') -Algorithm SHA256).Hash -eq $Request.ModuleHash
        $Tokens = $null; $Errors = $null
        [Management.Automation.Language.Parser]::ParseFile($Request.Helper, [ref]$Tokens, [ref]$Errors) | Out-Null
        $Capture.ParserErrors = @($Errors).Count
        $Capture.HelperLocked = $false; $Capture.DllLocked = $false
        try { [IO.File]::WriteAllText($Request.Helper, 'synthetic tamper') } catch [IO.IOException] { $Capture.HelperLocked = $true }
        try { [IO.File]::WriteAllText($Request.Dll, 'synthetic tamper') } catch [IO.IOException] { $Capture.DllLocked = $true }
        [IO.File]::WriteAllText($Request.Result, '{"success":true,"untouched":false}')
        return [pscustomobject]@{ ExitCode = 0 }
    }.GetNewClosure()
    Invoke-LingoMateElevatedRegistration $NewDll $false 'synthetic-user' $Root -ForceRegistration -Launch $FakeLaunch
    Check ($Capture.Request.UserSid -eq 'synthetic-user' -and $Capture.Request.Force) 'fixed helper carries original SID and profile repair flag'
    Check ($Capture.HashValid -and $Capture.ParserErrors -eq 0) 'generated trusted helper and captured module parse and match their digest'
    Check ($Capture.HelperLocked -and $Capture.DllLocked) 'trusted helper and checked DLL cannot be replaced across the UAC wait'
    Check ($Capture.ModuleSource.Contains('function Invoke-LingoMateRegistration') -and -not $Capture.ModuleSource.Contains('synthetic-0.2.0-update-core')) 'helper receives the already-loaded trusted module rather than package script contents'
    Check ($Capture.HelperSource.Contains('ExpectedUserSid') -and $Capture.HelperSource.Contains('RequireAdministrator') -and $Capture.HelperSource.Contains('ExpectedManifestHash')) 'child rechecks user admin architecture and package identity'
    Check (-not (Test-Path -LiteralPath (Split-Path $Capture.Request.Helper -Parent))) 'generated helper directory is removed after checked boundary resolution'
    $CancelLaunch = { param($Request) throw (New-Object ComponentModel.Win32Exception(1223)) }
    $Untouched = $false
    try { Invoke-LingoMateElevatedRegistration $NewDll $false 'synthetic-user' $Root -Launch $CancelLaunch } catch { $Untouched = $_.Exception.Data.Contains('LingoMateRegistrationUntouched') -and $_.Exception.Data['LingoMateRegistrationUntouched'] }
    Check $Untouched 'cancelled UAC is reported as registration untouched'
    $NoResultLaunch = { param($Request) return [pscustomobject]@{ ExitCode = 1 } }
    $UnknownMutation = $false
    try { Invoke-LingoMateElevatedRegistration $NewDll $false 'synthetic-user' $Root -Launch $NoResultLaunch } catch { $UnknownMutation = -not $_.Exception.Data.Contains('LingoMateRegistrationUntouched') }
    Check $UnknownMutation 'missing child result is not falsely reported as untouched registration'

    $StoppedRegistration = {
        param($Dll, $Remove, $Force)
        $Calls.Add("$Dll|$Remove")
        $Error = New-Object InvalidOperationException('Synthetic UAC cancellation')
        $Error.Data['LingoMateRegistrationUntouched'] = $true
        throw $Error
    }.GetNewClosure()
    $Calls.Clear()
    Reject { Install-LingoMatePackage $Package $Root $OldDll $StoppedRegistration $MockProfile } 'untouched registration failure is surfaced by installation'
    Check ($Calls.Count -eq 1) 'cancelled UAC does not trigger a second rollback UAC request'
    $ProfileFixtureState.Registered = $true; $ProfileFixtureState.Enabled = $true
    $Calls.Clear()
    Reject { Uninstall-LingoMateRegistration $NewDll $Root $StoppedRegistration $MockProfile } 'cancelled unregistration is surfaced'
    Check ($Calls.Count -eq 1 -and $ProfileFixtureState.Enabled) 'uninstall cancellation restores user enablement without requesting UAC again'
    $ProfileFixtureState.Registered = $false; $ProfileFixtureState.Enabled = $false
    $RepairFlags = New-Object 'System.Collections.Generic.List[bool]'
    $ProfileRepair = { param($Dll, $Remove, $Force) $RepairFlags.Add([bool]$Force); & $MockRegister $Dll $Remove }.GetNewClosure()
    Install-LingoMatePackage $Package $Root $NewDll $ProfileRepair $MockProfile | Out-Null
    Check ($RepairFlags.Count -eq 1 -and $RepairFlags[0]) 'same-package installation with missing profile forces TSF registration'
    Write-LingoMateJson @{ previous = $OldDll } (Join-Path $New 'install-receipt.json')

    [IO.File]::AppendAllText((Join-Path $Package 'update.ps1'), 'tampered')
    Reject { Test-LingoMatePackage $Package } 'tampered package rejected'
    $Calls.Clear()
    Reject { Install-LingoMatePackage $Package $Root $OldDll $MockRegister $MockProfile } 'tampering cannot reach registration'
    Check ($Calls.Count -eq 0) 'no registration for invalid package'
    Write-Output "Passed $script:Passed isolated update checks; no network or system registration was used."
} catch { Write-Output ($_ | Out-String); throw }
finally {
    $TemporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    $ResolvedTemporary = [IO.Path]::GetFullPath($Temporary)
    if ($ResolvedTemporary -ine $OwnedTemporary -or -not $ResolvedTemporary.StartsWith($TemporaryRoot, [StringComparison]::OrdinalIgnoreCase) -or [IO.Path]::GetFileName($ResolvedTemporary) -notmatch '^lingomate-update-test-[a-f0-9]{32}$') { throw 'Unexpected temporary test directory; files preserved.' }
    Remove-Item -LiteralPath $ResolvedTemporary -Recurse -Force
}
