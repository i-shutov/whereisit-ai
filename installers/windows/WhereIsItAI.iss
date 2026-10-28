; Inno Setup 6. Copyright 2026 WhereIsIt AI contributors.
; Intended for automated packaging of already-built release binaries.
; The application never bundles or copies the user's private catalog.
#define AppName "WhereIsIt AI"
#ifndef AppVersion
  #define AppVersion "1.0.0-rc.1"
#endif
#ifndef BuildDir
  #define BuildDir "../../target/release"
#endif
#ifndef OutputDir
  #define OutputDir "../../dist/windows"
#endif

[Setup]
AppId={{E89ED28D-1651-4C1E-9358-08C5FCE51722}
AppName={#AppName}
AppVersion={#AppVersion}
VersionInfoVersion=1.0.0.0
AppVerName={#AppName} {#AppVersion}
AppPublisher=WhereIsIt AI contributors
DefaultDirName={localappdata}\Programs\WhereIsIt AI
DefaultGroupName=WhereIsIt AI
OutputDir={#OutputDir}
OutputBaseFilename=WhereIsItAI-{#AppVersion}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\whereisit-ai.exe
AllowNoIcons=yes
ChangesAssociations=no
SetupLogging=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "{#BuildDir}/atlas-windows.exe"; DestDir: "{app}"; DestName: "whereisit-ai.exe"; Flags: ignoreversion
Source: "{#BuildDir}/atlas.exe"; DestDir: "{app}"; DestName: "whereisit-ai-cli.exe"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\WhereIsIt AI"; Filename: "{app}\whereisit-ai.exe"
Name: "{autodesktop}\WhereIsIt AI"; Filename: "{app}\whereisit-ai.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"; Flags: unchecked

[Run]
Filename: "{app}\whereisit-ai.exe"; Description: "Launch WhereIsIt AI"; Flags: nowait postinstall skipifsilent
