; nabiTerm Windows 설치 스크립트(Inno Setup 6). `cargo xtask dist`가 컴파일한다.
; 설치본은 nabiTerm.exe로 배포 — 개발본(nabi.exe)과 프로세스명이 달라
; 개발 중 프로세스 정리/재실행이 설치본을 건드리지 않는다.

#ifndef AppVer
  #define AppVer "0.1.0"
#endif

[Setup]
AppId=nabiTerm.aeokorea
AppName=nabiTerm
AppVersion={#AppVer}
AppPublisher=Nabisori
AppPublisherURL=https://nabisori.kr
; 제조사\제품 두 단계로 둔다 — 설치 방식에 따라 아래 둘 중 하나가 된다.
;   전체 설치(관리자):  C:\Program Files\Nabisori\NabiTerm
;   내 계정만(기본):    %LOCALAPPDATA%\Programs\Nabisori\NabiTerm
DefaultDirName={autopf}\Nabisori\NabiTerm
DefaultGroupName=nabiTerm
DisableProgramGroupPage=yes
; **기본은 관리자 권한 없는 per-user 설치다.** Program Files를 기본으로 삼으면 자동
; 업데이트가 매번 UAC를 띄우고 /SILENT로도 그건 막을 수 없다 — 조용한 업데이트가 이
; 프로그램의 약속이라 그쪽을 지킨다. 전체 설치가 필요하면 아래 다이얼로그에서 "모든
; 사용자"를 고르면 되고, 그때 경로가 Program Files가 된다.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=..\dist
OutputBaseFilename=nabiTerm-setup
Compression=lzma2
SolidCompression=yes
CloseApplications=yes
; 실행 중인 nabiTerm이 이 뮤텍스를 잡고 있으면 설치 '시작 전에' 종료를 요청한다 —
; 파일 교체 중 "DeleteFile failed; code 5"(잠금) 오류를 사전에 차단(2026-08-18 사용자 발생).
; 앱(main.rs)이 시작 시 같은 이름의 뮤텍스를 만든다. 여러 인스턴스가 떠 있어도 전부 감지.
AppMutex=nabiTermRunning
UninstallDisplayIcon={app}\nabiTerm.exe
WizardStyle=modern

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "shellmenu"; Description: "탐색기 우클릭 메뉴에 'nabiTerm에서 열기' 추가"; GroupDescription: "추가 기능:"

[Registry]
; 탐색기 우클릭 "nabiTerm에서 열기"(사용자 요청 2026-08-25).
;
; HKA = per-user 설치면 HKCU, 전체 설치면 HKLM으로 알아서 간다 — 권한 상승이 필요 없다.
; %V 는 폴더 위에서 눌렀든 폴더 안 빈 곳에서 눌렀든 그 폴더 경로를 준다.
; 이미 nabiTerm이 떠 있으면 새 pane으로 열리고, 아니면 그 폴더에서 새로 뜬다(openhere.rs).
Root: HKA; Subkey: "Software\Classes\Directory\shell\nabiTerm"; ValueType: string; ValueData: "nabiTerm에서 열기"; Flags: uninsdeletekey; Tasks: shellmenu
Root: HKA; Subkey: "Software\Classes\Directory\shell\nabiTerm"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\nabiTerm.exe"; Tasks: shellmenu
Root: HKA; Subkey: "Software\Classes\Directory\shell\nabiTerm\command"; ValueType: string; ValueData: """{app}\nabiTerm.exe"" --open-here ""%V"""; Tasks: shellmenu
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\nabiTerm"; ValueType: string; ValueData: "nabiTerm에서 열기"; Flags: uninsdeletekey; Tasks: shellmenu
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\nabiTerm"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\nabiTerm.exe"; Tasks: shellmenu
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\nabiTerm\command"; ValueType: string; ValueData: """{app}\nabiTerm.exe"" --open-here ""%V"""; Tasks: shellmenu
Root: HKA; Subkey: "Software\Classes\Drive\shell\nabiTerm"; ValueType: string; ValueData: "nabiTerm에서 열기"; Flags: uninsdeletekey; Tasks: shellmenu
Root: HKA; Subkey: "Software\Classes\Drive\shell\nabiTerm"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\nabiTerm.exe"; Tasks: shellmenu
Root: HKA; Subkey: "Software\Classes\Drive\shell\nabiTerm\command"; ValueType: string; ValueData: """{app}\nabiTerm.exe"" --open-here ""%V"""; Tasks: shellmenu

[Files]
Source: "..\dist\stage\nabiTerm.exe"; DestDir: "{app}"; Flags: ignoreversion
; 소프트웨어 OpenGL 폴백(Mesa llvmpipe)은 메인 설치본에 넣지 않는다. GPU 없는 VM 사용자는
; mesa-runtime 고정 릴리스의 별도 자산을 받아 이 폴더(nabiTerm.exe 옆)에 푼다.

[Icons]
Name: "{group}\nabiTerm"; Filename: "{app}\nabiTerm.exe"
Name: "{autodesktop}\nabiTerm"; Filename: "{app}\nabiTerm.exe"; Tasks: desktopicon

[Run]
; 설치가 끝나면 nabiTerm을 다시 켠다. `skipifsilent`를 **일부러 뺐다** — 앱에서 시작한
; 업데이트는 조용한 설치(/SILENT)로 돌기 때문에, 그 플래그가 있으면 재실행이 통째로
; 건너뛰어진다("업데이트했는데 다시 안 켜진다", 사용자 보고 2026-08-22).
; runasoriginaluser: 설치가 관리자로 올라갔더라도 앱은 원래 사용자 권한으로 켠다.
Filename: "{app}\nabiTerm.exe"; Description: "{cm:LaunchProgram,nabiTerm}"; Flags: nowait postinstall runasoriginaluser
