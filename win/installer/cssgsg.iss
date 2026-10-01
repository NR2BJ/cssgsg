; cssgsg 윈도우 설치기(Inno Setup 6). tools/win/build-installer.ps1이 빌드한 파일로 만든다:
;   ISCC /DVersion=0.1.0 /DNumericVersion=0.1.0 /DRepo=<저장소> win\installer\cssgsg.iss → build\installer\cssgsg-setup.exe
;
; - 관리자 쪽(Program Files\cssgsg에 복사, 입력기 DLL 등록)은 설치기가 하고, 사용자마다 하는 일(내 입력 목록, 엔진 호스트를
;   시작 프로그램에 넣고 띄우기)은 설치한 사용자로 cssgsg-host.exe --install-user가 한다(지울 때는 --uninstall-user).
; - 입력기 DLL은 모든 앱이 쥐고 있어서 덮어쓸 수 없다: 이름을 바꿔 두고 새 파일을 넣는다(이미 뜬 앱은 옛 DLL을 계속 쓴다).
;   옛 파일은 지울 수 있으면 바로, 아니면 다시 시작할 때 지운다. 앱을 닫으라고 하지 않는다(CloseApplications=no).
; - 엔진 호스트는 파일을 바꾸기 전에 끈다(--quit: 학습을 마무리하고 끝낸다, 안 되면 taskkill). 설정 앱도 끈다.
; - 설정 앱(build\settings\publish, tools/win/build-settings.ps1)은 {app}\settings에 통째로 넣는다(옛 판의 파일이 남지 않게
;   폴더를 지우고 넣는다). 시작 메뉴에 "cssgsg 설정"을 둔다.
; - 설정 앱이 업데이트하려고 이 설치기를 띄우면(/SILENT /RELAUNCH=1) 끝난 뒤 설정 앱을 정보 탭으로 다시 띄운다.
; - 학습·사용자 사전(%LOCALAPPDATA%\cssgsg)과 설정(%APPDATA%\cssgsg)은 지울 때도 남긴다.
; - 이 파일은 UTF-8(BOM)이다(Inno Setup 6은 BOM이 있으면 UTF-8로 읽는다).

#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef NumericVersion
  #define NumericVersion "0.0.0"
#endif
#ifndef Repo
  #define Repo "..\.."
#endif

