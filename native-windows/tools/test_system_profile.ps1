<#
.SYNOPSIS
Explicitly check a trusted, already installed LingoMate profile through Windows.
.DESCRIPTION
Run manually in the authorized local native testing scope; never part of default
check.ps1 or CI. A previously installed current-user profile and a matching trusted
14-file local package are required. This script does not install/register/enable a
profile, modify a COM mapping, select a global/default source, restart hosts, send
keys, focus a real application, read host text or access the clipboard.

Each isolated child observes ordinary COM or TF_TMAE_COMLESS loading with the
real Windows manager and its own empty ACP document. -OwnFocus briefly displays
and focuses only its own test window, associates that window with the empty ACP
document and restores the previous foreground HWND without reading host text.
A refused foreground request is reported as deferred, never bypassed. Hidden
mode can observe metadata but cannot accept a newly selected TIP's loading:
Windows may wait for actual thread focus even when ActivateProfile returns S_OK.
ActivateProfile uses flags=0
and explicitly restores the child's original thread profile. Its output contains
only phase/HRESULT/CLSID/IID metadata. Successful loading does not establish actual
ChatGPT, DingTalk, browser or real keyboard acceptance. COM-less here describes the
loader flag; the child initializes a private STA for its supported profile APIs.

Example: ./native-windows/tools/test_system_profile.ps1 -Package <trusted-package> -OwnFocus
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$Package,
    [switch]$OwnFocus,
    [ValidateSet('normal', 'comless')]
    [string[]]$Modes = @('normal', 'comless')
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not [Environment]::Is64BitProcess -or $env:PROCESSOR_ARCHITECTURE -ne 'AMD64' -or $env:PROCESSOR_ARCHITEW6432 -eq 'ARM64') {
    throw 'This installed-profile check requires Intel/AMD x64 Windows and x64 PowerShell.'
}
if ($Modes.Count -eq 0 -or @($Modes | Select-Object -Unique).Count -ne $Modes.Count) {
    throw 'Choose one or both distinct loading modes.'
}
Import-Module (Join-Path $PSScriptRoot 'update-core.psm1') -Force
$Package = (Resolve-Path -LiteralPath $Package).Path
$TrustedManifest = Test-LingoMatePackage $Package
if (@($TrustedManifest.PSObject.Properties.Name).Count -ne 14) { throw 'A complete current 14-file trusted package is required.' }
$TrustedDigest = (Get-FileHash -LiteralPath (Join-Path $Package 'manifest.json') -Algorithm SHA256).Hash
$Installed = Get-LingoMateRegisteredDirectory
$Dll = Join-Path $Installed 'lingomate_tsf.dll'
Assert-LingoMateOwnedDll $Dll (Get-LingoMateInstallRoot)

