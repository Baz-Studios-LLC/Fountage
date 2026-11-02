#ifndef SourceDir
  #error SourceDir must point to the extracted release folder
#endif
#ifndef ReleaseLabel
  #define ReleaseLabel "v001"
#endif
[Setup]
AppId={{D6F453A0-741C-4FD9-9E21-ADCE80210537}
AppName=Fountage
AppVersion={#ReleaseLabel}
AppVerName=Fountage {#ReleaseLabel} development preview
AppPublisher=Baz Studios LLC
AppPublisherURL=https://github.com/Baz-Studios-LLC/Fountage
AppSupportURL=https://github.com/Baz-Studios-LLC/Fountage/issues
DefaultDirName={localappdata}\Programs\Fountage
PrivilegesRequired=lowest
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\..\dist
OutputBaseFilename=Fountage-{#ReleaseLabel}-windows-x64-setup
SetupIconFile=..\icons\Fountage.ico
UninstallDisplayIcon={app}\Fountage.exe
WizardStyle=modern
Compression=lzma2
SolidCompression=yes
CloseApplications=no
RestartApplications=no
[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
[Icons]
Name: "{autoprograms}\Fountage"; Filename: "{app}\Fountage.exe"
[Run]
Filename: "{app}\Fountage.exe"; Description: "Open Fountage"; Flags: nowait postinstall skipifsilent