[Setup]
AppId={{2F0D1872-EBB9-4C74-A221-F36D4905540A}
AppName=cssgsg
AppVersion={#Version}
AppVerName=cssgsg {#Version}
AppPublisher=NR2BJ
AppPublisherURL=https://github.com/NR2BJ/cssgsg
AppSupportURL=https://github.com/NR2BJ/cssgsg/issues
AppUpdatesURL=https://github.com/NR2BJ/cssgsg/releases
VersionInfoVersion={#NumericVersion}
DefaultDirName={autopf}\cssgsg
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
OutputDir={#Repo}\build\installer
OutputBaseFilename=cssgsg-setup
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ShowLanguageDialog=no
CloseApplications=no
RestartApplications=no
UninstallDisplayName=cssgsg
UninstallDisplayIcon={app}\settings\cssgsg-settings.exe
SetupIconFile={#Repo}\win\settings\Assets\cssgsg.ico

[Languages]
Name: "ko"; MessagesFile: "compiler:Languages\Korean.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Messages]
ko.FinishedLabel=설치를 마쳤습니다.%n%nWin+Space를 눌러 입력기 목록에서 cssgsg(ENG)를 고르세요. 이미 열려 있던 앱은 다시 열면 새 입력기를 씁니다.
en.FinishedLabel=Setup has finished.%n%nPress Win+Space and choose cssgsg (ENG) in the input list. Apps that were already open use the new version after you reopen them.

[CustomMessages]
ko.AddingInput=입력 목록에 cssgsg를 넣는 중…
en.AddingInput=Adding cssgsg to your input methods…
ko.SettingsName=cssgsg 설정
en.SettingsName=cssgsg Settings
ko.OpenSettings=cssgsg 설정 열기
en.OpenSettings=Open cssgsg Settings

[Files]
Source: "{#Repo}\build\cargo\release\cssgsg_tip.dll"; DestDir: "{app}"; Flags: ignoreversion regserver uninsrestartdelete; BeforeInstall: MoveAside('{app}\cssgsg_tip.dll')
Source: "{#Repo}\build\cargo\release\cssgsg-host.exe"; DestDir: "{app}"; Flags: ignoreversion uninsrestartdelete; BeforeInstall: MoveAside('{app}\cssgsg-host.exe')
Source: "{#Repo}\build\mozc-out\lib\cssgsg_mozc.dll"; DestDir: "{app}\mozc"; Flags: ignoreversion uninsrestartdelete; BeforeInstall: MoveAside('{app}\mozc\cssgsg_mozc.dll')
Source: "{#Repo}\build\mozc-out\data\mozc.data"; DestDir: "{app}\mozc"; Flags: ignoreversion uninsrestartdelete; BeforeInstall: MoveAside('{app}\mozc\mozc.data')
Source: "{#Repo}\build\mozc-out\MOZC_VERSION"; DestDir: "{app}\mozc"; Flags: ignoreversion
Source: "{#Repo}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Repo}\THIRD_PARTY_NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Repo}\build\settings\publish\*"; DestDir: "{app}\settings"; Flags: ignoreversion recursesubdirs createallsubdirs

; 설정 앱은 판마다 파일 구성이 바뀔 수 있어(Windows App SDK) 넣기 전에 폴더째 지운다. 설정 앱은 PrepareToInstall에서 껐다.
[InstallDelete]
Type: filesandordirs; Name: "{app}\settings"

[Icons]
Name: "{autoprograms}\{cm:SettingsName}"; Filename: "{app}\settings\cssgsg-settings.exe"; WorkingDir: "{app}\settings"

; 옆으로 치운 파일 중 남은 것. 설치 때는 지우지 않는다: [InstallDelete]에 넣으면 Inno가 그 파일들을 자기 파일로 보고, 다시
; 시작 때 지우기로 예약된 것이 있으면 "다시 시작해야 설치를 마칠 수 있다"며 설치를 멈춘다(업데이트를 연달아 못 한다).
[UninstallDelete]
Type: files; Name: "{app}\*.old-*"
Type: files; Name: "{app}\mozc\*.old-*"

[Run]
Filename: "{app}\cssgsg-host.exe"; Parameters: "--install-user"; Flags: runasoriginaluser waituntilterminated; StatusMsg: "{cm:AddingInput}"
; 설정 앱의 업데이트(/RELAUNCH=1): 끝나면 정보 탭으로 다시 띄운다. 손으로 설치했으면 마지막 화면에서 열지 고른다.
Filename: "{app}\settings\cssgsg-settings.exe"; Parameters: "--tab about"; Flags: nowait runasoriginaluser; Check: RelaunchRequested
Filename: "{app}\settings\cssgsg-settings.exe"; Description: "{cm:OpenSettings}"; Flags: postinstall nowait skipifsilent runasoriginaluser

[UninstallRun]
Filename: "{app}\cssgsg-host.exe"; Parameters: "--uninstall-user"; Flags: runhidden waituntilterminated; RunOnceId: "UninstallUser"
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM cssgsg-host.exe"; Flags: runhidden waituntilterminated; RunOnceId: "KillHost"
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM cssgsg-settings.exe"; Flags: runhidden waituntilterminated; RunOnceId: "KillSettings"

[Code]
{ 쓰는 중일 수 있는 파일을 옆으로 치운다: 이름을 바꾸고, 지울 수 있으면 지우고, 아니면 다시 시작할 때 지운다. }
procedure MoveAside(const Target: String);
var
  Path, Old: String;
begin
  Path := ExpandConstant(Target);
  if not FileExists(Path) then
    exit;
  Old := Path + '.old-' + GetDateTimeString('yyyymmddhhnnsszzz', #0, #0);
  if RenameFile(Path, Old) then
  begin
    if not DeleteFile(Old) then
      RestartReplace(Old, '');
  end
  else if not DeleteFile(Path) then
    Log('Could not move aside ' + Path);
end;

{ 엔진 호스트를 끈다: 학습을 마무리하고 끝내게 하고(--quit), 그래도 남았으면 강제로. 설정 앱도 끈다(파일을 바꾼다). }
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Code: Integer;
  HostExe: String;
begin
  HostExe := ExpandConstant('{app}\cssgsg-host.exe');
  if FileExists(HostExe) then
    Exec(HostExe, '--quit', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM cssgsg-host.exe', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM cssgsg-settings.exe', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Result := '';
end;

{ 설정 앱이 업데이트하려고 띄웠다(/RELAUNCH=1): 끝나면 설정 앱을 다시 띄운다. }
function RelaunchRequested: Boolean;
begin
  Result := ExpandConstant('{param:RELAUNCH|0}') = '1';
end;
