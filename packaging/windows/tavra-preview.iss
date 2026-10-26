#define AppVersion "0.1.0"
[Setup]
AppId={{8FCF6038-A6E0-46EC-A794-4C06D2A50B0B}
AppName=TAVRA Preview
AppVersion={#AppVersion}
AppPublisher=TAVRA
DefaultDirName={localappdata}\Programs\TAVRA-Preview
DefaultGroupName=TAVRA Preview
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\..\dist
OutputBaseFilename=TAVRA-Preview-Windows-x64-Setup
Compression=lzma2
SolidCompression=yes
UninstallDisplayIcon={app}\tavra.exe
[Files]
Source: "..\..\apps\tavra\build\windows\x64\runner\Release\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
[Icons]
Name: "{group}\TAVRA Preview"; Filename: "{app}\tavra.exe"
[Run]
Filename: "{app}\tavra.exe"; Description: "Launch TAVRA Preview"; Flags: nowait postinstall skipifsilent
