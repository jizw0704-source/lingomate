param([string]$Package = (Join-Path (Split-Path $PSScriptRoot -Parent) 'package/windows-x64'))
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:PROCESSOR_ARCHITECTURE -ne 'AMD64' -or $env:PROCESSOR_ARCHITEW6432 -eq 'ARM64') { throw 'The first preview requires Intel/AMD x64 Windows.' }
if (-not [Environment]::Is64BitProcess) { throw 'Use 64-bit PowerShell.' }
$Package = (Resolve-Path -LiteralPath $Package).Path
$Manifest = Get-Content -LiteralPath (Join-Path $Package 'manifest.json') -Raw | ConvertFrom-Json
foreach ($Entry in $Manifest.PSObject.Properties) {
    if ($Entry.Name -notmatch '^[A-Za-z0-9_.-]+$') { throw 'Invalid package filename.' }
    $Hash = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $Package $Entry.Name)).Hash
    if ($Hash -ne $Entry.Value) { throw "Package verification failed: $($Entry.Name)" }
}
foreach ($Required in @('lingomate_tsf.dll', 'bilingual-ime-bridge.exe', 'dict.tsv', 'glossary-en.tsv', 'details.json', 'LICENSE', 'GLOSSARY-NOTICE.md', 'LEXICON-NOTICE.md')) {
    if ($Required -notin $Manifest.PSObject.Properties.Name) { throw "Incomplete package: $Required" }
}
$Version = '0.1.0-' + ((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $Package 'manifest.json')).Hash.Substring(0, 12))
$Destination = Join-Path $env:LOCALAPPDATA "LingoMate/Windows/$Version"
$ClassKey = 'HKCU:\Software\Classes\CLSID\{078A5202-3F7A-4DD5-89CC-322A9011DF81}\InprocServer32'
$Previous = if (Test-Path -LiteralPath $ClassKey) { (Get-Item -LiteralPath $ClassKey).GetValue('') } else { $null }
if (Test-Path -LiteralPath $Destination) { throw 'This version is already present; existing files preserved.' }
New-Item -ItemType Directory -Path $Destination -Force | Out-Null
foreach ($Entry in $Manifest.PSObject.Properties) { Copy-Item -LiteralPath (Join-Path $Package $Entry.Name) -Destination (Join-Path $Destination $Entry.Name) }
Copy-Item -LiteralPath (Join-Path $Package 'manifest.json') -Destination $Destination
$Dll = Join-Path $Destination 'lingomate_tsf.dll'
$Regsvr = Join-Path $env:WINDIR 'System32/regsvr32.exe'
$Process = Start-Process -FilePath $Regsvr -ArgumentList @('/s', ('"' + $Dll + '"')) -Wait -PassThru
if ($Process.ExitCode -ne 0) {
    if ($Previous -and (Test-Path -LiteralPath $Previous)) {
        Start-Process -FilePath $Regsvr -ArgumentList @('/s', ('"' + $Previous + '"')) -Wait | Out-Null
    } else {
        Start-Process -FilePath $Regsvr -ArgumentList @('/s', '/u', ('"' + $Dll + '"')) -Wait | Out-Null
    }
    throw 'Registration failed; the previous version was retained. Check Windows permissions before retrying.'
}
@{ version = $Version; destination = $Destination; previous = $Previous } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Destination 'install-receipt.json') -Encoding UTF8
Write-Output 'Installed Lingomate preview. Select it manually using the Windows input switcher.'
Write-Output 'Other input methods, applications and ctfmon were not changed or restarted.'