function Assert-InstalledProfilePackage {
    $Registered = Get-LingoMateRegisteredDirectory
    if ([IO.Path]::GetFullPath($Registered) -ine [IO.Path]::GetFullPath($Installed)) {
        throw 'The registered installation changed during this check.'
    }
    $InstalledManifest = Test-LingoMatePackage $Installed
    if (@($InstalledManifest.PSObject.Properties.Name).Count -ne 14) {
        throw 'The registered installation does not have a complete current 14-file package.'
    }
    if ((Get-FileHash -LiteralPath (Join-Path $Installed 'manifest.json') -Algorithm SHA256).Hash -ine $TrustedDigest) {
        throw 'The registered installation does not match the explicitly supplied trusted package.'
    }
    foreach ($Entry in $TrustedManifest.PSObject.Properties) {
        if ($InstalledManifest.PSObject.Properties[$Entry.Name].Value -ine $Entry.Value) {
            throw 'The installed file hashes do not match the trusted 14-file package.'
        }
    }
    # COM-less Windows may read this machine mapping directly. Missing mapping
    # must produce a real loading failure; never create it here. Refuse an
    # existing mapping to a different DLL before any activation can load it.
    $MachineKey = 'HKLM:\SOFTWARE\Classes\CLSID\{078A5202-3F7A-4DD5-89CC-322A9011DF81}\InprocServer32'
    if (Test-Path -LiteralPath $MachineKey) {
        $MachineDll = (Get-Item -LiteralPath $MachineKey).GetValue('')
        if ($MachineDll -isnot [string] -or -not $MachineDll) { throw 'The owned machine COM mapping has no valid DLL path.' }
        Assert-LingoMateOwnedDll $MachineDll (Get-LingoMateInstallRoot)
        if ([IO.Path]::GetFullPath($MachineDll) -ine [IO.Path]::GetFullPath($Dll)) {
            throw 'The owned machine COM mapping points to a different installation; no profile was activated.'
        }
    }
}
Assert-InstalledProfilePackage
Write-Output ('Trusted installed package manifest SHA-256: ' + $TrustedDigest)
if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
    $VsWhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    if (-not (Test-Path -LiteralPath $VsWhere)) { throw 'Run this check from an x64 Visual Studio Developer PowerShell.' }
    $VsPath = (& $VsWhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath | Select-Object -First 1)
    if (-not $VsPath) { throw 'MSVC C++ tools are required for this local check.' }
    Import-Module (Join-Path $VsPath 'Common7/Tools/Microsoft.VisualStudio.DevShell.dll')
    Enter-VsDevShell -VsInstallPath $VsPath -SkipAutomaticLocation -Arch amd64 -HostArch amd64 | Out-Null
}
$Temporary = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) ('lingomate-system-profile-' + [Guid]::NewGuid().ToString('N'))))
$OwnedTemporary = $Temporary
New-Item -ItemType Directory -Path $Temporary | Out-Null
try {
    $DeferredModes = @()
    $Exe = Join-Path $Temporary 'system_profile_host.exe'
    $Object = Join-Path $Temporary 'system_profile_host.obj'
    $Source = Join-Path (Split-Path $PSScriptRoot -Parent) 'tests/support/system_profile_host.cpp'
    & cl.exe /nologo /std:c++17 /EHsc /utf-8 /W4 $Source ('/Fe:' + $Exe) ('/Fo:' + $Object) /link ole32.lib oleaut32.lib user32.lib uuid.lib
    if ($LASTEXITCODE -ne 0) { throw 'The system-loading probe could not be compiled.' }
    foreach ($Mode in $Modes) {
        Assert-InstalledProfilePackage
        # The child creates this fresh directory itself and refuses an existing
        # one. It cannot read actual personal vocabulary or user configuration.
        $LocalData = Join-Path $Temporary ('local-data-' + $Mode)
        $Arguments = '"' + $Dll + '" "' + $LocalData + '"'
        if ($Mode -eq 'comless') { $Arguments += ' --comless' }
        if ($OwnFocus) { $Arguments += ' --own-focus' }
        $Start = New-Object Diagnostics.ProcessStartInfo
        $Start.FileName = $Exe
        $Start.Arguments = $Arguments
        $Start.UseShellExecute = $false
        $Start.CreateNoWindow = $true
        $Start.RedirectStandardOutput = $true
        $Start.RedirectStandardError = $true
        $Child = New-Object Diagnostics.Process
        $Child.StartInfo = $Start
        try {
            Write-Output ('Installed Windows profile loading mode: ' + $Mode)
            if (-not $Child.Start()) { throw 'The owned system-loading probe did not start.' }
            $OutputTask = $Child.StandardOutput.ReadToEndAsync()
            $ErrorTask = $Child.StandardError.ReadToEndAsync()
            if (-not $Child.WaitForExit(30000)) {
                # Only this explicitly created probe can be terminated. A
                # timeout cannot claim successful thread-profile restoration.
                $Child.Kill()
                if (-not $Child.WaitForExit(5000)) { throw 'The owned probe did not stop after its timeout; temporary files may be retained.' }
                throw 'The owned system-loading probe timed out; restoration was not verified.'
            }
            $Child.WaitForExit()
            $Output = $OutputTask.GetAwaiter().GetResult()
            $Errors = $ErrorTask.GetAwaiter().GetResult()
            $Lines = @($Output -split '\r?\n' | Where-Object { $_ })
            $MetadataLine = '^phase=[A-Za-z0-9_]+ (HRESULT=0x[0-9A-F]{8}|(CLSID|IID)=\{[0-9A-F-]{36}\})$'
            if ($Errors -or @($Lines | Where-Object { $_ -cnotmatch $MetadataLine }).Count -gt 0) {
                throw 'The probe emitted output outside the approved technical metadata format.'
            }
            foreach ($Line in $Lines) { Write-Output $Line }
            if ($Child.ExitCode -eq 3) {
                $DeferredPhases = @(
                    'hidden_profile_switch_loading_deferred_without_focused_acceptance',
                    'focused_system_loading_check_not_established',
                    'own_focus_lost_loading_check_deferred')
                if ($Lines -cnotcontains 'phase=probe_process_exit HRESULT=0x00000001' -or
                    @($DeferredPhases | Where-Object { $Lines -ccontains ('phase=' + $_ + ' HRESULT=0x00000001') }).Count -eq 0) {
                    throw 'The probe exit code did not match a verified deferred loading stage.'
                }
                $DeferredModes += $Mode
                Write-Output ('SKIP focused Windows loading acceptance: ' + $Mode + '. Actual input focus was unavailable or not requested; this is not a product loading failure.')
                Assert-InstalledProfilePackage
                continue
            }
            if ($Child.ExitCode -ne 0) { throw ('Actual Windows profile loading failed in ' + $Mode + ' mode; see the technical phases above.') }
            if (-not $OwnFocus) { throw 'An unfocused hidden probe cannot claim focused system-loading acceptance.' }
            $ActivationPhase = if ($Mode -eq 'comless') { 'COMLESS_thread_manager_activate' } else { 'ordinary_thread_manager_activate' }
            foreach ($Phase in @(
                    $ActivationPhase,
                    'activate_LingoMate_profile_current_thread_flags_zero',
                    'profile_activation_was_S_OK',
                    'associate_owned_window_with_empty_ACP_document',
                    'own_foreground_request_accepted',
                    'owned_native_window_has_actual_focus',
                    'owned_TSF_thread_has_actual_focus',
                    'owned_TSF_thread_still_has_actual_focus',
                    'active_thread_profile_matches_LingoMate',
                    'foreground_text_service_query',
                    'foreground_text_service_matches_LingoMate',
                    'focused_document_matches_fixture',
                    'owned_module_loaded_by_Windows_profile_activation',
                    'loaded_module_matches_expected_installation',
                    'loaded_module_x64_PE_metadata',
                    'owned_synthetic_document_remains_empty',
                    'restore_original_thread_profile',
                    'restored_thread_profile_identity',
                    'restore_owned_window_document_association',
                    'deactivate_owned_thread_manager',
                    'probe_process_exit')) {
                if ($Lines -cnotcontains ('phase=' + $Phase + ' HRESULT=0x00000000')) {
                    throw ('The ' + $Mode + ' probe did not confirm required stage: ' + $Phase)
                }
            }
        } finally { $Child.Dispose() }
        Assert-InstalledProfilePackage
        Write-Output ('PASS installed Windows profile load and thread-profile restoration: ' + $Mode)
    }
    if ($DeferredModes.Count) {
        throw ('Focused system-loading acceptance was not established for ' + ($DeferredModes -join ', ') + '; stages were deferred, not accepted. Use -OwnFocus only in authorized local testing.')
    }
    Write-Output 'Focused installed-profile checks passed for the requested modes. No registration, global source changes, keys or external host text were used.'
    Write-Output 'These owned-window loading checks do not establish real keyboard acceptance in ChatGPT, DingTalk or any browser.'
} finally {
    $TemporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    $ResolvedTemporary = [IO.Path]::GetFullPath($Temporary)
    if ($ResolvedTemporary -ine $OwnedTemporary -or -not $ResolvedTemporary.StartsWith($TemporaryRoot, [StringComparison]::OrdinalIgnoreCase) -or [IO.Path]::GetFileName($ResolvedTemporary) -notmatch '^lingomate-system-profile-[a-f0-9]{32}$') {
        throw 'Unexpected temporary check directory; files were preserved.'
    }
    Remove-Item -LiteralPath $ResolvedTemporary -Recurse -Force
}
