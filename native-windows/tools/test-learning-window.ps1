param([Parameter(Mandatory)][string]$Dll, [Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class LearningPreview {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder text, int count);
}
'@
foreach ($Kind in @('wide', 'narrow')) {
    $Entry = if ($Kind -eq 'wide') { 'LingoMateLearningPreviewW' } else { 'LingoMateLearningNarrowPreviewW' }
    $Process = Start-Process -FilePath (Join-Path $env:WINDIR 'System32/rundll32.exe') -ArgumentList ('"' + (Resolve-Path $Dll).Path + '",' + $Entry) -PassThru
    try {
        $Deadline = [DateTime]::UtcNow.AddSeconds(15)
        do {
            $Process.Refresh()
            if ($Process.HasExited) { throw 'The isolated learning helper exited before showing its window.' }
            if ($Process.MainWindowHandle -ne [IntPtr]::Zero) { break }
            Start-Sleep -Milliseconds 200
        } while ([DateTime]::UtcNow -lt $Deadline)
        if ($Process.MainWindowHandle -eq [IntPtr]::Zero) { throw 'The isolated learning window did not appear.' }
        $Window = $Process.MainWindowHandle
        foreach ($Id in @(1,5,6,7,8)) {
            $Rect = New-Object LearningPreview+Rect
            if (-not [LearningPreview]::GetWindowRect([LearningPreview]::GetDlgItem($Window, $Id), [ref]$Rect) -or ($Rect.Bottom - $Rect.Top) -lt 44) { throw 'Learning actions must provide >=44px targets.' }
        }
        $Text = New-Object Text.StringBuilder(256)
        [LearningPreview]::GetWindowText([LearningPreview]::GetDlgItem($Window,4), $Text, $Text.Capacity) | Out-Null
        if (-not $Text.ToString().Contains('共 2 个词条')) { throw 'Synthetic preview words were not loaded.' }
        $Rect = New-Object LearningPreview+Rect
        if (-not [LearningPreview]::GetWindowRect($Window,[ref]$Rect)) { throw 'Window geometry unavailable.' }
        $Bitmap = New-Object Drawing.Bitmap(($Rect.Right - $Rect.Left), ($Rect.Bottom - $Rect.Top))
        $Graphics = [Drawing.Graphics]::FromImage($Bitmap)
        $Context = $Graphics.GetHdc()
        try {
            if (-not [LearningPreview]::PrintWindow($Window,$Context,2)) { throw 'Learning preview rendering failed.' }
        } finally { $Graphics.ReleaseHdc($Context); $Graphics.Dispose() }
        try { $Bitmap.Save((Join-Path $OutputDirectory ('learning-preview-' + $Kind + '.png')), [Drawing.Imaging.ImageFormat]::Png) } finally { $Bitmap.Dispose() }
        $Process.CloseMainWindow() | Out-Null
        if (-not $Process.WaitForExit(5000)) { throw 'Learning helper did not close cleanly.' }
    } finally {
        # Stop only the helper launched above; it uses disposable synthetic records.
        if (-not $Process.HasExited) { Stop-Process -Id $Process.Id -ErrorAction SilentlyContinue }
    }
}
Write-Output 'Offline learning helper: wide/narrow windows rendered, >=44px actions checked; no TSF registration or personal records accessed.'
