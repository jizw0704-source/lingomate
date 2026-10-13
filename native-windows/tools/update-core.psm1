Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$script:TrustedModuleSource = [IO.File]::ReadAllText($PSCommandPath, [Text.Encoding]::UTF8)
$script:Repository = 'jizw0704-source/lingomate'
$script:PackageFiles = @('lingomate_tsf.dll', 'bilingual-ime-bridge.exe', 'dict.tsv', 'glossary-en.tsv', 'details.json', 'LICENSE', 'GLOSSARY-NOTICE.md', 'LEXICON-NOTICE.md', 'version.json', 'update.ps1', 'update-worker.ps1', 'update-core.psm1', 'lingomate_engine.dll')

function Get-LingoMateVersion([string]$Value) {
    if ($Value -notmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$') { throw '版本格式无效。' }
    return [Version]$Value
}
function Read-LingoMateJson([string]$Path, [int]$Limit = 65536) {
    if ((Get-Item -LiteralPath $Path).Length -gt $Limit) { throw '更新信息过大。' }
    return ([IO.File]::ReadAllText($Path, [Text.Encoding]::UTF8) | ConvertFrom-Json)
}
function Write-LingoMateJson($Value, [string]$Path) {
    $Temporary = $Path + '.' + [Guid]::NewGuid().ToString('N') + '.tmp'
    $Backup = $Temporary + '.bak'
    try {
        [IO.File]::WriteAllText($Temporary, ($Value | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
        # Small local records are replaced without changing immutable version files.
        if (Test-Path -LiteralPath $Path) { [IO.File]::Replace($Temporary, $Path, $Backup) }
        else { [IO.File]::Move($Temporary, $Path) }
    } finally {
        foreach ($File in @($Temporary, $Backup)) { if (Test-Path -LiteralPath $File) { Remove-Item -LiteralPath $File -Force } }
    }
}
function Assert-LingoMateConfig($Config) {
    if ($Config.schema -ne 1 -or $Config.arch -ne 'x64' -or $Config.channel -ne 'preview' -or $Config.repository -ne $script:Repository -or $Config.minimum_windows_build -ne 22000) { throw '此版本的更新配置不受支持。' }
    Get-LingoMateVersion $Config.version | Out-Null
    return $Config
}
function Get-LingoMateInstallRoot {
    return (Join-Path $env:LOCALAPPDATA 'LingoMate/Windows')
}
function Get-LingoMateRegisteredDirectory {
    $Key = 'HKCU:\Software\Classes\CLSID\{078A5202-3F7A-4DD5-89CC-322A9011DF81}\InprocServer32'
    if (-not (Test-Path -LiteralPath $Key)) { throw '灵果尚未安装，请先完成首次安装。' }
    $Dll = (Get-Item -LiteralPath $Key).GetValue('')
    Assert-LingoMateOwnedDll $Dll (Get-LingoMateInstallRoot)
    return (Split-Path $Dll -Parent)
}
function Assert-LingoMateOwnedDll([string]$Dll, [string]$Root) {
    $Allowed = [IO.Path]::GetFullPath($Root).TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    $Full = [IO.Path]::GetFullPath($Dll)
    if (-not $Full.StartsWith($Allowed, [StringComparison]::OrdinalIgnoreCase) -or [IO.Path]::GetFileName($Full) -cne 'lingomate_tsf.dll' -or -not (Test-Path -LiteralPath $Full -PathType Leaf)) { throw '输入法登记路径不属于灵果，操作已停止。' }
}
function Assert-LingoMateUrl([string]$Url, [switch]$Redirect) {
    $Uri = [Uri]$Url
    if (-not $Uri.IsAbsoluteUri -or $Uri.Scheme -ne 'https' -or $Uri.Port -ne 443 -or $Uri.UserInfo -or $Uri.Fragment) { throw '更新地址不受信任。' }
    $Allowed = ($Uri.Host -ceq 'raw.githubusercontent.com' -and $Uri.AbsolutePath -ceq "/$script:Repository/main/updates/windows-preview.json") -or ($Uri.Host -ceq 'github.com' -and $Uri.AbsolutePath.StartsWith("/$script:Repository/releases/download/", [StringComparison]::Ordinal))
    if ($Redirect) { $Allowed = $Allowed -or ($Uri.Host -cin @('release-assets.githubusercontent.com', 'objects.githubusercontent.com')) }
    if (-not $Allowed) { throw '更新地址不属于灵果的 GitHub 发布源。' }
    return $Uri
}
function Receive-LingoMateFile([string]$Url, [string]$Path, [long]$Limit) {
    $Uri = Assert-LingoMateUrl $Url
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    $Deadline = [DateTime]::UtcNow.AddMinutes(3)
    for ($Hop = 0; $Hop -le 3; $Hop++) {
        $Request = [Net.HttpWebRequest]::Create($Uri)
        $Request.AllowAutoRedirect = $false
        $Request.Timeout = 15000
        $Request.ReadWriteTimeout = 15000
        $Request.UserAgent = 'LingoMate-Windows-Updater'
        $Request.Accept = 'application/vnd.github+json'
        $Response = $null
        try {
            $Response = $Request.GetResponse()
            $Status = [int]$Response.StatusCode
            if ($Status -in @(301, 302, 303, 307, 308)) {
                if ($Hop -eq 3) { throw '更新下载重定向过多。' }
                $Uri = Assert-LingoMateUrl ([Uri]::new($Uri, $Response.Headers['Location']).AbsoluteUri) -Redirect
                continue
            }
            if ($Status -ne 200 -or $Response.ContentLength -gt $Limit) { throw '更新响应大小或状态无效。' }
            $InputStream = $Response.GetResponseStream()
            $OutputStream = [IO.File]::Open($Path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
            try {
                $Buffer = New-Object byte[] 65536
                $Total = 0L
                while (($Count = $InputStream.Read($Buffer, 0, $Buffer.Length)) -gt 0) {
                    $Total += $Count
                    if ($Total -gt $Limit -or [DateTime]::UtcNow -gt $Deadline) { throw '更新下载超过大小或时间限制。' }
                    $OutputStream.Write($Buffer, 0, $Count)
                }
            } finally { $OutputStream.Dispose(); $InputStream.Dispose() }
            return
        } finally { if ($Response) { $Response.Dispose() }; $Request.Abort() }
    }
}
function Assert-LingoMateUpdate($Metadata, [string]$Current) {
    if ($Metadata.schema -ne 1 -or $Metadata.platform -ne 'windows' -or $Metadata.arch -ne 'x64' -or $Metadata.channel -ne 'preview' -or $Metadata.minimum_windows_build -ne 22000) { throw '更新包与当前平台不一致。' }
    $CurrentVersion = Get-LingoMateVersion $Current
    if ($Metadata.status -ceq 'unpublished') { return $null }
    if ($Metadata.status -cne 'published') { throw '更新发布状态无效。' }
    Get-LingoMateVersion $Metadata.version | Out-Null
    if ($Metadata.sha256 -cnotmatch '^[a-f0-9]{64}$' -or $Metadata.manifest_sha256 -cnotmatch '^[a-f0-9]{64}$' -or $Metadata.size -le 0 -or $Metadata.size -gt 134217728) { throw '更新校验信息无效。' }
    $Name = 'lingomate-windows-x64-' + $Metadata.version + '.zip'
    if ($Metadata.asset -cne $Name) { throw '更新包名称无效。' }
    $Url = $Metadata.download_url
    Assert-LingoMateUrl $Url | Out-Null
    $Expected = "https://github.com/$script:Repository/releases/download/windows-v$($Metadata.version)/$Name"
    if ($Url -cne $Expected) { throw '更新文件不属于当前发布版本。' }
    if ((Get-LingoMateVersion $Metadata.version) -le $CurrentVersion) { return $null }
    return [pscustomobject]@{ metadata = $Metadata; url = $Url; notes = [string]$Metadata.notes }
}
function Get-LingoMateUpdate([string]$Current, [string]$Work) {
    $Info = Join-Path $Work 'metadata.json'
    Receive-LingoMateFile "https://raw.githubusercontent.com/$script:Repository/main/updates/windows-preview.json" $Info 65536
    return (Assert-LingoMateUpdate (Read-LingoMateJson $Info) $Current)
}
function Test-LingoMatePackage([string]$Package, [switch]$Legacy) {
    $Manifest = Read-LingoMateJson (Join-Path $Package 'manifest.json')
    $Names = @($Manifest.PSObject.Properties.Name)
    # New installs require the in-process engine. Only explicit rollback may use
    # one of the two complete historical formats; partial combinations are invalid.
    $Required = if ($Legacy -and $Names.Count -eq 8) { $script:PackageFiles[0..7] }
        elseif ($Legacy -and $Names.Count -eq 12) { $script:PackageFiles[0..11] }
        else { $script:PackageFiles }
    if ($Names.Count -ne $Required.Count -or @($Names | Where-Object { $_ -cnotin $Required }).Count -gt 0) { throw '安装包文件清单不完整或包含额外文件。' }
    foreach ($Entry in $Manifest.PSObject.Properties) {
        if ($Entry.Name -cnotin $Required -or $Entry.Value -notmatch '^[a-fA-F0-9]{64}$') { throw '安装包校验清单无效。' }
        $File = Get-Item -LiteralPath (Join-Path $Package $Entry.Name)
        if ($File.PSIsContainer -or ($File.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw '安装包不能包含链接或目录。' }
        if ((Get-FileHash -Algorithm SHA256 -LiteralPath $File.FullName).Hash -ine $Entry.Value) { throw '安装文件校验失败，旧版本未修改。' }
    }
    if ($Names -contains 'version.json') { Assert-LingoMateConfig (Read-LingoMateJson (Join-Path $Package 'version.json')) | Out-Null }
    return $Manifest
}
function Expand-LingoMateUpdate([string]$Archive, [string]$Destination, $Metadata) {
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $Archive).Hash -ine $Metadata.sha256 -or (Get-Item -LiteralPath $Archive).Length -ne $Metadata.size) { throw '下载校验失败，旧版本未修改。' }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    Add-Type -AssemblyName System.IO.Compression
    $Zip = [IO.Compression.ZipFile]::OpenRead($Archive)
    try {
        $Allowed = @($script:PackageFiles) + 'manifest.json'
        $Seen = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
        $Size = 0L
        foreach ($Entry in $Zip.Entries) {
            $Size += $Entry.Length
            if ($Entry.FullName -cnotin $Allowed -or -not $Seen.Add($Entry.FullName) -or $Entry.Length -gt 134217728 -or $Size -gt 268435456 -or (($Entry.ExternalAttributes -shr 16) -band 61440) -eq 40960) { throw '更新压缩包包含无效路径、重复文件、链接或超大文件。' }
        }
        if ($Seen.Count -ne $Allowed.Count) { throw '更新压缩包不完整。' }
        New-Item -ItemType Directory -Path $Destination -ErrorAction Stop | Out-Null
        foreach ($Entry in $Zip.Entries) { [IO.Compression.ZipFileExtensions]::ExtractToFile($Entry, (Join-Path $Destination $Entry.FullName), $false) }
    } finally { $Zip.Dispose() }
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $Destination 'manifest.json')).Hash -ine $Metadata.manifest_sha256) { throw '更新文件清单校验失败。' }
    Test-LingoMatePackage $Destination | Out-Null
    $Config = Read-LingoMateJson (Join-Path $Destination 'version.json')
    if ($Config.version -cne $Metadata.version) { throw '安装包版本与发布信息不一致。' }
}
function Initialize-LingoMateInputProfileApi {
    if ('LingoMate.TrustedInputProfileApi' -as [type]) { return }
    # These are installer APIs, never called from the text service. Load only
    # the OS input.dll; InstallLayoutOrTip returns BOOL, not HRESULT.
    # https://learn.microsoft.com/windows/win32/tsf/installlayoutortip
    Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Runtime.InteropServices;
using Microsoft.Win32;
namespace LingoMate {
    public static class TrustedInputProfileApi {
        public const string ProfileId = "0804:{078A5202-3F7A-4DD5-89CC-322A9011DF81}{5C46B182-7E32-4B6E-AB5D-2EE4F17D7695}";
        private const string ProfileKey = @"SOFTWARE\Microsoft\CTF\TIP\{078A5202-3F7A-4DD5-89CC-322A9011DF81}\LanguageProfile\0x00000804\{5C46B182-7E32-4B6E-AB5D-2EE4F17D7695}";
        private const uint LoadLibrarySearchSystem32 = 0x00000800;
        private static readonly Guid Service = new Guid("078a5202-3f7a-4dd5-89cc-322a9011df81");
        private static readonly Guid Profile = new Guid("5c46b182-7e32-4b6e-ab5d-2ee4f17d7695");
        [UnmanagedFunctionPointer(CallingConvention.StdCall, CharSet = CharSet.Unicode)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private delegate bool InstallLayoutOrTipDelegate([MarshalAs(UnmanagedType.LPWStr)] string profile, uint flags);
        [UnmanagedFunctionPointer(CallingConvention.StdCall)]
        private delegate int IsEnabledDelegate(IntPtr instance, ref Guid service, ushort language, ref Guid profile, out int enabled);
        [UnmanagedFunctionPointer(CallingConvention.StdCall)]
        private delegate int EnableDelegate(IntPtr instance, ref Guid service, ushort language, ref Guid profile, int enabled);
        [DllImport("kernel32.dll", EntryPoint = "LoadLibraryExW", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern IntPtr LoadLibraryEx(string path, IntPtr reserved, uint flags);
        [DllImport("kernel32.dll", CharSet = CharSet.Ansi, ExactSpelling = true)]
        private static extern IntPtr GetProcAddress(IntPtr module, string name);
        [DllImport("kernel32.dll", ExactSpelling = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool FreeLibrary(IntPtr module);
        [DllImport("ole32.dll", ExactSpelling = true)]
        private static extern int CoInitializeEx(IntPtr reserved, uint flags);
        [DllImport("ole32.dll", ExactSpelling = true)]
        private static extern void CoUninitialize();
        [DllImport("ole32.dll", ExactSpelling = true)]
        private static extern int CoCreateInstance(ref Guid clsid, IntPtr outer, uint context, ref Guid iid, out IntPtr instance);
        private static IntPtr OpenInputDll(out InstallLayoutOrTipDelegate install) {
            IntPtr module = LoadLibraryEx(Path.Combine(Environment.SystemDirectory, "input.dll"), IntPtr.Zero, LoadLibrarySearchSystem32);
            if (module == IntPtr.Zero) throw new InvalidOperationException("System input API could not be loaded.");
            IntPtr address = GetProcAddress(module, "InstallLayoutOrTip");
            if (address == IntPtr.Zero) {
                FreeLibrary(module);
                throw new InvalidOperationException("System input installation API is unavailable.");
            }
            install = (InstallLayoutOrTipDelegate)Marshal.GetDelegateForFunctionPointer(address, typeof(InstallLayoutOrTipDelegate));
            return module;
        }
        public static void ValidateApi() {
            InstallLayoutOrTipDelegate install;
            IntPtr module = OpenInputDll(out install);
            FreeLibrary(module);
        }
        public static bool IsRegistered() {
            using (RegistryKey user = Registry.CurrentUser.OpenSubKey(ProfileKey)) {
                if (user != null) return true;
            }
            using (RegistryKey machine = Registry.LocalMachine.OpenSubKey(ProfileKey)) {
                return machine != null;
            }
        }
        private static IntPtr OpenProfiles(out bool initialized) {
            int result = CoInitializeEx(IntPtr.Zero, 2);
            initialized = result >= 0;
            if (result < 0 && result != unchecked((int)0x80010106)) Marshal.ThrowExceptionForHR(result);
            Guid factory = new Guid("33c53a50-f456-4884-b049-85fd643ecfed");
            Guid iid = new Guid("1f02b6c5-7842-4ee6-8a0b-9a24183a95ca");
            IntPtr instance;
            result = CoCreateInstance(ref factory, IntPtr.Zero, 1, ref iid, out instance);
            if (result < 0) {
                if (initialized) CoUninitialize();
                initialized = false;
                Marshal.ThrowExceptionForHR(result);
            }
            return instance;
        }
        public static bool IsEnabled() {
            if (!IsRegistered()) return false;
            bool initialized;
            IntPtr instance = OpenProfiles(out initialized);
            try {
                IntPtr method = Marshal.ReadIntPtr(Marshal.ReadIntPtr(instance), 18 * IntPtr.Size);
                IsEnabledDelegate query = (IsEnabledDelegate)Marshal.GetDelegateForFunctionPointer(method, typeof(IsEnabledDelegate));
                Guid service = Service, profile = Profile;
                int enabled;
                Marshal.ThrowExceptionForHR(query(instance, ref service, 0x0804, ref profile, out enabled));
                return enabled != 0;
            } finally {
                Marshal.Release(instance);
                if (initialized) CoUninitialize();
            }
        }
        public static void RestoreEnabled(bool enabled) {
            bool initialized;
            IntPtr instance = OpenProfiles(out initialized);
            try {
                IntPtr method = Marshal.ReadIntPtr(Marshal.ReadIntPtr(instance), 17 * IntPtr.Size);
                EnableDelegate restore = (EnableDelegate)Marshal.GetDelegateForFunctionPointer(method, typeof(EnableDelegate));
                Guid service = Service, profile = Profile;
                Marshal.ThrowExceptionForHR(restore(instance, ref service, 0x0804, ref profile, enabled ? 1 : 0));
            } finally {
                Marshal.Release(instance);
                if (initialized) CoUninitialize();
            }
        }
        public static void Install(bool remove) {
            InstallLayoutOrTipDelegate install;
            IntPtr module = OpenInputDll(out install);
            try {
                // One owned profile only. Never DEFPROFILE, DEFUSER or CLEANINSTALL.
                if (!install(ProfileId, remove ? 1u : 0u)) throw new InvalidOperationException("Input profile installation failed.");
            } finally { FreeLibrary(module); }
        }
    }
}
'@
}
function Invoke-LingoMateInputProfile([ValidateSet('Snapshot', 'Install', 'Uninstall', 'Restore')][string]$Operation, $State) {
    Initialize-LingoMateInputProfileApi
    $UserSid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    try {
        if ($Operation -eq 'Snapshot') {
            [LingoMate.TrustedInputProfileApi]::ValidateApi()
            return [pscustomobject]@{ Registered = [LingoMate.TrustedInputProfileApi]::IsRegistered(); Enabled = [LingoMate.TrustedInputProfileApi]::IsEnabled(); UserSid = $UserSid }
        }
        Assert-LingoMateInputProfileState $State
        if ($State.UserSid -cne $UserSid) { throw '输入法安装用户发生变化，操作已停止。' }
        switch ($Operation) {
            'Install' { [LingoMate.TrustedInputProfileApi]::Install($false) }
            'Uninstall' { [LingoMate.TrustedInputProfileApi]::Install($true) }
            'Restore' { [LingoMate.TrustedInputProfileApi]::RestoreEnabled($State.Enabled) }
        }
    } catch { throw '输入法用户配置未完成。原版本文件已保留，请检查系统输入设置。' }
}
function Assert-LingoMateInputProfileState($State) {
    if ($null -eq $State -or $State.Registered -isnot [bool] -or $State.Enabled -isnot [bool] -or -not ($State.UserSid -is [string]) -or -not $State.UserSid -or ($State.Enabled -and -not $State.Registered)) { throw '输入法用户配置快照无效，操作已停止。' }
}
function Get-LingoMateRegistrationContext {
    $Identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    try {
        $Principal = New-Object Security.Principal.WindowsPrincipal($Identity)
        return [pscustomobject]@{ UserSid = $Identity.User.Value; Is64Bit = [Environment]::Is64BitProcess; IsAdministrator = $Principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator) }
    } finally { $Identity.Dispose() }
}
function Invoke-LingoMateClassRegistry([ValidateSet('Read', 'Write', 'Remove', 'Restore')][string]$Operation, [ValidateSet('User', 'Machine')][string]$Scope, [string]$Dll, $State) {
    $Hive = if ($Scope -eq 'Machine') { [Microsoft.Win32.RegistryHive]::LocalMachine } else { [Microsoft.Win32.RegistryHive]::CurrentUser }
    $Base = [Microsoft.Win32.RegistryKey]::OpenBaseKey($Hive, [Microsoft.Win32.RegistryView]::Registry64)
    $Path = 'Software\Classes\CLSID\{078A5202-3F7A-4DD5-89CC-322A9011DF81}'
    try {
        if ($Operation -eq 'Read') {
            $Class = $Base.OpenSubKey($Path)
            $ClassValues = @(); $InprocValues = @(); $ClassSubkeys = @(); $InprocSubkeys = @(); $HasInproc = $false
            try {
                if ($Class) {
                    $ClassSubkeys = @($Class.GetSubKeyNames())
                    foreach ($Name in $Class.GetValueNames()) { $ClassValues += [pscustomobject]@{ Name = $Name; Kind = $Class.GetValueKind($Name).ToString(); Value = $Class.GetValue($Name, $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames) } }
                    $Inproc = $Class.OpenSubKey('InprocServer32')
                    try {
                        $HasInproc = $null -ne $Inproc
                        if ($Inproc) {
                            $InprocSubkeys = @($Inproc.GetSubKeyNames())
                            foreach ($Name in $Inproc.GetValueNames()) { $InprocValues += [pscustomobject]@{ Name = $Name; Kind = $Inproc.GetValueKind($Name).ToString(); Value = $Inproc.GetValue($Name, $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames) } }
                        }
                    } finally { if ($Inproc) { $Inproc.Dispose() } }
                }
                return [pscustomobject]@{ ClassExists = $null -ne $Class; InprocExists = $HasInproc; ClassValues = $ClassValues; InprocValues = $InprocValues; ClassSubkeys = $ClassSubkeys; InprocSubkeys = $InprocSubkeys }
            } finally { if ($Class) { $Class.Dispose() } }
        }
        if ($Operation -eq 'Write') {
            $Key = $Base.CreateSubKey($Path + '\InprocServer32')
            try {
                $Key.SetValue('', $Dll, [Microsoft.Win32.RegistryValueKind]::String)
                $Key.SetValue('ThreadingModel', 'Apartment', [Microsoft.Win32.RegistryValueKind]::String)
            } finally { $Key.Dispose() }
            return
        }
        $Base.DeleteSubKeyTree($Path, $false)
        if ($Operation -eq 'Restore' -and $State.ClassExists) {
            $Key = $Base.CreateSubKey($Path)
            try { foreach ($Value in $State.ClassValues) { $Key.SetValue($Value.Name, $Value.Value, [Microsoft.Win32.RegistryValueKind]::String) } }
            finally { $Key.Dispose() }
            if ($State.InprocExists) {
                $Key = $Base.CreateSubKey($Path + '\InprocServer32')
                try { foreach ($Value in $State.InprocValues) { $Key.SetValue($Value.Name, $Value.Value, [Microsoft.Win32.RegistryValueKind]::String) } }
                finally { $Key.Dispose() }
            }
        }
    } finally { $Base.Dispose() }
}
function Assert-LingoMateClassState($State, [string]$InstallRoot) {
    if ($null -eq $State -or $State.ClassExists -isnot [bool] -or $State.InprocExists -isnot [bool]) { throw '输入法 COM 登记快照无效。' }
    if (@($State.ClassSubkeys | Where-Object { $_ -ine 'InprocServer32' }).Count -or @($State.InprocSubkeys).Count -or @($State.ClassValues | Where-Object { $_.Name -ne '' -or $_.Kind -ne 'String' }).Count -or @($State.InprocValues | Where-Object { $_.Name -notin @('', 'ThreadingModel') -or $_.Kind -ne 'String' }).Count) { throw '灵果 CLSID 中存在非预期登记，操作已停止。' }
    $Paths = @($State.InprocValues | Where-Object { $_.Name -eq '' })
    if ($Paths.Count -gt 1) { throw '灵果 COM 登记路径无效。' }
    if ($Paths.Count -eq 1) {
        Assert-LingoMateOwnedDll ([string]$Paths[0].Value) $InstallRoot
        Test-LingoMatePackage (Split-Path ([string]$Paths[0].Value) -Parent) -Legacy | Out-Null
    } elseif (@($State.ClassValues).Count -or @($State.InprocValues).Count) { throw '灵果 CLSID 中没有可核验的自有 DLL，操作已停止。' }
}
function Test-LingoMateClassMapping($State, [string]$Dll) {
    $Paths = @($State.InprocValues | Where-Object { $_.Name -eq '' })
    $Models = @($State.InprocValues | Where-Object { $_.Name -eq 'ThreadingModel' })
    return ($State.InprocExists -and $Paths.Count -eq 1 -and $Models.Count -eq 1 -and [IO.Path]::GetFullPath([string]$Paths[0].Value) -ieq [IO.Path]::GetFullPath($Dll) -and $Models[0].Value -ceq 'Apartment')
}
function Invoke-LingoMateRegsvr32([string]$Dll, [bool]$Remove) {
    $Args = @('/s')
    if ($Remove) { $Args += '/u' }
    $Args += ('"' + $Dll + '"')
    $Process = Start-Process -FilePath (Join-Path ([Environment]::SystemDirectory) 'regsvr32.exe') -ArgumentList $Args -WindowStyle Hidden -Wait -PassThru
    try { if ($Process.ExitCode -ne 0) { throw '输入法 DLL 登记失败。' } } finally { $Process.Dispose() }
}
function ConvertTo-LingoMateProcessArgument([string]$Value) {
    if ($Value -match '["\r\n]' -or $Value.EndsWith('\')) { throw '安装辅助进程参数无效。' }
    return '"' + $Value + '"'
}
function Start-LingoMateRegistrationHelper($Request) {
    $Args = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', (ConvertTo-LingoMateProcessArgument $Request.Helper), '-Dll', (ConvertTo-LingoMateProcessArgument $Request.Dll), '-InstallRoot', (ConvertTo-LingoMateProcessArgument $Request.InstallRoot), '-ExpectedSid', (ConvertTo-LingoMateProcessArgument $Request.UserSid), '-ModuleHash', $Request.ModuleHash, '-ManifestHash', $Request.ManifestHash)
    if ($Request.Remove) { $Args += '-Remove' }
    if ($Request.Force) { $Args += '-ForceRegistration' }
    $Process = Start-Process -FilePath (Join-Path ([Environment]::SystemDirectory) 'WindowsPowerShell/v1.0/powershell.exe') -ArgumentList $Args -Verb RunAs -WindowStyle Hidden -Wait -PassThru
    try { return [pscustomobject]@{ ExitCode = $Process.ExitCode } } finally { $Process.Dispose() }
}
function Invoke-LingoMateElevatedRegistration([string]$Dll, [bool]$Remove, [string]$UserSid, [string]$InstallRoot, [switch]$ForceRegistration, [scriptblock]$Launch = { param($Request) Start-LingoMateRegistrationHelper $Request }) {
    $Work = Join-Path ([IO.Path]::GetTempPath()) ('lingomate-registration-' + [Guid]::NewGuid().ToString('N'))
    $ExpectedWork = [IO.Path]::GetFullPath($Work)
    $Leases = New-Object 'System.Collections.Generic.List[System.IDisposable]'
    $Launched = $false
    try {
        New-Item -ItemType Directory -Path $Work | Out-Null
        $Module = Join-Path $Work 'trusted-core.psm1'
        $Helper = Join-Path $Work 'register.ps1'
        [IO.File]::WriteAllText($Module, $script:TrustedModuleSource, (New-Object Text.UTF8Encoding($true)))
        $Source = @'
param([string]$Dll, [string]$InstallRoot, [string]$ExpectedSid, [string]$ModuleHash, [string]$ManifestHash, [switch]$Remove, [switch]$ForceRegistration)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$Status = @{ success = $false; untouched = $true }
$MayChangeRegistration = $false
$Code = 1
try {
    $Identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    try {
        $Principal = New-Object Security.Principal.WindowsPrincipal($Identity)
        if (-not [Environment]::Is64BitProcess -or $Identity.User.Value -cne $ExpectedSid -or -not $Principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw '需要同一 Windows 账号的 64 位管理员授权。' }
    } finally { $Identity.Dispose() }
    $Module = Join-Path $PSScriptRoot 'trusted-core.psm1'
    if ($ModuleHash -notmatch '^[A-Fa-f0-9]{64}$' -or (Get-FileHash -LiteralPath $Module -Algorithm SHA256).Hash -ine $ModuleHash) { throw '可信安装模块校验失败。' }
    Import-Module $Module -Force
    $MayChangeRegistration = $true
    Invoke-LingoMateRegistration $Dll ([bool]$Remove) -InstallRoot $InstallRoot -ExpectedUserSid $ExpectedSid -ExpectedManifestHash $ManifestHash -RequireAdministrator -ForceRegistration:$ForceRegistration
    $Status.success = $true
    $Status.untouched = $false
    $Code = 0
} catch {
    $Status.message = '管理员辅助登记未完成，版本文件已保留。'
    $Status.untouched = -not $MayChangeRegistration -or ($_.Exception.Data.Contains('LingoMateRegistrationUntouched') -and [bool]$_.Exception.Data['LingoMateRegistrationUntouched'])
}
[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'result.json'), ($Status | ConvertTo-Json -Compress), (New-Object Text.UTF8Encoding($false)))
exit $Code
'@
        [IO.File]::WriteAllText($Helper, $Source, (New-Object Text.UTF8Encoding($true)))
        $Manifest = Join-Path (Split-Path $Dll -Parent) 'manifest.json'
        $Request = [pscustomobject]@{ Helper = $Helper; Dll = $Dll; Remove = $Remove; Force = [bool]$ForceRegistration; UserSid = $UserSid; InstallRoot = $InstallRoot; ModuleHash = (Get-FileHash -LiteralPath $Module -Algorithm SHA256).Hash; ManifestHash = (Get-FileHash -LiteralPath $Manifest -Algorithm SHA256).Hash; Result = Join-Path $Work 'result.json' }
        # Keep generated trusted code and the checked DLL/manifest immutable
        # across the UAC wait. No script from the downloaded package is executed.
        foreach ($File in @($Module, $Helper, $Dll, $Manifest)) { $Leases.Add([IO.File]::Open($File, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)) }
        $Launched = $true
        $Process = & $Launch $Request
        if (-not (Test-Path -LiteralPath $Request.Result)) { throw '管理员授权被取消或辅助登记未完成，原版本文件已保留。' }
        $Result = Read-LingoMateJson $Request.Result
        if ($Process.ExitCode -ne 0 -or $Result.success -ne $true) {
            $Failure = New-Object InvalidOperationException('管理员辅助登记失败，版本文件已保留。')
            if ($Result.untouched) { $Failure.Data['LingoMateRegistrationUntouched'] = $true }
            throw $Failure
        }
    } catch {
        $Failure = $_.Exception
        $Cancelled = $false
        while ($Failure) { if ($Failure -is [ComponentModel.Win32Exception] -and $Failure.NativeErrorCode -eq 1223) { $Cancelled = $true }; $Failure = $Failure.InnerException }
        if (-not $Launched -or $Cancelled) { $_.Exception.Data['LingoMateRegistrationUntouched'] = $true }
        throw
    } finally {
        foreach ($Lease in $Leases) { $Lease.Dispose() }
        if (Test-Path -LiteralPath $Work) {
            $TempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
            $Resolved = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $Work).ProviderPath)
            if ($Resolved -ine $ExpectedWork -or -not $Resolved.StartsWith($TempRoot, [StringComparison]::OrdinalIgnoreCase) -or [IO.Path]::GetFileName($Resolved) -notmatch '^lingomate-registration-[a-f0-9]{32}$' -or ((Get-Item -LiteralPath $Resolved).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw '辅助安装临时路径发生变化，文件已保留。' }
            Remove-Item -LiteralPath $Resolved -Recurse -Force
        }
    }
}
function Invoke-LingoMateRegistration([string]$Dll, [bool]$Remove = $false, [string]$InstallRoot = (Get-LingoMateInstallRoot), [string]$ExpectedUserSid = '', [string]$ExpectedManifestHash = '', [switch]$RequireAdministrator, [switch]$ForceRegistration, [scriptblock]$Context = { Get-LingoMateRegistrationContext }, [scriptblock]$Registry = { param($Operation, $Scope, $Dll, $State) Invoke-LingoMateClassRegistry $Operation $Scope $Dll $State }, [scriptblock]$Register = { param($Dll, $Remove) Invoke-LingoMateRegsvr32 $Dll $Remove }, [scriptblock]$Elevate = { param($Dll, $Remove, $Sid, $Root, $Force) Invoke-LingoMateElevatedRegistration $Dll $Remove $Sid $Root -ForceRegistration:$Force }) {
    try {
        $Current = & $Context
        if (-not $Current.Is64Bit -or -not $Current.UserSid -or ($ExpectedUserSid -and $Current.UserSid -cne $ExpectedUserSid) -or ($RequireAdministrator -and -not $Current.IsAdministrator)) { throw '需要同一 Windows 账号的 64 位管理员授权。' }
        Assert-LingoMateOwnedDll $Dll $InstallRoot
        Test-LingoMatePackage (Split-Path $Dll -Parent) -Legacy | Out-Null
        if ($ExpectedManifestHash -and ($ExpectedManifestHash -notmatch '^[A-Fa-f0-9]{64}$' -or (Get-FileHash -LiteralPath (Join-Path (Split-Path $Dll -Parent) 'manifest.json') -Algorithm SHA256).Hash -ine $ExpectedManifestHash)) { throw '安装包在管理员授权期间发生变化，登记已停止。' }
        $UserBefore = & $Registry 'Read' 'User' $Dll $null
        $MachineBefore = & $Registry 'Read' 'Machine' $Dll $null
        Assert-LingoMateClassState $UserBefore $InstallRoot
        Assert-LingoMateClassState $MachineBefore $InstallRoot
    } catch { $_.Exception.Data['LingoMateRegistrationUntouched'] = $true; throw }
    if (-not $Remove -and -not $ForceRegistration -and (Test-LingoMateClassMapping $UserBefore $Dll) -and (Test-LingoMateClassMapping $MachineBefore $Dll)) { return }
    if (-not $Current.IsAdministrator) { & $Elevate $Dll $Remove $Current.UserSid $InstallRoot ([bool]$ForceRegistration); return }
    try {
        & $Register $Dll $Remove
        foreach ($Scope in @('User', 'Machine')) {
            if ($Remove) { & $Registry 'Remove' $Scope $Dll $null }
            else { & $Registry 'Write' $Scope $Dll $null }
        }
        foreach ($Scope in @('User', 'Machine')) {
            $After = & $Registry 'Read' $Scope $Dll $null
            if (($Remove -and $After.ClassExists) -or (-not $Remove -and -not (Test-LingoMateClassMapping $After $Dll))) { throw '输入法用户和机器 COM 登记未能保持一致。' }
        }
    } catch {
        $RecoveryFailed = $false
        foreach ($Scope in @('User', 'Machine')) {
            $Before = if ($Scope -eq 'User') { $UserBefore } else { $MachineBefore }
            try { & $Registry 'Restore' $Scope $Dll $Before } catch { $RecoveryFailed = $true }
        }
        if ($RecoveryFailed) { throw '输入法登记失败，原用户或机器 COM 映射的恢复也未完成。文件已保留。' }
        throw '输入法登记失败，原用户和机器 COM 映射已恢复，文件已保留。'
    }
}
function Install-LingoMatePackage([string]$Package, [string]$InstallRoot, [string]$PreviousDll, [scriptblock]$Register = { param($Dll, $Remove, $Force) Invoke-LingoMateRegistration $Dll $Remove -ForceRegistration:$Force }, [scriptblock]$Profile = { param($Operation, $State) Invoke-LingoMateInputProfile $Operation $State }) {
    $Manifest = Test-LingoMatePackage $Package
    $Config = Read-LingoMateJson (Join-Path $Package 'version.json')
    if ($PreviousDll) {
        Assert-LingoMateOwnedDll $PreviousDll $InstallRoot
        Test-LingoMatePackage (Split-Path $PreviousDll -Parent) -Legacy | Out-Null
    }
    $ProfileState = & $Profile 'Snapshot' $null
    Assert-LingoMateInputProfileState $ProfileState
    $Digest = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $Package 'manifest.json')).Hash
    $Destination = Join-Path $InstallRoot ($Config.version + '-' + $Digest.Substring(0, 12))
    if (-not (Test-Path -LiteralPath $Destination)) {
        New-Item -ItemType Directory -Path $InstallRoot -Force | Out-Null
        $Staging = Join-Path $InstallRoot ('.staging-' + [Guid]::NewGuid().ToString('N'))
        try {
            New-Item -ItemType Directory -Path $Staging | Out-Null
            foreach ($Name in @($Manifest.PSObject.Properties.Name) + 'manifest.json') { Copy-Item -LiteralPath (Join-Path $Package $Name) -Destination (Join-Path $Staging $Name) }
            Test-LingoMatePackage $Staging | Out-Null
            [IO.Directory]::Move($Staging, $Destination)
        } finally { if (Test-Path -LiteralPath $Staging) { Remove-Item -LiteralPath $Staging -Recurse -Force } }
    } else { Test-LingoMatePackage $Destination | Out-Null }
    $Dll = Join-Path $Destination 'lingomate_tsf.dll'
    $SameDll = $PreviousDll -and [IO.Path]::GetFullPath($PreviousDll) -ieq [IO.Path]::GetFullPath($Dll)
    # Retry both COM mappings even for the same immutable package, preserving
    # the original rollback receipt. Already-consistent mappings need no UAC.
    if (-not $SameDll) { Write-LingoMateJson @{ version = $Config.version; destination = $Destination; previous = $PreviousDll } (Join-Path $Destination 'install-receipt.json') }
    try {
        & $Register $Dll $false (-not $ProfileState.Registered)
        if ($ProfileState.Registered -and -not $ProfileState.Enabled) { & $Profile 'Restore' $ProfileState }
        else { & $Profile 'Install' $ProfileState }
    }
    catch {
        if ($_.Exception.Data.Contains('LingoMateRegistrationUntouched') -and $_.Exception.Data['LingoMateRegistrationUntouched']) { throw }
        $RecoveryFailed = $false
        if ($PreviousDll) {
            try { & $Register $PreviousDll $false $true } catch { $RecoveryFailed = $true }
            try { & $Profile 'Restore' $ProfileState } catch { $RecoveryFailed = $true }
        } elseif (-not $ProfileState.Registered) {
            # Only undo an input profile introduced by this first installation.
            # Remove the user item before unregistering its service.
            try { & $Profile 'Uninstall' $ProfileState } catch { $RecoveryFailed = $true }
            try { & $Register $Dll $true } catch { $RecoveryFailed = $true }
        } else {
            # A pre-existing profile without a previous owned DLL is not ours to
            # delete on failure. Preserve its prior enablement and version files.
            try { & $Profile 'Restore' $ProfileState } catch { $RecoveryFailed = $true }
        }
        if ($RecoveryFailed) { throw '安装未完成，原输入法用户配置或登记的恢复也未完成。版本文件已保留，请检查系统输入设置。' }
        if ($PreviousDll) { throw '安装未完成，已恢复原登记和启用状态，旧版本文件保留。' }
        throw '安装未完成，原用户启用状态已保留，版本文件保留。'
    }
    return $Destination
}
function Restore-LingoMateVersion([string]$Current, [string]$InstallRoot, [scriptblock]$Register = { param($Dll, $Remove, $Force) Invoke-LingoMateRegistration $Dll $Remove -ForceRegistration:$Force }, [scriptblock]$Profile = { param($Operation, $State) Invoke-LingoMateInputProfile $Operation $State }) {
    Assert-LingoMateOwnedDll (Join-Path $Current 'lingomate_tsf.dll') $InstallRoot
    Test-LingoMatePackage $Current -Legacy | Out-Null
    $Receipt = Read-LingoMateJson (Join-Path $Current 'install-receipt.json')
    if (-not $Receipt.previous) { throw '没有可恢复的上一版本。' }
    Assert-LingoMateOwnedDll $Receipt.previous $InstallRoot
    $Previous = Split-Path $Receipt.previous -Parent
    Test-LingoMatePackage $Previous -Legacy | Out-Null
    $ProfileState = & $Profile 'Snapshot' $null
    Assert-LingoMateInputProfileState $ProfileState
    try {
        & $Register $Receipt.previous $false (-not $ProfileState.Registered)
        if ($ProfileState.Enabled) { & $Profile 'Install' $ProfileState }
        else { & $Profile 'Restore' $ProfileState }
    }
    catch {
        if ($_.Exception.Data.Contains('LingoMateRegistrationUntouched') -and $_.Exception.Data['LingoMateRegistrationUntouched']) { throw }
        $RecoveryFailed = $false
        try { & $Register (Join-Path $Current 'lingomate_tsf.dll') $false $true } catch { $RecoveryFailed = $true }
        try { & $Profile 'Restore' $ProfileState } catch { $RecoveryFailed = $true }
        if ($RecoveryFailed) { throw '上一版本恢复失败，当前登记或启用状态也未恢复。两份文件均保留，请检查系统输入设置。' }
        throw '上一版本恢复失败，已恢复当前版本登记和启用状态，文件保留。'
    }
    return $Previous
}
function Uninstall-LingoMateRegistration([string]$Dll, [string]$InstallRoot, [scriptblock]$Register = { param($Dll, $Remove, $Force) Invoke-LingoMateRegistration $Dll $Remove -ForceRegistration:$Force }, [scriptblock]$Profile = { param($Operation, $State) Invoke-LingoMateInputProfile $Operation $State }) {
    Assert-LingoMateOwnedDll $Dll $InstallRoot
    Test-LingoMatePackage (Split-Path $Dll -Parent) -Legacy | Out-Null
    $ProfileState = & $Profile 'Snapshot' $null
    Assert-LingoMateInputProfileState $ProfileState
    try {
        if ($ProfileState.Registered) { & $Profile 'Uninstall' $ProfileState }
        & $Register $Dll $true
    } catch {
        $RecoveryFailed = $false
        if (-not ($_.Exception.Data.Contains('LingoMateRegistrationUntouched') -and $_.Exception.Data['LingoMateRegistrationUntouched'])) { try { & $Register $Dll $false $true } catch { $RecoveryFailed = $true } }
        try { & $Profile 'Restore' $ProfileState } catch { $RecoveryFailed = $true }
        if ($RecoveryFailed) { throw '卸载未完成，原登记或启用状态的恢复也未完成。版本文件已保留。' }
        throw '卸载未完成，已恢复原登记和启用状态，版本文件保留。'
    }
}
Export-ModuleMember -Function *-LingoMate*
