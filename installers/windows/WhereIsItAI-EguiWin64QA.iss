; Native Rust/egui Win64 QA — separate unsigned installer, for prerelease testing.
#ifndef AppVersion
  #define AppVersion "1.1.1"
#endif
#ifndef BuildDir
  #define BuildDir "../../target/release"
#endif
#ifndef OutputDir
  #define OutputDir "../../dist/windows-egui-win64-qa"
#endif
[Setup]
AppId={{C147B6E6-D2DA-4F11-8FC4-87BAE1F07967}
AppName=WhereIsIt AI egui Win64 QA
AppVersion={#AppVersion}
VersionInfoVersion=1.1.1.0
AppVerName=WhereIsIt AI egui Win64 QA {#AppVersion} (unsigned test build)
AppPublisher=WhereIsIt AI contributors
DefaultDirName={localappdata}\Programs\WhereIsIt AI egui Win64 QA
DefaultGroupName=WhereIsIt AI egui Win64 QA
OutputDir={#OutputDir}
OutputBaseFilename=WhereIsItAI-egui-win64-QA-{#AppVersion}-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\whereisit-egui-win64.exe
AllowNoIcons=yes
ChangesAssociations=no
SetupLogging=yes
[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
[Files]
Source: "{#BuildDir}/atlas-desktop-egui.exe"; DestDir: "{app}"; DestName: "whereisit-egui-win64.exe"; Flags: ignoreversion
Source: "{#BuildDir}/atlas.exe"; DestDir: "{app}"; DestName: "whereisit-ai-cli.exe"; Flags: ignoreversion
Source: "../../LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "../../THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "../../licenses/*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs createallsubdirs
[Icons]
Name: "{autoprograms}\WhereIsIt AI egui Win64 QA"; Filename: "{app}\whereisit-egui-win64.exe"
