; Side-by-side Rust/egui product candidate. Legacy Win32 and isolated QA have separate identities.
#ifndef AppVersion
  #define AppVersion "1.1.0-rc.1"
#endif
#ifndef BuildDir
  #define BuildDir "../../dist/windows-egui-product/payload"
#endif
#ifndef OutputDir
  #define OutputDir "../../dist/windows-egui-product"
#endif
#ifndef ReleaseLabel
  #define ReleaseLabel "unsigned prerelease"
#endif
[Setup]
AppId={{D4BE3A9B-9A09-4A12-A73B-F9A56C7B1C6A}
AppName=WhereIsIt AI egui
AppVersion={#AppVersion}
VersionInfoVersion=1.1.0.0
AppVerName=WhereIsIt AI egui {#AppVersion} ({#ReleaseLabel})
AppPublisher=WhereIsIt AI contributors
DefaultDirName={localappdata}\Programs\WhereIsIt AI egui
DefaultGroupName=WhereIsIt AI egui
OutputDir={#OutputDir}
OutputBaseFilename=WhereIsItAI-egui-{#AppVersion}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\whereisit-ai-egui.exe
AllowNoIcons=yes
ChangesAssociations=no
SetupLogging=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "{#BuildDir}/atlas-desktop-egui.exe"; DestDir: "{app}"; DestName: "whereisit-ai-egui.exe"; Flags: ignoreversion
Source: "{#BuildDir}/atlas.exe"; DestDir: "{app}"; DestName: "whereisit-ai-cli.exe"; Flags: ignoreversion
Source: "../../LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "../../THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "../../licenses/*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\WhereIsIt AI egui"; Filename: "{app}\whereisit-ai-egui.exe"
