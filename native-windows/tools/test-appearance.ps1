param([Parameter(Mandatory)][string]$Dll, [Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class CandidatePreview {
    public delegate bool Callback(IntPtr h,IntPtr p);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
    [DllImport("user32.dll")] public static extern bool EnumWindows(Callback cb,IntPtr p);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint pid);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h,StringBuilder s,int n);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out Rect r);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h,IntPtr dc,uint flags);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
    [DllImport("shell32.dll",CharSet=CharSet.Unicode)] public static extern uint ExtractIconEx(string path,int index,IntPtr[] large,IntPtr[] small,uint count);
    [DllImport("user32.dll")] public static extern bool DestroyIcon(IntPtr icon);
    public static IntPtr Find(uint expected) {
        IntPtr found=IntPtr.Zero;
        EnumWindows((h,p)=> { uint pid; GetWindowThreadProcessId(h,out pid); if(pid==expected) { var s=new StringBuilder(128); GetWindowText(h,s,128); if(s.ToString().Contains("隔离样例")) {found=h;return false;} } return true; },IntPtr.Zero);
        return found;
    }
}
'@
$Dll = (Resolve-Path $Dll).Path
$Large = New-Object IntPtr[] 1
$Small = New-Object IntPtr[] 1
if ([CandidatePreview]::ExtractIconEx($Dll,0,$Large,$Small,1) -ne 1 -or $Large[0] -eq [IntPtr]::Zero -or $Small[0] -eq [IntPtr]::Zero) { throw 'Embedded 果 icon could not be extracted from the installed DLL.' }
try {
    $Icon = [Drawing.Icon]::FromHandle($Large[0])
    $Bitmap = $Icon.ToBitmap()
    try { $Bitmap.Save((Join-Path $OutputDirectory 'input-icon.png'),[Drawing.Imaging.ImageFormat]::Png) } finally { $Bitmap.Dispose(); $Icon.Dispose() }
} finally { [CandidatePreview]::DestroyIcon($Large[0]) | Out-Null; [CandidatePreview]::DestroyIcon($Small[0]) | Out-Null }
foreach ($Kind in @('light','dark','large','narrow')) {
    $Entry = switch ($Kind) { 'light' {'LingoMateCandidatePreviewW'} 'dark' {'LingoMateCandidateDarkPreviewW'} 'large' {'LingoMateCandidateLargePreviewW'} 'narrow' {'LingoMateCandidateNarrowPreviewW'} }
    $Process = Start-Process -FilePath (Join-Path $env:WINDIR 'System32/rundll32.exe') -ArgumentList ('"' + $Dll + '",' + $Entry) -PassThru
    try {
        $Deadline = [DateTime]::UtcNow.AddSeconds(10)
        $Window = [IntPtr]::Zero
        do {
            $Process.Refresh()
            if ($Process.HasExited) { throw 'Candidate preview exited unexpectedly.' }
            $Window = [CandidatePreview]::Find($Process.Id)
            if ($Window -ne [IntPtr]::Zero) { break }
            Start-Sleep -Milliseconds 100
        } while ([DateTime]::UtcNow -lt $Deadline)
        if ($Window -eq [IntPtr]::Zero) { throw 'Candidate preview window not found.' }
        $Rect = New-Object CandidatePreview+Rect
        if (-not [CandidatePreview]::GetWindowRect($Window,[ref]$Rect)) { throw 'Candidate geometry unavailable.' }
        if ($Kind -eq 'narrow' -and ($Rect.Right-$Rect.Left) -gt 324) { throw 'Narrow font22 panel failed to wrap.' }
        $Bitmap = New-Object Drawing.Bitmap(($Rect.Right-$Rect.Left),($Rect.Bottom-$Rect.Top))
        $Graphics = [Drawing.Graphics]::FromImage($Bitmap)
        $Context = $Graphics.GetHdc()
        try { if (-not [CandidatePreview]::PrintWindow($Window,$Context,2)) { throw 'Candidate rendering failed.' } } finally { $Graphics.ReleaseHdc($Context); $Graphics.Dispose() }
        try {
            $Color = $Bitmap.GetPixel(6,6)
            if ($Kind -eq 'light' -and $Color.R -lt 220) { throw 'Light candidate surface was not rendered.' }
            if ($Kind -ne 'light' -and $Color.R -gt 60) { throw 'Dark candidate surface was not rendered.' }
            $Bitmap.Save((Join-Path $OutputDirectory ('candidate-preview-'+$Kind+'.png')),[Drawing.Imaging.ImageFormat]::Png)
        } finally { $Bitmap.Dispose() }
        [CandidatePreview]::PostMessage($Window,0x10,[IntPtr]::Zero,[IntPtr]::Zero) | Out-Null
        if (-not $Process.WaitForExit(5000)) { throw 'Candidate preview did not close cleanly.' }
    } finally { if (-not $Process.HasExited) { Stop-Process -Id $Process.Id -ErrorAction SilentlyContinue } }
}
Write-Output 'Actual DLL icon extracted; isolated light/dark/font22/narrow candidate panels rendered. No TSF registration or personal records.'
