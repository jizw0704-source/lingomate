Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$script:Repository = 'jizw0704-source/lingomate'
$script:PackageFiles = @('lingomate_tsf.dll', 'bilingual-ime-bridge.exe', 'dict.tsv', 'glossary-en.tsv', 'details.json', 'LICENSE', 'GLOSSARY-NOTICE.md', 'LEXICON-NOTICE.md', 'version.json', 'update.ps1', 'update-worker.ps1', 'update-core.psm1')

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
    $Allowed = ($Uri.Host -ceq 'api.github.com' -and $Uri.AbsolutePath -eq "/repos/$script:Repository/releases") -or ($Uri.Host -ceq 'github.com' -and $Uri.AbsolutePath.StartsWith("/$script:Repository/releases/download/", [StringComparison]::Ordinal))
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
function Select-LingoMateRelease($Releases, [string]$Current) {
    $CurrentVersion = Get-LingoMateVersion $Current
    $Matches = @($Releases | Where-Object {
        -not $_.draft -and $_.tag_name -cmatch '^windows-v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$' -and @($_.assets | Where-Object { $_.name -ceq 'lingomate-windows-update.json' }).Count -eq 1
    } | Sort-Object { Get-LingoMateVersion $_.tag_name.Substring(9) } -Descending)
    if ($Matches.Count -eq 0) { return $null }
    $Release = $Matches[0]
    $Version = $Release.tag_name.Substring(9)
    if ((Get-LingoMateVersion $Version) -le $CurrentVersion) { return $null }
    $Asset = @($Release.assets | Where-Object { $_.name -ceq 'lingomate-windows-update.json' })[0]
    if ($Asset.size -le 0 -or $Asset.size -gt 65536) { throw '发布元数据大小无效。' }
    Assert-LingoMateUrl $Asset.browser_download_url | Out-Null
    return [pscustomobject]@{ version = $Version; release = $Release; metadata_url = $Asset.browser_download_url }
}
function Assert-LingoMateUpdate($Metadata, $Selected) {
    if ($Metadata.schema -ne 1 -or $Metadata.platform -ne 'windows' -or $Metadata.arch -ne 'x64' -or $Metadata.channel -ne 'preview' -or $Metadata.minimum_windows_build -ne 22000 -or $Metadata.version -cne $Selected.version) { throw '更新包与当前平台或发布版本不一致。' }
    Get-LingoMateVersion $Metadata.version | Out-Null
    if ($Metadata.sha256 -cnotmatch '^[a-f0-9]{64}$' -or $Metadata.manifest_sha256 -cnotmatch '^[a-f0-9]{64}$' -or $Metadata.size -le 0 -or $Metadata.size -gt 134217728) { throw '更新校验信息无效。' }
    $Name = 'lingomate-windows-x64-' + $Metadata.version + '.zip'
    if ($Metadata.asset -cne $Name) { throw '更新包名称无效。' }
    $Assets = @($Selected.release.assets | Where-Object { $_.name -ceq $Name })
    if ($Assets.Count -ne 1 -or $Assets[0].size -ne $Metadata.size) { throw '更新包缺失或大小不一致。' }
    $Url = $Assets[0].browser_download_url
    Assert-LingoMateUrl $Url | Out-Null
    $Expected = "https://github.com/$script:Repository/releases/download/windows-v$($Metadata.version)/$Name"
    if ($Url -cne $Expected -or $Selected.metadata_url -cne "https://github.com/$script:Repository/releases/download/windows-v$($Metadata.version)/lingomate-windows-update.json") { throw '更新文件不属于当前发布版本。' }
    return [pscustomobject]@{ metadata = $Metadata; url = $Url; notes = [string]$Selected.release.body }
}
function Get-LingoMateUpdate([string]$Current, [string]$Work) {
    $List = Join-Path $Work 'releases.json'
    Receive-LingoMateFile "https://api.github.com/repos/$script:Repository/releases?per_page=30" $List 1048576
    $Selected = Select-LingoMateRelease (Read-LingoMateJson $List 1048576) $Current
    if (-not $Selected) { return $null }
    $Info = Join-Path $Work 'metadata.json'
    Receive-LingoMateFile $Selected.metadata_url $Info 65536
    return (Assert-LingoMateUpdate (Read-LingoMateJson $Info) $Selected)
}
function Test-LingoMatePackage([string]$Package, [switch]$Legacy) {
    $Manifest = Read-LingoMateJson (Join-Path $Package 'manifest.json')
    $Names = @($Manifest.PSObject.Properties.Name)
    $Required = if ($Legacy -and $Names -notcontains 'version.json') { $script:PackageFiles[0..7] } else { $script:PackageFiles }
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
function Invoke-LingoMateRegistration([string]$Dll, [bool]$Remove = $false) {
    $Args = @('/s')
    if ($Remove) { $Args += '/u' }
    $Args += ('"' + $Dll + '"')
    $Process = Start-Process -FilePath (Join-Path $env:WINDIR 'System32/regsvr32.exe') -ArgumentList $Args -Wait -PassThru
    if ($Process.ExitCode -ne 0) { throw '输入法登记失败。' }
}
function Install-LingoMatePackage([string]$Package, [string]$InstallRoot, [string]$PreviousDll, [scriptblock]$Register = { param($Dll, $Remove) Invoke-LingoMateRegistration $Dll $Remove }) {
    $Manifest = Test-LingoMatePackage $Package
    $Config = Read-LingoMateJson (Join-Path $Package 'version.json')
    if ($PreviousDll) { Assert-LingoMateOwnedDll $PreviousDll $InstallRoot }
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
    # A failed first installation can be retried without losing its inactive files.
    if ($PreviousDll -and [IO.Path]::GetFullPath($PreviousDll) -ieq [IO.Path]::GetFullPath($Dll)) { return $Destination }
    Write-LingoMateJson @{ version = $Config.version; destination = $Destination; previous = $PreviousDll } (Join-Path $Destination 'install-receipt.json')
    try { & $Register $Dll $false }
    catch {
        try {
            if ($PreviousDll) { & $Register $PreviousDll $false } else { & $Register $Dll $true }
        } catch { throw '新版本登记失败，回退登记也未完成。旧版本文件已保留，请检查系统输入设置。' }
        throw '新版本登记失败，已恢复原登记，旧版本文件保留。'
    }
    return $Destination
}
function Restore-LingoMateVersion([string]$Current, [string]$InstallRoot, [scriptblock]$Register = { param($Dll, $Remove) Invoke-LingoMateRegistration $Dll $Remove }) {
    Assert-LingoMateOwnedDll (Join-Path $Current 'lingomate_tsf.dll') $InstallRoot
    $Receipt = Read-LingoMateJson (Join-Path $Current 'install-receipt.json')
    if (-not $Receipt.previous) { throw '没有可恢复的上一版本。' }
    Assert-LingoMateOwnedDll $Receipt.previous $InstallRoot
    $Previous = Split-Path $Receipt.previous -Parent
    Test-LingoMatePackage $Previous -Legacy | Out-Null
    & $Register $Receipt.previous $false
    return $Previous
}
Export-ModuleMember -Function *-LingoMate*
