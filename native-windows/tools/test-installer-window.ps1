param([Parameter(Mandatory)][string]$Installer, [Parameter(Mandatory)][string]$Output)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
using System.Collections.Generic;
public static class InstallerPreview {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
    public delegate bool Callback(IntPtr h, IntPtr p);
    [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr h, Callback cb, IntPtr p);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder text, int count);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder text, int count);
    public static Rect[] NextButtons(IntPtr h) {
        var found = new List<Rect>();
        EnumChildWindows(h, (child, state) => {
            var text = new StringBuilder(256); GetWindowText(child, text, text.Capacity);
            var kind = new StringBuilder(256); GetClassName(child, kind, kind.Capacity);
            if (kind.ToString().IndexOf("button", StringComparison.OrdinalIgnoreCase) >= 0 && text.ToString().Contains("下一步")) { Rect r; if (GetWindowRect(child, out r)) found.Add(r); }
            return true;
        }, IntPtr.Zero);
        return found.ToArray();
    }
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
    if (-not $WindowProcess) { throw 'The initial installer window did not appear. No installation was attempted.' }
    $Buttons = [InstallerPreview]::NextButtons($WindowProcess.MainWindowHandle)
    if ($Buttons.Count -ne 1 -or ($Buttons[0].Bottom - $Buttons[0].Top) -lt 44) { throw 'The Chinese Next action must have a >=44px target.' }
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
    Write-Output 'Chinese initial installer window rendered with >=44px action. Install, TSF registration and uninstall were not executed.'
} finally {
    # Only the owned, never-confirmed installer window and its loader are stopped.
    if ($WindowProcess -and -not $WindowProcess.HasExited) { Stop-Process -Id $WindowProcess.Id -ErrorAction SilentlyContinue }
    if (-not $Loader.HasExited) { Stop-Process -Id $Loader.Id -ErrorAction SilentlyContinue }
}
