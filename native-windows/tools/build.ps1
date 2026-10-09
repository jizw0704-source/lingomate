$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$Root = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$Native = Join-Path $Root 'native-windows'
$Upstream = Join-Path $Root 'upstream/qingjian'
$Commit = & git -C $Upstream rev-parse HEAD
if ($LASTEXITCODE -ne 0 -or $Commit -ne 'c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e') { throw 'Run setup.ps1 with the pinned source first.' }
$Changes = & git -C $Upstream status --porcelain
if ($LASTEXITCODE -ne 0 -or $Changes) { throw 'Upstream changes are preserved; build stopped.' }
$Target = 'x86_64-pc-windows-msvc'
& cargo +1.96.0 build --manifest-path (Join-Path $Root 'prototype/bridge/Cargo.toml') --target-dir (Join-Path $Upstream 'target') --release --locked --target $Target
if ($LASTEXITCODE -ne 0) { throw 'Shared engine build failed.' }
& cargo +1.96.0 build --manifest-path (Join-Path $Native 'Cargo.toml') --release --locked --target $Target
if ($LASTEXITCODE -ne 0) { throw 'Windows text-service build failed.' }
$Package = Join-Path $Native 'package/windows-x64'
if (Test-Path $Package) { Remove-Item -LiteralPath $Package -Recurse -Force }
New-Item -ItemType Directory -Path $Package | Out-Null
$Files = @{
    'lingomate_tsf.dll' = (Join-Path $Native "target/$Target/release/lingomate_tsf.dll")
    'bilingual-ime-bridge.exe' = (Join-Path $Upstream "target/$Target/release/bilingual-ime-bridge.exe")
    'dict.tsv' = (Join-Path $Upstream 'assets/lexicon/dict.tsv')
    'glossary-en.tsv' = (Join-Path $Upstream 'assets/glossary/glossary-en.tsv')
    'details.json' = (Join-Path $Root 'prototype/data/details.json')
    'LICENSE' = (Join-Path $Root 'LICENSE')
    'GLOSSARY-NOTICE.md' = (Join-Path $Upstream 'assets/glossary/README.md')
    'LEXICON-NOTICE.md' = (Join-Path $Upstream 'assets/lexicon/README.md')
}
foreach ($Name in $Files.Keys) { Copy-Item -LiteralPath $Files[$Name] -Destination (Join-Path $Package $Name) }
$Manifest = @{}
foreach ($Name in $Files.Keys) { $Manifest[$Name] = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $Package $Name)).Hash }
$Manifest | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Package 'manifest.json') -Encoding UTF8
Write-Output "Built local Windows preview: $Package"
Write-Output 'Build does not install, register or switch input methods.'
