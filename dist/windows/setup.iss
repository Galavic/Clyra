#ifndef AppVersion
  #error AppVersion must be supplied by the packaging script
#endif
#ifndef PackageDir
  #error PackageDir must be supplied by the packaging script
#endif
#ifndef OutputDir
  #error OutputDir must be supplied by the packaging script
#endif

[Setup]
AppId={{05C04E94-D480-420E-A5C7-FD0B2DB85E88}
AppName=Clyra
AppVersion={#AppVersion}
AppPublisher=Clyra
DefaultDirName={localappdata}\Programs\Clyra
DefaultGroupName=Clyra
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#OutputDir}
OutputBaseFilename=clyra-{#AppVersion}-windows-x86_64-setup
SetupIconFile=clyra.ico
UninstallDisplayIcon={app}\clyra.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
Uninstallable=not IsExtractOnly
CreateUninstallRegKey=not IsExtractOnly

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked; Check: not IsExtractOnly

[Files]
Source: "{#PackageDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\Clyra"; Filename: "{app}\clyra.exe"; Check: not IsExtractOnly
Name: "{autodesktop}\Clyra"; Filename: "{app}\clyra.exe"; Tasks: desktopicon; Check: not IsExtractOnly

[Run]
Filename: "{app}\clyra.exe"; Description: "Launch Clyra"; Flags: nowait postinstall skipifsilent; Check: not IsExtractOnly

[Code]
function IsExtractOnly: Boolean;
begin
  Result := ExpandConstant('{param:extractonly|0}') = '1';
end;
