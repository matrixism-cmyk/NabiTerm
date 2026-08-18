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
AppPublisher=aeo
DefaultDirName={autopf}\nabiTerm
DefaultGroupName=nabiTerm
DisableProgramGroupPage=yes
; 관리자 권한 불필요(per-user 설치 기본, 다이얼로그로 전체 설치 선택 가능).
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

[Files]
Source: "..\dist\stage\nabiTerm.exe"; DestDir: "{app}"; Flags: ignoreversion
; 소프트웨어 OpenGL 폴백(Mesa llvmpipe)은 메인 설치본에 넣지 않는다. GPU 없는 VM 사용자는
; mesa-runtime 고정 릴리스의 별도 자산을 받아 이 폴더(nabiTerm.exe 옆)에 푼다.

[Icons]
Name: "{group}\nabiTerm"; Filename: "{app}\nabiTerm.exe"
Name: "{autodesktop}\nabiTerm"; Filename: "{app}\nabiTerm.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\nabiTerm.exe"; Description: "{cm:LaunchProgram,nabiTerm}"; Flags: nowait postinstall skipifsilent
