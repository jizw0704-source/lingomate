param([switch]$Background, [switch]$SmokeTest, [string]$PreviewPath)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot 'update-core.psm1') -Force
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
[Windows.Forms.Application]::EnableVisualStyles()
$SettingsDirectory = Join-Path $env:LOCALAPPDATA 'LingoMate'
$SettingsPath = Join-Path $SettingsDirectory 'update-settings.json'
$Settings = @{ auto_check = $false; last_checked = 0L }
if (-not $SmokeTest -and (Test-Path -LiteralPath $SettingsPath)) {
    try {
        $Saved = Read-LingoMateJson $SettingsPath
        if ($Saved.auto_check -is [bool] -and ($Saved.last_checked -is [long] -or $Saved.last_checked -is [int])) {
            $Settings.auto_check = [bool]$Saved.auto_check
            $Settings.last_checked = [long]$Saved.last_checked
        }
    } catch { }
}
if ($Background -and (-not $Settings.auto_check -or ([DateTimeOffset]::UtcNow.ToUnixTimeSeconds() - $Settings.last_checked) -lt 86400)) { return }
$Mutex = $null
$HasMutex = $false
if (-not $SmokeTest) {
    $Sid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    $Mutex = New-Object Threading.Mutex($false, ('Local\LingoMate.Updates.' + $Sid))
    try { $HasMutex = $Mutex.WaitOne(0) } catch [Threading.AbandonedMutexException] { $HasMutex = $true }
    if (-not $HasMutex) { $Mutex.Dispose(); return }
}
$Script:Worker = $null
$Script:Work = $null
$Script:Operation = ''
$Script:State = 'Idle'
$Script:Prepared = $null
$Script:HiddenCheck = [bool]$Background
$Script:Smoke = [bool]$SmokeTest

function Save-Settings {
    if ($Script:Smoke) { return }
    New-Item -ItemType Directory -Path $SettingsDirectory -Force | Out-Null
    Write-LingoMateJson $Settings $SettingsPath
}
function Set-Status([string]$State, [string]$Text, [string]$Button) {
    $Script:State = $State
    $Status.Text = $Text
    $Primary.Text = $Button
    $Primary.Enabled = $State -notin @('Busy', 'Installed', 'Restored')
    $Restore.Enabled = $State -ne 'Busy' -and -not $Script:HiddenCheck
    $Close.Enabled = $Script:Operation -notin @('Install', 'Rollback') -or $State -ne 'Busy'
    $Close.Text = if ($State -eq 'Busy') { '取消' } else { '关闭' }
    $Progress.Visible = $State -eq 'Busy'
}
function Start-Operation([string]$Action) {
    if ($Script:Smoke) { return }
    $Script:Operation = $Action
    if (-not $Script:Work) {
        $Script:Work = Join-Path $env:LOCALAPPDATA ('LingoMate/UpdateCache/' + [Guid]::NewGuid().ToString('N'))
        New-Item -ItemType Directory -Path $Script:Work -Force | Out-Null
    }
    $JobDirectory = switch ($Action) {
        'Check' { Join-Path $Script:Work ('check-' + [Guid]::NewGuid().ToString('N')) }
        'Download' { Join-Path $Script:Work 'download' }
        default { $Script:Work }
    }
    if (Test-Path -LiteralPath (Join-Path $JobDirectory 'result.json')) { Remove-Item -LiteralPath (Join-Path $JobDirectory 'result.json') -Force }
    New-Item -ItemType Directory -Path $JobDirectory -Force | Out-Null
    $Script:JobDirectory = $JobDirectory
    if ($Action -eq 'Download' -and (Test-Path -LiteralPath $JobDirectory)) {
        Remove-Item -LiteralPath $JobDirectory -Recurse -Force
        New-Item -ItemType Directory -Path $JobDirectory | Out-Null
    }
    $Message = switch ($Action) {
        'Check' { '正在检查新版本…' }
        'Download' { '正在下载并校验更新…' }
        'Install' { '正在安装，请保持此窗口打开…' }
        'Rollback' { '正在恢复上一版本，请保持此窗口打开…' }
    }
    Set-Status 'Busy' $Message '请稍候'
    $PowerShell = Join-Path $env:WINDIR 'System32/WindowsPowerShell/v1.0/powershell.exe'
    $Arguments = @('-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', ('"' + (Join-Path $PSScriptRoot 'update-worker.ps1') + '"'), '-Action', $Action, '-Work', ('"' + $JobDirectory + '"'))
    try {
        $Script:Worker = Start-Process -FilePath $PowerShell -ArgumentList $Arguments -WindowStyle Hidden -PassThru
        $Timer.Start()
    } catch { Set-Status 'Error' '无法启动更新程序，请检查公司电脑的运行策略。' '重试' }
}
function Receive-Result {
    if (-not $Script:Worker -or -not $Script:Worker.HasExited) { return }
    $Timer.Stop()
    $Script:Worker.Dispose()
    $Script:Worker = $null
    try { $Result = Read-LingoMateJson (Join-Path $Script:JobDirectory 'result.json') }
    catch { $Result = [pscustomobject]@{ status = 'Error'; message = '更新程序未完成，请检查网络或公司电脑的运行策略。' } }
    if ($Script:HiddenCheck -and $Result.status -ne 'Available') { [Windows.Forms.Application]::ExitThread(); return }
    if ($Script:HiddenCheck) { $Script:HiddenCheck = $false; $Form.Show(); $Form.Activate() }
    switch ($Result.status) {
        'Current' { Set-Status 'Current' ('已安装 ' + $Result.current + '。当前没有更高版本可用。') '重新检查'; $Notes.Text = '更新来源：灵果 GitHub Releases。未发布的版本不会显示为可下载更新。' }
        'Available' { Set-Status 'Available' ('发现新版本 ' + $Result.version + ' · 已安装 ' + $Result.current) '下载更新'; $Notes.Text = $Result.notes }
        'Ready' {
            $Script:Prepared = $Result
            Write-LingoMateJson $Result (Join-Path $Script:Work 'prepared.json')
            Set-Status 'Ready' ('版本 ' + $Result.version + ' 已下载并通过校验。') '安装更新'
        }
        'Installed' { Set-Status 'Installed' ('已安装 ' + $Result.version + '。') '已完成'; $Notes.Text = '请先切换到其他输入法，保存工作后重新打开需要使用灵果的应用，再选择灵果。已经打开的应用可能继续使用旧版本。旧版已保留，可恢复上一版本。' }
        'Restored' { Set-Status 'Restored' '已恢复上一版本。' '已完成'; $Notes.Text = '请保存工作并重新打开需要使用灵果的应用，使其加载恢复后的版本。' }
        default { Set-Status 'Error' ([string]$Result.message) '重新检查' }
    }
    $Script:Operation = ''
}

