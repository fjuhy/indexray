Unicode true
RequestExecutionLevel user
Name "IndexRay"
OutFile "..\release\IndexRay-x64-setup.exe"
InstallDir "$LOCALAPPDATA\IndexRay"
InstallDirRegKey HKCU "Software\IndexRay" "InstallLocation"
SetCompressor /SOLID lzma
!include "MUI2.nsh"
!define UNKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\IndexRay"
!define LEGACY_UNKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Folioseek"
!define RUNKEY "Software\Microsoft\Windows\CurrentVersion\Run"
Var LegacyDir

; Pass paths as environment values rather than interpolating them into PowerShell.
!macro UpdatePath add
 System::Call 'kernel32::SetEnvironmentVariableW(w "INDEXRAY_SETUP_DIR", w "$INSTDIR")'
 System::Call 'kernel32::SetEnvironmentVariableW(w "INDEXRAY_SETUP_OLD_DIR", w "$LegacyDir")'
 nsExec::ExecToLog 'powershell.exe -NoProfile -NonInteractive -Command "$$p=[Environment]::GetEnvironmentVariable($\'Path$\',$\'User$\'); $$items=@($$p -split $\';$\' | Where-Object {$$_ -and $$_ -ne $$env:INDEXRAY_SETUP_DIR -and $$_ -ne $$env:INDEXRAY_SETUP_OLD_DIR}); if (${add}) {$$items += $$env:INDEXRAY_SETUP_DIR}; [Environment]::SetEnvironmentVariable($\'Path$\', ($$items -join $\';$\'), $\'User$\')"'
 Pop $0
 ${If} $0 != 0
  SetErrorLevel 1
  Abort "Could not update user PATH."
 ${EndIf}
!macroend
!include "LogicLib.nsh"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Korean"
Function .onInit
 SetRegView 64
 ReadRegStr $0 HKCU "${UNKEY}" "InstallLocation"
 StrCmp $0 "" +2
 StrCpy $INSTDIR "$0"
 ReadRegStr $LegacyDir HKCU "${LEGACY_UNKEY}" "InstallLocation"
 ; Older Tauri installers stored this value with surrounding quotes.
 StrCpy $0 $LegacyDir 1
 StrCmp $0 '"' 0 +2
 StrCpy $LegacyDir $LegacyDir -1 1
FunctionEnd
Section
 SetRegView 64
 ${If} $LegacyDir != ""
  ${If} $LegacyDir != $INSTDIR
   ; Remove only the registered legacy executables. Leave data and unknown files alone.
   ClearErrors
   Delete "$LegacyDir\Folioseek.exe"
   Delete "$LegacyDir\IndexRay.exe"
   ${If} ${Errors}
    SetErrorLevel 1
    Abort "Close the previous Folioseek or IndexRay app before installing."
   ${EndIf}
  ${EndIf}
 ${EndIf}
 SetOutPath "$INSTDIR"
 SetOverwrite on
 File /r "..\release\IndexRay-portable-x64\*"
 WriteUninstaller "$INSTDIR\uninstall.exe"
 CreateShortcut "$SMPROGRAMS\IndexRay.lnk" "$INSTDIR\IndexRay.exe"
 ReadRegStr $0 HKCU "${RUNKEY}" "IndexRay"
 ${If} $0 == ""
  ReadRegStr $0 HKCU "${RUNKEY}" "Folioseek"
 ${EndIf}
 ${If} $0 != ""
  WriteRegStr HKCU "${RUNKEY}" "IndexRay" '"$INSTDIR\IndexRay.exe"'
 ${EndIf}
 DeleteRegValue HKCU "${RUNKEY}" "Folioseek"
 WriteRegStr HKCU "Software\IndexRay" "InstallLocation" "$INSTDIR"
 WriteRegStr HKCU "${UNKEY}" "DisplayName" "IndexRay"
 WriteRegStr HKCU "${UNKEY}" "DisplayVersion" "1.1.0"
 WriteRegStr HKCU "${UNKEY}" "Publisher" "IndexRay"
 WriteRegStr HKCU "${UNKEY}" "InstallLocation" "$INSTDIR"
 WriteRegStr HKCU "${UNKEY}" "DisplayIcon" "$INSTDIR\IndexRay.exe"
 WriteRegStr HKCU "${UNKEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
 !insertmacro UpdatePath "$$true"
 ${If} $LegacyDir != ""
  Delete "$LegacyDir\Folioseek.exe"
  Delete "$LegacyDir\folioseek-cli.exe"
  ${If} $LegacyDir != $INSTDIR
   Delete "$LegacyDir\indexray-cli.exe"
   Delete "$LegacyDir\uninstall.exe"
  ${EndIf}
  Delete "$SMPROGRAMS\Folioseek.lnk"
  DeleteRegKey HKCU "${LEGACY_UNKEY}"
  DeleteRegKey HKCU "Software\Folioseek\Folioseek"
 ${EndIf}
SectionEnd
Section "Uninstall"
 SetRegView 64
 !insertmacro UpdatePath "$$false"
 ReadRegStr $0 HKCU "${RUNKEY}" "IndexRay"
 StrCmp $0 '"$INSTDIR\IndexRay.exe"' 0 +2
 DeleteRegValue HKCU "${RUNKEY}" "IndexRay"
 Delete "$INSTDIR\IndexRay.exe"
 Delete "$INSTDIR\indexray-cli.exe"
 Delete "$INSTDIR\LICENSE-MIT.txt"
 Delete "$INSTDIR\LICENSE-APACHE.txt"
 Delete "$INSTDIR\THIRD-PARTY-NOTICES.txt"
 Delete "$INSTDIR\Guide.md"
 Delete "$SMPROGRAMS\IndexRay.lnk"
 Delete "$INSTDIR\uninstall.exe"
!macro RemoveDict file
 Delete "$INSTDIR\ko-dic\${file}"
!macroend
 !insertmacro RemoveDict "char_def.bin"
 !insertmacro RemoveDict "dict.trie"
 !insertmacro RemoveDict "dict.vals"
 !insertmacro RemoveDict "dict.valsidx"
 !insertmacro RemoveDict "dict.words"
 !insertmacro RemoveDict "dict.wordsidx"
 !insertmacro RemoveDict "matrix.mtx"
 !insertmacro RemoveDict "metadata.json"
 !insertmacro RemoveDict "NOTICE.txt"
 !insertmacro RemoveDict "unk.bin"
 RMDir "$INSTDIR\ko-dic"
 RMDir "$INSTDIR"
 DeleteRegKey HKCU "${UNKEY}"
 DeleteRegKey HKCU "Software\IndexRay"
SectionEnd
