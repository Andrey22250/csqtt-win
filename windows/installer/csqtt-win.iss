; =====================================================================
; csqtt-win Inno Setup 6 Script
; Installer for the csqtt VPN Windows client and background service.
; =====================================================================

#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif

#ifndef SourceDir
  #define SourceDir "dist"
#endif

#if FileExists(SourceDir + "\LICENSE")
  #define LicenseFilePath SourceDir + "\LICENSE"
#elif FileExists("..\..\LICENSE")
  #define LicenseFilePath "..\..\LICENSE"
#else
  #define LicenseFilePath SourceDir + "\LICENSE"
#endif

; --- Setup Configuration ---
[Setup]
AppId={{B52A7D2C-82BE-4C70-9884-26EFE496690B}
AppName=csqtt-win
AppVersion={#AppVersion}
AppPublisher=amurcanov
AppSupportURL=https://github.com/amurcanov/csqtt
DefaultDirName={autopf}\csqtt-win
DefaultGroupName=csqtt-win
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.19041
PrivilegesRequired=admin
LicenseFile={#LicenseFilePath}
Compression=lzma2/ultra64
SolidCompression=yes
OutputDir=.
OutputBaseFilename=csqtt-win-{#AppVersion}-setup

; --- Languages ---
[Languages]
Name: "ru"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

; --- User-facing Messages ---
[CustomMessages]
ru.AutoStartTray=Запускать трей при входе
ru.LaunchTrayNow=Запустить csqtt (трей) сейчас
ru.DeleteUserDataPrompt=Удалить пользовательские данные (настройки, секреты, журналы в %%ProgramData%%\csqtt-win и кэш WebView2 в %%LOCALAPPDATA%%\csqtt-win)?
ru.WebView2MissingWarning=Microsoft Edge WebView2 Runtime не обнаружен.%n%nОкна решения капчи и авторизации VK требуют наличия WebView2 Runtime.%nВы можете скачать и установить его по ссылке:%nhttps://go.microsoft.com/fwlink/p/?LinkId=2124703%n%nУстановка будет продолжена.

en.AutoStartTray=Start tray application on login
en.LaunchTrayNow=Launch csqtt (tray) now
en.DeleteUserDataPrompt=Do you want to delete user data (settings, secrets, logs in %%ProgramData%%\csqtt-win and WebView2 cache in %%LOCALAPPDATA%%\csqtt-win)?
en.WebView2MissingWarning=Microsoft Edge WebView2 Runtime was not detected.%n%nCaptcha and VK authorization windows require Microsoft Edge WebView2 Runtime.%nYou can download and install it from:%nhttps://go.microsoft.com/fwlink/p/?LinkId=2124703%n%nSetup will continue.

; --- Optional Tasks ---
[Tasks]
Name: "autostart"; Description: "{cm:AutoStartTray}"; Flags: unchecked

; --- Registry Entries ---
[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "csqtt-tray"; ValueData: """{app}\csqtt.exe"" tray"; Flags: uninsdeletevalue; Tasks: autostart

; --- Application Files ---
[Files]
Source: "{#SourceDir}\csqtt-svc.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\csqtt.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\client.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\wintun.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\wintun-LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\domains.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\README-windows.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

; --- Run After Installation ---
[Run]
Filename: "{app}\csqtt-svc.exe"; Parameters: "install"; Flags: runhidden waituntilterminated
Filename: "{sys}\sc.exe"; Parameters: "start csqtt-win"; Flags: runhidden
Filename: "{app}\csqtt.exe"; Parameters: "tray"; Description: "{cm:LaunchTrayNow}"; Flags: nowait postinstall runasoriginaluser

; --- Uninstall Process Execution ---
[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/IM csqtt.exe /F"; Flags: runhidden; RunOnceId: "KillCsqttTray"
Filename: "{app}\csqtt-svc.exe"; Parameters: "uninstall"; Flags: runhidden; RunOnceId: "UninstallCsqttSvc"
Filename: "{app}\csqtt-svc.exe"; Parameters: "cleanup"; Flags: runhidden; RunOnceId: "CleanupCsqttSvc"

; --- Cleanup Leftover Application Files ---
[UninstallDelete]
Type: filesandordirs; Name: "{app}"

; --- Pascal Script Code ---
[Code]
var
  UninstallDeleteUserData: Boolean;

// Checks whether Microsoft Edge WebView2 Runtime is installed in HKLM or HKCU.
function IsWebView2Installed: Boolean;
var
  Pv: String;
begin
  Result := False;

  // Check HKLM 32-bit node (WOW64 on 64-bit Windows)
  if RegQueryStringValue(HKLM, 'SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Pv) then
  begin
    if Pv <> '' then
    begin
      Result := True;
      Exit;
    end;
  end;

  // Check HKLM native node
  if RegQueryStringValue(HKLM, 'SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Pv) then
  begin
    if Pv <> '' then
    begin
      Result := True;
      Exit;
    end;
  end;

  // Check HKCU per-user installation
  if RegQueryStringValue(HKCU, 'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Pv) then
  begin
    if Pv <> '' then
    begin
      Result := True;
      Exit;
    end;
  end;

  // Check HKCU WOW64 node
  if RegQueryStringValue(HKCU, 'Software\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Pv) then
  begin
    if Pv <> '' then
    begin
      Result := True;
      Exit;
    end;
  end;
end;

// Initializes setup: checks for WebView2 Runtime and displays a warning if missing.
function InitializeSetup(): Boolean;
var
  WarnMsg: String;
begin
  Result := True;
  if not IsWebView2Installed then
  begin
    // Check OS UI language (0x19 = Russian) so warning is shown correctly prior to language selection
    if (GetUILanguage and $FF) = $19 then
    begin
      WarnMsg := 'Microsoft Edge WebView2 Runtime не обнаружен.' + #13#10 + #13#10 +
        'Окна решения капчи и авторизации VK требуют наличия WebView2 Runtime.' + #13#10 +
        'Вы можете скачать и установить его по ссылке:' + #13#10 +
        'https://go.microsoft.com/fwlink/p/?LinkId=2124703' + #13#10 + #13#10 +
        'Установка будет продолжена.';
    end
    else
    begin
      WarnMsg := 'Microsoft Edge WebView2 Runtime was not detected.' + #13#10 + #13#10 +
        'Captcha and VK authorization windows require Microsoft Edge WebView2 Runtime.' + #13#10 +
        'You can download and install it from:' + #13#10 +
        'https://go.microsoft.com/fwlink/p/?LinkId=2124703' + #13#10 + #13#10 +
        'Setup will continue.';
    end;
    MsgBox(WarnMsg, mbInformation, MB_OK);
  end;
end;

// Executed before installation/upgrade starts.
// Terminates running tray process and stops csqtt-win service to prevent file locks.
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  ResultCode: Integer;
begin
  Result := '';

  // Close running tray process (ignore errors)
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/IM csqtt.exe /F', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);

  // Stop service if it exists (sc.exe stop csqtt-win, wait ~5 s)
  if Exec(ExpandConstant('{sys}\sc.exe'), 'stop csqtt-win', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
  begin
    if ResultCode = 0 then
      Sleep(5000);
  end;
end;

// Handles uninstall steps: asks user whether to delete data directories and executes removal.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  CommonDataDir, LocalDataDir: String;
begin
  if CurUninstallStep = usUninstall then
  begin
    // Prompt user to delete user data; default is No (MB_DEFBUTTON2)
    if MsgBox(CustomMessage('DeleteUserDataPrompt'), mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
      UninstallDeleteUserData := True
    else
      UninstallDeleteUserData := False;
  end
  else if CurUninstallStep = usPostUninstall then
  begin
    if UninstallDeleteUserData then
    begin
      CommonDataDir := ExpandConstant('{commonappdata}\csqtt-win');
      LocalDataDir := ExpandConstant('{localappdata}\csqtt-win');
      if DirExists(CommonDataDir) then
        DelTree(CommonDataDir, True, True, True);
      if DirExists(LocalDataDir) then
        DelTree(LocalDataDir, True, True, True);
    end;
  end;
end;
