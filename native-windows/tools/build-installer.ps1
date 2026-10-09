param([string]$Package = (Join-Path (Split-Path $PSScriptRoot -Parent) 'package/windows-x64'), [string]$Compiler)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot 'update-core.psm1') -Force
$Package = (Resolve-Path -LiteralPath $Package).Path
Test-LingoMatePackage $Package | Out-Null
$Config = Read-LingoMateJson (Join-Path $Package 'version.json')
if (-not $Compiler) {
    $Candidates = @((Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6/ISCC.exe'), (Join-Path $env:ProgramFiles 'Inno Setup 6/ISCC.exe'))
    $Compiler = $Candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
}
if (-not $Compiler -or -not (Test-Path -LiteralPath $Compiler)) { throw 'Install Inno Setup 6 from jrsoftware.org, or pass its ISCC.exe path using -Compiler. No installer was downloaded automatically.' }
$Native = Split-Path $PSScriptRoot -Parent
$Output = Join-Path $Native ('package/installer-' + $Config.version)
if (Test-Path -LiteralPath $Output) { throw 'Installer output already exists; files preserved.' }
New-Item -ItemType Directory -Path $Output | Out-Null
& $Compiler ('/DPackageDir=' + $Package) ('/DProductVersion=' + $Config.version) ('/DOutputPath=' + $Output) (Join-Path $Native 'packaging/lingomate.iss')
if ($LASTEXITCODE -ne 0) { throw 'Inno Setup compilation failed.' }
$Installer = Join-Path $Output ('lingomate-windows-x64-' + $Config.version + '-setup.exe')
if (-not (Test-Path -LiteralPath $Installer)) { throw 'Installer executable missing.' }
$Hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $Installer).Hash.ToLowerInvariant()
($Hash + '  ' + [IO.Path]::GetFileName($Installer)) | Set-Content -LiteralPath ($Installer + '.sha256') -Encoding ASCII
Write-Output "Local installer: $Installer"
Write-Output 'No installation, registration or publication was performed. Distribution rights, signing and Windows 11 acceptance remain pending.'
