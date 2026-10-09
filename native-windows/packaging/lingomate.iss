#ifndef PackageDir
  #error PackageDir is required
#endif
#ifndef ProductVersion
  #error ProductVersion is required
#endif
#ifndef OutputPath
  #error OutputPath is required
#endif

[Setup]
AppId={{BD62BB2D-FC33-4BC9-9188-B69EDB70AA58}
AppName=灵果 LingoMate
AppVersion={#ProductVersion}
AppPublisher=LingoMate
AppPublisherURL=https://github.com/jizw0704-source/lingomate
DefaultDirName={localappdata}\LingoMate\Installer
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
MinVersion=10.0.22000
WizardStyle=modern
WizardSizePercent=120
OutputDir={#OutputPath}
OutputBaseFilename=lingomate-windows-x64-{#ProductVersion}-setup
Compression=lzma2
SolidCompression=yes
InfoBeforeFile=安装与试用说明.txt
InfoAfterFile=安装完成说明.txt
UninstallDisplayName=灵果 LingoMate
CloseApplications=no
RestartApplications=no
AlwaysRestart=no
SetupLogging=yes

[Languages]
Name: "chinesesimplified"; MessagesFile: "compiler:Default.isl,ChineseSimplified.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "{#PackageDir}\*"; DestDir: "{tmp}\LingoMate-package"; Flags: ignoreversion deleteafterinstall
Source: "..\tools\install.ps1"; DestDir: "{tmp}\LingoMate-tools"; Flags: ignoreversion deleteafterinstall
Source: "..\tools\update-core.psm1"; DestDir: "{tmp}\LingoMate-tools"; Flags: ignoreversion deleteafterinstall
Source: "..\tools\uninstall.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "安装与试用说明.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Code]
procedure InitializeWizard();
var ButtonTop, ButtonWidth: Integer;
begin
  ButtonTop := WizardForm.ClientHeight - ScaleY(60);
  ButtonWidth := ScaleX(132);
  WizardForm.CancelButton.SetBounds(WizardForm.ClientWidth - ScaleX(20) - ButtonWidth,
    ButtonTop, ButtonWidth, ScaleY(44));
  WizardForm.NextButton.SetBounds(WizardForm.CancelButton.Left - ScaleX(16) - ButtonWidth,
    ButtonTop, ButtonWidth, ScaleY(44));
  WizardForm.BackButton.SetBounds(WizardForm.NextButton.Left - ScaleX(16) - ButtonWidth,
    ButtonTop, ButtonWidth, ScaleY(44));
  WizardForm.Bevel.Top := ButtonTop - ScaleY(12);
  WizardForm.OuterNotebook.Height := WizardForm.Bevel.Top;
end;

function RunOwnedScript(ScriptPath, Extra: String): Boolean;
var ExitCode: Integer;
begin
  ExitCode := -1;
  Result := Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
    '-NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "' + ScriptPath + '" ' + Extra,
    '', SW_HIDE, ewWaitUntilTerminated, ExitCode);
  Result := Result and (ExitCode = 0);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    if not RunOwnedScript(ExpandConstant('{tmp}\LingoMate-tools\install.ps1'),
      '-Package "' + ExpandConstant('{tmp}\LingoMate-package') + '"') then
      RaiseException('灵果安装或登记未完成。旧版文件及个人数据已保留，请检查安装记录并重试。');
  end;
end;

function InitializeUninstall(): Boolean;
begin
  Result := MsgBox('卸载将移除灵果输入源登记，保留个人数据与历史版本。请先保存工作并切换到其他输入法。继续卸载？',
    mbConfirmation, MB_YESNO) = IDYES;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
  begin
    if not RunOwnedScript(ExpandConstant('{app}\uninstall.ps1'), '') then
      RaiseException('灵果取消登记未完成，安装文件已保留。请重试或联系维护者。');
  end;
end;
