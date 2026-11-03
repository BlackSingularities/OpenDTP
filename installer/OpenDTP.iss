; Instalator OpenDTP – w Program Files, z uprawnieniami administratora.
; Aktualizacje trafiają zawsze do tego samego folderu (stały AppId + /DIR z programu).
; Wersję podaje workflow: ISCC /DAppVersion=1.2.3 installer\OpenDTP.iss

#ifndef AppVersion
  #define AppVersion "0.8.1"
#endif

[Setup]
AppId={{4E7B1C2D-9A35-4F60-B8D1-2C6E5A9F0D37}
AppName=OpenDTP
AppVersion={#AppVersion}
AppVerName=OpenDTP {#AppVersion}
AppPublisher=Adam Rędzikowski
AppPublisherURL=https://redzikowski.com
AppSupportURL=https://redzikowski.com
AppUpdatesURL=https://redzikowski.com
AppCopyright=Pomysł i wykonanie: Adam Rędzikowski · redzikowski.com
DefaultDirName={autopf}\OpenDTP
DefaultGroupName=OpenDTP
DisableProgramGroupPage=yes
DisableDirPage=auto
UsePreviousAppDir=yes
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir=..\out
OutputBaseFilename=OpenDTP-Setup-{#AppVersion}
SetupIconFile=icon.ico
UninstallDisplayIcon={app}\OpenDTP.exe
UninstallDisplayName=OpenDTP
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=yes
ChangesAssociations=yes
VersionInfoVersion={#AppVersion}
VersionInfoDescription=OpenDTP – edytor tekstu i skład dokumentów

[Languages]
Name: "pl"; MessagesFile: "compiler:Languages\Polish.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
pl.AssocGroup=Skojarzenia plików:
pl.AssocOdtp=Otwieraj pliki .odtp w OpenDTP
pl.AssocDocx=Dodaj OpenDTP do „Otwórz za pomocą” dla .docx, .txt i .md
pl.DocType=Dokument OpenDTP
en.AssocGroup=File associations:
en.AssocOdtp=Open .odtp files with OpenDTP
en.AssocDocx=Add OpenDTP to “Open with” for .docx, .txt and .md
en.DocType=OpenDTP document

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "assocodtp"; Description: "{cm:AssocOdtp}"; GroupDescription: "{cm:AssocGroup}"
Name: "assocdocx"; Description: "{cm:AssocDocx}"; GroupDescription: "{cm:AssocGroup}"

[Files]
Source: "..\dist\OpenDTP.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\OpenDTP"; Filename: "{app}\OpenDTP.exe"
Name: "{autodesktop}\OpenDTP"; Filename: "{app}\OpenDTP.exe"; Tasks: desktopicon

[Registry]
Root: HKA; Subkey: "Software\Classes\.odtp"; ValueType: string; ValueData: "OpenDTP.Document"; Flags: uninsdeletevalue; Tasks: assocodtp
Root: HKA; Subkey: "Software\Classes\OpenDTP.Document"; ValueType: string; ValueData: "{cm:DocType}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\OpenDTP.Document\DefaultIcon"; ValueType: string; ValueData: "{app}\OpenDTP.exe,0"
Root: HKA; Subkey: "Software\Classes\OpenDTP.Document\shell\open\command"; ValueType: string; ValueData: """{app}\OpenDTP.exe"" ""%1"""
Root: HKA; Subkey: "Software\Classes\Applications\OpenDTP.exe\shell\open\command"; ValueType: string; ValueData: """{app}\OpenDTP.exe"" ""%1"""; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\.docx\OpenWithProgids"; ValueType: string; ValueName: "OpenDTP.Document"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assocdocx
Root: HKA; Subkey: "Software\Classes\.txt\OpenWithProgids"; ValueType: string; ValueName: "OpenDTP.Document"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assocdocx
Root: HKA; Subkey: "Software\Classes\.md\OpenWithProgids"; ValueType: string; ValueName: "OpenDTP.Document"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assocdocx

[Run]
Filename: "{app}\OpenDTP.exe"; Description: "{cm:LaunchProgram,OpenDTP}"; Flags: nowait postinstall skipifsilent
