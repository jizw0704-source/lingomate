$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:PROCESSOR_ARCHITECTURE -ne 'AMD64' -or $env:PROCESSOR_ARCHITEW6432 -eq 'ARM64') { throw 'The first preview requires Intel/AMD x64 Windows.' }
if (-not [Environment]::Is64BitOperatingSystem -or -not [Environment]::Is64BitProcess) {
    throw 'Use 64-bit PowerShell on an Intel/AMD Windows computer.'
}
$Root = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$Expected = 'c08ae57cb88b6a4a46f4a5e9c1d6d11c5e69222e'
$Checkout = Join-Path $Root 'upstream/qingjian'
foreach ($Tool in @('git', 'cargo', 'rustup')) { Get-Command $Tool -ErrorAction Stop | Out-Null }
& rustup toolchain install 1.96.0 --profile minimal --component rustfmt --component clippy
if ($LASTEXITCODE -ne 0) { throw 'Rust preparation failed.' }
if (Test-Path $Checkout) {
    $Commit = & git -C $Checkout rev-parse HEAD
    if ($LASTEXITCODE -ne 0 -or $Commit -ne $Expected) { throw 'Pinned source differs; existing files preserved.' }
    $Changes = & git -C $Checkout status --porcelain
    if ($LASTEXITCODE -ne 0 -or $Changes) { throw 'Pinned source has local changes; existing files preserved.' }
} else {
    New-Item -ItemType Directory -Path (Split-Path $Checkout -Parent) -Force | Out-Null
    & git clone --filter=blob:none --no-checkout https://github.com/qingjian-team/qingjian.git $Checkout
    if ($LASTEXITCODE -ne 0) { throw 'Source download failed.' }
    & git -C $Checkout fetch --depth=1 origin $Expected
    if ($LASTEXITCODE -ne 0) { throw 'Pinned source fetch failed.' }
    & git -C $Checkout checkout --detach $Expected
    if ($LASTEXITCODE -ne 0) { throw 'Pinned source checkout failed.' }
}
Write-Output 'Windows build inputs ready; nothing has been registered or selected.'
