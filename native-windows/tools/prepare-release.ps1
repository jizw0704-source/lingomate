param([string]$Package = (Join-Path (Split-Path $PSScriptRoot -Parent) 'package/windows-x64'), [string]$NotesFile)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot 'update-core.psm1') -Force
$Manifest = Test-LingoMatePackage $Package
$Config = Read-LingoMateJson (Join-Path $Package 'version.json')
$Output = Join-Path (Split-Path $PSScriptRoot -Parent) ('package/release-' + $Config.version)
if (Test-Path -LiteralPath $Output) { throw 'Release output already exists; files preserved.' }
New-Item -ItemType Directory -Path $Output | Out-Null
$Name = 'lingomate-windows-x64-' + $Config.version + '.zip'
$Archive = Join-Path $Output $Name
$Files = @($Manifest.PSObject.Properties.Name) + 'manifest.json' | ForEach-Object { Join-Path $Package $_ }
Compress-Archive -LiteralPath $Files -DestinationPath $Archive -CompressionLevel Optimal
$Metadata = @{
    schema = 1; platform = 'windows'; arch = 'x64'; channel = 'preview'; version = $Config.version; status = 'published'
    minimum_windows_build = 22000; asset = $Name; size = (Get-Item -LiteralPath $Archive).Length
    sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $Archive).Hash.ToLowerInvariant()
    manifest_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $Package 'manifest.json')).Hash.ToLowerInvariant()
    download_url = 'https://github.com/jizw0704-source/lingomate/releases/download/windows-v' + $Config.version + '/' + $Name
    notes = if ($NotesFile) { [IO.File]::ReadAllText((Resolve-Path -LiteralPath $NotesFile).Path, [Text.Encoding]::UTF8) } else { 'Version ' + $Config.version }
}
if ($Metadata.notes.Length -gt 8000) { throw 'Release notes must be <= 8000 characters.' }
Write-LingoMateJson $Metadata (Join-Path $Output 'lingomate-windows-update.json')
Write-Output "Prepared local release files: $Output"
Write-Output 'Nothing was uploaded or published. Verify data distribution rights and Windows installation before publishing.'
Write-Output 'After publishing and verifying both assets, copy the metadata to updates/windows-preview.json and push the feed change.'
