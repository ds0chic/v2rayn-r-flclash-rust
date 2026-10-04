; v2rayN-R Windows x64 installer (Inno Setup 6).
;
; Packages the portable release directory produced by build_windows.ps1
; (dist/v2rayN-R-1.0.0+1-windows-x64) into a per-user installer. The layout is
; preserved bit-for-bit, LICENSE/NOTICE are shown during setup, and Start Menu
; / Desktop shortcuts are optional tasks.
;
; Silent contract used by tools/release/install_test.ps1:
;   installer.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /DIR=<dir>
;                 /MERGETASKS="startmenuicon,desktopicon"
;   unins000.exe  /VERYSILENT /SUPPRESSMSGBOXES /NORESTART
;
; GPL-3.0 derived refactor; see NOTICE.md. No proxy core is bundled.

#define MyAppName "v2rayN-R"
#define MyAppVersion "1.0.0+1"
#define MyAppPublisher "v2rayN-R contributors"
#define MyAppExeName "v2rayn_desktop.exe"
#define MyAppId "{{7A1E4C2D-9B3F-4E6A-8D2C-5F0B7A9E1C34}"
#define MySourceDir "..\..\dist\v2rayN-R-1.0.0+1-windows-x64"

[Setup]
AppId={#MyAppId}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL=https://github.com/2dust/v2rayN
AppSupportURL=https://github.com/anomalyco/opencode/issues
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
DisableReadyPage=no
PrivilegesRequired=lowest
OutputDir=..\..\dist
OutputBaseFilename=v2rayN-R-1.0.0+1-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayName={#MyAppName} {#MyAppVersion}
UninstallDisplayIcon={app}\{#MyAppExeName}
LicenseFile={#MySourceDir}\LICENSE
InfoBeforeFile={#MySourceDir}\NOTICE.md
VersionInfoVersion=1.0.0.1
VersionInfoCompany={#MyAppPublisher}
VersionInfoProductName={#MyAppName}
VersionInfoProductVersion=1.0.0.1
RestartIfNeededByRun=no
SetupLogging=yes
AllowCancelDuringInstall=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "startmenuicon"; Description: "Create a Start Menu shortcut"; GroupDescription: "Additional shortcuts:"; Flags: checkedonce
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "Additional shortcuts:"; Flags: unchecked

[Files]
Source: "{#MySourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"; Tasks: startmenuicon
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Inno removes the files it installed. Only managed upgrade leftovers are
; deleted explicitly, and the install directory is removed only when empty:
; user/foreign files placed under {app} are never recursively deleted.
Type: filesandordirs; Name: "{app}\.staging"
Type: filesandordirs; Name: "{app}\app.previous"
Type: dirifempty; Name: "{app}"
