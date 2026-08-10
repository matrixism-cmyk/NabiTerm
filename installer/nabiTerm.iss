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
