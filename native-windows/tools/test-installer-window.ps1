param([Parameter(Mandatory)][string]$Installer, [Parameter(Mandatory)][string]$Output)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class InstallerPreview {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
}
'@
$Loader = Start-Process -FilePath $Installer -ArgumentList @('/LANG=chinesesimplified', '/NORESTART') -PassThru
$WindowProcess = $null
try {
    $Deadline = [DateTime]::UtcNow.AddSeconds(20)
    do {
        $Children = @(Get-CimInstance Win32_Process -Filter ('ParentProcessId = ' + $Loader.Id))
        foreach ($Id in @($Loader.Id) + @($Children | ForEach-Object { $_.ProcessId })) {
            $Candidate = Get-Process -Id $Id -ErrorAction SilentlyContinue
            if ($Candidate -and $Candidate.MainWindowHandle -ne [IntPtr]::Zero) { $WindowProcess = $Candidate; break }
        }
        if ($WindowProcess) { break }
        Start-Sleep -Milliseconds 250
    } while ([DateTime]::UtcNow -lt $Deadline)
    if (-not $WindowProcess) { throw 'The installer welcome window did not appear. No installation was attempted.' }
    $Rect = New-Object InstallerPreview+Rect
    if (-not [InstallerPreview]::GetWindowRect($WindowProcess.MainWindowHandle, [ref]$Rect)) { throw 'Installer window geometry unavailable.' }
    $Width = $Rect.Right - $Rect.Left; $Height = $Rect.Bottom - $Rect.Top
    if ($Width -lt 400 -or $Height -lt 300) { throw 'Installer window is unexpectedly small.' }
    $Bitmap = New-Object Drawing.Bitmap($Width, $Height)
    $Graphics = [Drawing.Graphics]::FromImage($Bitmap)
    $Context = $Graphics.GetHdc()
    try {
        if (-not [InstallerPreview]::PrintWindow($WindowProcess.MainWindowHandle, $Context, 2)) { throw 'Installer window rendering failed.' }
    } finally { $Graphics.ReleaseHdc($Context); $Graphics.Dispose() }
    try { $Bitmap.Save($Output, [Drawing.Imaging.ImageFormat]::Png) } finally { $Bitmap.Dispose() }
    Write-Output 'Installer welcome rendered. Install, TSF registration and uninstall were not executed.'
} finally {
    # Only the owned, never-confirmed installer window and its loader are stopped.
    if ($WindowProcess -and -not $WindowProcess.HasExited) { Stop-Process -Id $WindowProcess.Id -ErrorAction SilentlyContinue }
    if (-not $Loader.HasExited) { Stop-Process -Id $Loader.Id -ErrorAction SilentlyContinue }
}