$Form = New-Object Windows.Forms.Form
$Form.Text = '灵果 · 软件更新'
$Form.ClientSize = New-Object Drawing.Size(528, 424)
$Form.MinimumSize = New-Object Drawing.Size(496, 440)
$Form.StartPosition = 'CenterScreen'
$Form.AutoScaleMode = 'Dpi'
$Form.Font = New-Object Drawing.Font('Microsoft YaHei UI', 10)
$Form.BackColor = [Drawing.SystemColors]::Window
$Form.ForeColor = [Drawing.SystemColors]::WindowText
$Form.MaximizeBox = $false
$Title = New-Object Windows.Forms.Label
$Title.Text = '软件更新'
$Title.Font = New-Object Drawing.Font('Microsoft YaHei UI', 18)
$Title.SetBounds(24, 24, 480, 40)
$Title.Anchor = 'Top, Left, Right'
$Status = New-Object Windows.Forms.Label
$Status.SetBounds(24, 72, 480, 56)
$Status.Anchor = 'Top, Left, Right'
$Notes = New-Object Windows.Forms.TextBox
$Notes.Multiline = $true; $Notes.ReadOnly = $true; $Notes.ScrollBars = 'Vertical'
$Notes.BackColor = [Drawing.SystemColors]::Window
$Notes.ForeColor = [Drawing.SystemColors]::WindowText
$Notes.SetBounds(24, 136, 480, 144)
$Notes.Anchor = 'Top, Bottom, Left, Right'
$Notes.Text = '更新来源：灵果 GitHub Releases。下载完成并通过校验后，由你确认安装。'
$Auto = New-Object Windows.Forms.CheckBox
$Auto.Text = '每天自动检查更新（只提醒，不自动安装）'
$Auto.Checked = [bool]$Settings.auto_check
$Auto.SetBounds(24, 288, 480, 44)
$Auto.Anchor = 'Bottom, Left, Right'
$Auto.Add_CheckedChanged({ $Settings.auto_check = $Auto.Checked; try { Save-Settings } catch { Set-Status 'Error' '更新偏好保存失败，请检查文件权限。' '重新检查' } })
$Progress = New-Object Windows.Forms.ProgressBar
$Progress.Style = 'Marquee'
$Progress.MarqueeAnimationSpeed = 0
$Progress.SetBounds(24, 336, 480, 8)
$Progress.Anchor = 'Bottom, Left, Right'
$Progress.Visible = $false
$Primary = New-Object Windows.Forms.Button
$Primary.SetBounds(24, 356, 144, 44); $Primary.Anchor = 'Bottom, Left'
$Primary.FlatStyle = 'System'; $Primary.TabIndex = 0
$Restore = New-Object Windows.Forms.Button
$Restore.Text = '恢复上一版本'; $Restore.SetBounds(176, 356, 160, 44); $Restore.Anchor = 'Bottom, Left'
$Restore.FlatStyle = 'System'; $Restore.TabIndex = 1
$Close = New-Object Windows.Forms.Button
$Close.Text = '关闭'; $Close.SetBounds(344, 356, 160, 44); $Close.Anchor = 'Bottom, Right'
$Close.FlatStyle = 'System'; $Close.TabIndex = 2
$Form.Controls.AddRange(@($Title, $Status, $Notes, $Auto, $Progress, $Primary, $Restore, $Close))
$Form.AcceptButton = $Primary
$Form.CancelButton = $Close
$Timer = New-Object Windows.Forms.Timer
$Timer.Interval = 250
$Timer.Add_Tick({ Receive-Result })
$Primary.Add_Click({
    switch ($Script:State) {
        'Available' { Start-Operation 'Download' }
        'Ready' {
            $Answer = [Windows.Forms.MessageBox]::Show($Form, '安装后，已打开的应用可能仍使用旧版本。请保存工作，再重新打开应用。现在安装更新吗？', '安装灵果更新', 'YesNo', 'Question', 'Button2')
            if ($Answer -eq 'Yes') { Start-Operation 'Install' }
        }
        default {
            $Settings.last_checked = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
            try { Save-Settings } catch { }
            Start-Operation 'Check'
        }
    }
})
$Restore.Add_Click({
    $Answer = [Windows.Forms.MessageBox]::Show($Form, '恢复上一版本后，需要重新打开使用灵果的应用。现在恢复吗？', '恢复灵果版本', 'YesNo', 'Question', 'Button2')
    if ($Answer -eq 'Yes') { Start-Operation 'Rollback' }
})
$Close.Add_Click({ $Form.Close() })
$Form.Add_FormClosing({
    param($Sender, $Event)
    if ($Script:State -eq 'Busy' -and $Script:Operation -in @('Install', 'Rollback')) { $Event.Cancel = $true; return }
    if ($Script:Worker -and -not $Script:Worker.HasExited) { $Script:Worker.Kill(); $Script:Worker.WaitForExit(); $Script:Worker.Dispose(); $Script:Worker = $null }
})
$Form.Add_FormClosed({ [Windows.Forms.Application]::ExitThread() })
Set-Status 'Idle' '检查灵果的新版本。' '检查更新'
try {
    if ($SmokeTest) {
        if (-not $PreviewPath) { throw 'Specify PreviewPath for the isolated UI render.' }
        $Form.Show(); [Windows.Forms.Application]::DoEvents()
        if ($Primary.Height -lt 44 -or $Close.Height -lt 44 -or $Auto.Height -lt 44) { throw 'Update controls must be at least 44px high.' }
        Set-Status 'Busy' '正在检查新版本…' '请稍候'
        if ($Primary.Enabled -or -not $Close.Enabled) { throw 'Check state must allow cancellation.' }
        $Script:Operation = 'Install'; Set-Status 'Busy' '正在安装，请保持此窗口打开…' '请稍候'
        if ($Close.Enabled) { throw 'Registration must not be cancelled midway.' }
        $Script:Operation = ''; Set-Status 'Error' '网络暂不可用，请稍后重试。' '重新检查'
        if (-not $Primary.Enabled -or -not $Close.Enabled) { throw 'Retry must remain available.' }
        Set-Status 'Idle' '检查灵果的新版本。' '检查更新'
        $Bitmap = New-Object Drawing.Bitmap($Form.Width, $Form.Height)
        try { $Form.DrawToBitmap($Bitmap, (New-Object Drawing.Rectangle(0, 0, $Form.Width, $Form.Height))); $Bitmap.Save($PreviewPath, [Drawing.Imaging.ImageFormat]::Png) } finally { $Bitmap.Dispose() }
        $Form.Close()
    } elseif ($Background) {
        $Settings.last_checked = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds(); Save-Settings
        Start-Operation 'Check'
        [Windows.Forms.Application]::Run()
    } else { [Windows.Forms.Application]::Run($Form) }
} finally {
    $Timer.Stop(); $Timer.Dispose(); $Form.Dispose()
    if ($HasMutex) { $Mutex.ReleaseMutex() }
    if ($Mutex) { $Mutex.Dispose() }
    if ($Script:Work -and (Test-Path -LiteralPath $Script:Work)) { Remove-Item -LiteralPath $Script:Work -Recurse -Force -ErrorAction SilentlyContinue }
}
