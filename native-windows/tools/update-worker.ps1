param(
    [Parameter(Mandatory = $true)][ValidateSet('Check', 'Download', 'Install', 'Rollback')][string]$Action,
    [Parameter(Mandatory = $true)][string]$Work
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot 'update-core.psm1') -Force
$Result = Join-Path $Work 'result.json'
try {
    $Directory = Get-LingoMateRegisteredDirectory
    $Config = Assert-LingoMateConfig (Read-LingoMateJson (Join-Path $Directory 'version.json'))
    if ($env:PROCESSOR_ARCHITECTURE -ne 'AMD64' -or -not [Environment]::Is64BitProcess -or [Environment]::OSVersion.Version.Build -lt $Config.minimum_windows_build) { throw '此更新仅支持 Windows 11 Intel / AMD 64 位电脑。' }
    switch ($Action) {
        'Check' {
            $Update = Get-LingoMateUpdate $Config.version $Work
            if ($Update) {
                if ($Update.notes.Length -gt 8000) { $Update.notes = $Update.notes.Substring(0, 8000) }
                Write-LingoMateJson $Update (Join-Path $Work 'update.json')
                Write-LingoMateJson @{ status = 'Available'; version = $Update.metadata.version; current = $Config.version; notes = $Update.notes } $Result
            } else { Write-LingoMateJson @{ status = 'Current'; current = $Config.version } $Result }
        }
        'Download' {
            # Re-fetch the release instead of trusting cached local URLs or metadata.
            $Update = Get-LingoMateUpdate $Config.version $Work
            if (-not $Update) { throw '此更新已撤回或当前版本已经是最新版本，请重新检查。' }
            Receive-LingoMateFile $Update.url (Join-Path $Work 'update.zip') $Update.metadata.size
            Expand-LingoMateUpdate (Join-Path $Work 'update.zip') (Join-Path $Work 'package') $Update.metadata
            Write-LingoMateJson @{ status = 'Ready'; version = $Update.metadata.version; metadata = $Update.metadata } $Result
        }
        'Install' {
            $Prepared = Read-LingoMateJson (Join-Path $Work 'prepared.json')
            if ($Prepared.status -ne 'Ready') { throw '请先下载并校验更新。' }
            $Package = Join-Path $Work 'download/package'
            if ((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $Package 'manifest.json')).Hash -ine $Prepared.metadata.manifest_sha256) { throw '更新文件清单发生变化，请重新下载。' }
            $PackageConfig = Read-LingoMateJson (Join-Path $Package 'version.json')
            if ($PackageConfig.version -cne $Prepared.metadata.version -or (Get-LingoMateVersion $PackageConfig.version) -le (Get-LingoMateVersion $Config.version)) { throw '此更新不是更高版本，请重新检查。' }
            $Destination = Install-LingoMatePackage $Package (Get-LingoMateInstallRoot) (Join-Path $Directory 'lingomate_tsf.dll')
            if ((Get-LingoMateRegisteredDirectory) -ine $Destination) { throw '安装登记未指向新版本。旧版本文件已保留，请检查系统输入设置。' }
            Write-LingoMateJson @{ status = 'Installed'; version = $PackageConfig.version } $Result
        }
        'Rollback' {
            $Previous = Restore-LingoMateVersion $Directory (Get-LingoMateInstallRoot)
            if ((Get-LingoMateRegisteredDirectory) -ine $Previous) { throw '恢复登记未指向上一版本，请检查系统输入设置。' }
            Write-LingoMateJson @{ status = 'Restored' } $Result
        }
    }
} catch {
    # Errors never include input content; callers receive status only, not process output.
    $Message = '更新未完成。请检查网络或公司代理，稍后重试。旧版本文件未删除。'
    if ($_.Exception.Message -match '[\u4e00-\u9fff]') { $Message = $_.Exception.Message }
    Write-LingoMateJson @{ status = 'Error'; message = $Message } $Result
    exit 1
}
