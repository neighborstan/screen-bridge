#define AppName "ScreenBridge"

#ifndef ProjectRoot
  #error ProjectRoot must be provided by scripts\build-installer.ps1
#endif

#ifndef GStreamerRoot
  #error GStreamerRoot must be provided by scripts\build-installer.ps1
#endif

#ifndef AppVersion
  #error AppVersion must be provided by scripts\build-installer.ps1
#endif

[Setup]
AppId={{8570E7BB-879A-4D88-8E63-D91D09B37F38}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=ScreenBridge contributors
DefaultDirName={localappdata}\Programs\ScreenBridge
DefaultGroupName=ScreenBridge
OutputDir={#ProjectRoot}\dist
OutputBaseFilename=ScreenBridge-{#AppVersion}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
WizardStyle=modern
LicenseFile={#ProjectRoot}\LICENSE
SetupIconFile={#ProjectRoot}\installer\assets\screenbridge.ico
UninstallDisplayIcon={app}\assets\screenbridge.ico
CloseApplications=yes

[Dirs]
Name: "{app}\empty-gio-modules"
Name: "{userappdata}\ScreenBridge"; Permissions: users-modify

[InstallDelete]
Type: filesandordirs; Name: "{app}\bin"
Type: filesandordirs; Name: "{app}\etc"
Type: filesandordirs; Name: "{app}\lib"
Type: filesandordirs; Name: "{app}\libexec"
Type: filesandordirs; Name: "{app}\share"
Type: filesandordirs; Name: "{app}\config"
Type: filesandordirs; Name: "{app}\assets"
Type: filesandordirs; Name: "{app}\empty-gio-modules"

[Files]
Source: "{#ProjectRoot}\target\release\screen-bridge-host.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "{#ProjectRoot}\target\release\screen-bridge-viewer.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "{#ProjectRoot}\installer\launch-host.cmd"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "{#ProjectRoot}\installer\launch-viewer.cmd"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "{#ProjectRoot}\installer\assets\screenbridge.ico"; DestDir: "{app}\assets"; Flags: ignoreversion
Source: "{#ProjectRoot}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#ProjectRoot}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#ProjectRoot}\config\host.example.toml"; DestDir: "{app}\config"; Flags: ignoreversion
Source: "{#ProjectRoot}\config\viewer.example.toml"; DestDir: "{app}\config"; Flags: ignoreversion
Source: "{#ProjectRoot}\config\host.example.toml"; DestDir: "{userappdata}\ScreenBridge"; DestName: "host.toml"; Flags: ignoreversion onlyifdoesntexist uninsneveruninstall
Source: "{#ProjectRoot}\config\viewer.example.toml"; DestDir: "{userappdata}\ScreenBridge"; DestName: "viewer.toml"; Flags: ignoreversion onlyifdoesntexist uninsneveruninstall
Source: "{#GStreamerRoot}\bin\*.dll"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "{#GStreamerRoot}\bin\gst-inspect-1.0.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "{#GStreamerRoot}\bin\gst-launch-1.0.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "{#GStreamerRoot}\bin\gspawn-win64-helper*.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "{#GStreamerRoot}\bin\pkg-config.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "{#GStreamerRoot}\etc\*"; DestDir: "{app}\etc"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#GStreamerRoot}\lib\gstreamer-1.0\*.dll"; DestDir: "{app}\lib\gstreamer-1.0"; Flags: ignoreversion
Source: "{#GStreamerRoot}\lib\pkgconfig\*"; DestDir: "{app}\lib\pkgconfig"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#GStreamerRoot}\libexec\gstreamer-1.0\*"; DestDir: "{app}\libexec\gstreamer-1.0"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#GStreamerRoot}\share\glib-2.0\*"; DestDir: "{app}\share\glib-2.0"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#GStreamerRoot}\share\gstreamer\*"; DestDir: "{app}\share\gstreamer"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#GStreamerRoot}\share\gstreamer-1.0\*"; DestDir: "{app}\share\gstreamer-1.0"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#GStreamerRoot}\share\licenses\*"; DestDir: "{app}\share\licenses"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\ScreenBridge Host"; Filename: "{app}\bin\launch-host.cmd"; WorkingDir: "{userappdata}\ScreenBridge"; IconFilename: "{app}\assets\screenbridge.ico"
Name: "{group}\ScreenBridge Viewer"; Filename: "{app}\bin\launch-viewer.cmd"; WorkingDir: "{userappdata}\ScreenBridge"; IconFilename: "{app}\assets\screenbridge.ico"
Name: "{group}\ScreenBridge Config"; Filename: "{userappdata}\ScreenBridge"; IconFilename: "{app}\assets\screenbridge.ico"
Name: "{group}\Uninstall ScreenBridge"; Filename: "{uninstallexe}"; IconFilename: "{app}\assets\screenbridge.ico"
